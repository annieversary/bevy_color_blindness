use bevy::{
    asset::ChangeWatcher,
    core_pipeline::{
        clear_color::ClearColorConfig, core_3d,
        fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    },
    prelude::*,
    render::{
        extract_component::{
            ComponentUniforms, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin,
        },
        render_graph::{Node, NodeRunError, RenderGraphApp, RenderGraphContext},
        render_resource::{
            BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
            BindGroupLayoutEntry, BindingResource, BindingType, CachedRenderPipelineId,
            ColorTargetState, ColorWrites, FragmentState, MultisampleState, Operations,
            PipelineCache, PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor,
            RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
            ShaderType, TextureFormat, TextureSampleType, TextureViewDimension,
        },
        renderer::{RenderContext, RenderDevice},
        texture::BevyDefault,
        view::{ExtractedView, ViewTarget},
        RenderApp,
    },
    utils::Duration,
};

use crate::{ColorBlindnessCamera, ColorBlindnessMode, ColorBlindnessPercentages};

pub struct ColorBlindnessPlugin;

/// Component to apply the colorblind effect
///
/// Adding this component to a camera will set up the post-processing pipeline
/// which simulates color blindness.
/// This is done by adding a render pass taking the original output texture as input,
/// Then applying a shader to the whole texture, rendering it to a full screen triangle.
#[derive(Component, Default, Clone, Copy, ExtractComponent, ShaderType)]
pub struct ColorBlindnessPostProcess {
    percentages: ColorBlindnessPercentages,
}

/// handle to the color blindness simulation shader
//const COLOR_BLINDNESS_SHADER_HANDLE: HandleUntyped =
//    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 3937837360667146578);

impl Plugin for ColorBlindnessPlugin {
    fn build(&self, app: &mut App) {
        // TODO: figure out how to load that
        /*load_internal_asset!(
            app,
            COLOR_BLINDNESS_SHADER_HANDLE,
            "color_blindness.wgsl",
            Shader::from_wgsl
        );*/
        app
            // The settings will be a component that lives in the main world but will
            // be extracted to the render world every frame.
            // This makes it possible to control the effect from the main world.
            // This plugin will take care of extracting it automatically.
            // It's important to derive [`ExtractComponent`] on [`PostProcessingSettings`]
            // for this plugin to work correctly.
            .add_plugin(ExtractComponentPlugin::<ColorBlindnessPostProcess>::default())
            // The settings will also be the data used in the shader.
            // This plugin will prepare the component for the GPU by creating a uniform buffer
            // and writing the data to that buffer every frame.
            .add_plugin(UniformComponentPlugin::<ColorBlindnessPostProcess>::default());

        // We need to get the render app from the main app
        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            // Bevy's renderer uses a render graph which is a collection of nodes in a directed acyclic graph.
            // It currently runs on each view/camera and executes each node in the specified order.
            // It will make sure that any node that needs a dependency from another node
            // only runs when that dependency is done.
            //
            // Each node can execute arbitrary work, but it generally runs at least one render pass.
            // A node only has access to the render world, so if you need data from the main world
            // you need to extract it manually or with the plugin like above.
            // Add a [`Node`] to the [`RenderGraph`]
            // The Node needs to impl FromWorld
            .add_render_graph_node::<PostProcessNode>(
                // Specifiy the name of the graph, in this case we want the graph for 3d
                core_3d::graph::NAME,
                // It also needs the name of the node
                PostProcessNode::NAME,
            )
            .add_render_graph_edges(
                core_3d::graph::NAME,
                // Specify the node ordering.
                // This will automatically create all required node edges to enforce the given ordering.
                &[
                    core_3d::graph::node::TONEMAPPING,
                    PostProcessNode::NAME,
                    core_3d::graph::node::END_MAIN_PASS_POST_PROCESSING,
                ],
            );
        app.add_systems(Update, update_percentages);
    }

    fn finish(&self, app: &mut App) {
        // We need to get the render app from the main app
        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            // Initialize the pipeline
            .init_resource::<PostProcessPipeline>();
    }
}

/// The post process node used for the render graph
struct PostProcessNode {
    // The node needs a query to gather data from the ECS in order to do its rendering,
    // but it's not a normal system so we need to define it manually.
    query: QueryState<&'static ViewTarget, With<ExtractedView>>,
}

impl PostProcessNode {
    pub const NAME: &str = "post_process";
}

impl FromWorld for PostProcessNode {
    fn from_world(world: &mut World) -> Self {
        Self {
            query: QueryState::new(world),
        }
    }
}

impl Node for PostProcessNode {
    // This will run every frame before the run() method
    // The important difference is that `self` is `mut` here
    fn update(&mut self, world: &mut World) {
        // Since this is not a system we need to update the query manually.
        // This is mostly boilerplate. There are plans to remove this in the future.
        // For now, you can just copy it.
        self.query.update_archetypes(world);
    }

    // Runs the node logic
    // This is where you encode draw commands.
    //
    // This will run on every view on which the graph is running. If you don't want your effect to run on every camera,
    // you'll need to make sure you have a marker component to identify which camera(s) should run the effect.
    fn run(
        &self,
        graph_context: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        // Get the entity of the view for the render graph where this node is running
        let view_entity = graph_context.view_entity();

        // We get the data we need from the world based on the view entity passed to the node.
        // The data is the query that was defined earlier in the [`PostProcessNode`]
        let Ok(view_target) = self.query.get_manual(world, view_entity) else {
            return Ok(());
        };

        // Get the pipeline resource that contains the global data we need to create the render pipeline
        let post_process_pipeline = world.resource::<PostProcessPipeline>();

        // The pipeline cache is a cache of all previously created pipelines.
        // It is required to avoid creating a new pipeline each frame, which is expensive due to shader compilation.
        let pipeline_cache = world.resource::<PipelineCache>();

        // Get the pipeline from the cache
        let Some(pipeline) = pipeline_cache.get_render_pipeline(post_process_pipeline.pipeline_id) else {
            return Ok(());
        };

        // Get the settings uniform binding
        let settings_uniforms = world.resource::<ComponentUniforms<ColorBlindnessPostProcess>>();
        let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
            return Ok(());
        };

        // This will start a new "post process write", obtaining two texture
        // views from the view target - a `source` and a `destination`.
        // `source` is the "current" main texture and you _must_ write into
        // `destination` because calling `post_process_write()` on the
        // [`ViewTarget`] will internally flip the [`ViewTarget`]'s main
        // texture to the `destination` texture. Failing to do so will cause
        // the current main texture information to be lost.
        let post_process = view_target.post_process_write();

        // The bind_group gets created each frame.
        //
        // Normally, you would create a bind_group in the Queue set, but this doesn't work with the post_process_write().
        // The reason it doesn't work is because each post_process_write will alternate the source/destination.
        // The only way to have the correct source/destination for the bind_group is to make sure you get it during the node execution.
        let bind_group = render_context
            .render_device()
            .create_bind_group(&BindGroupDescriptor {
                label: Some("post_process_bind_group"),
                layout: &post_process_pipeline.layout,
                // It's important for this to match the BindGroupLayout defined in the PostProcessPipeline
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        // Make sure to use the source view
                        resource: BindingResource::TextureView(post_process.source),
                    },
                    BindGroupEntry {
                        binding: 1,
                        // Use the sampler created for the pipeline
                        resource: BindingResource::Sampler(&post_process_pipeline.sampler),
                    },
                    BindGroupEntry {
                        binding: 2,
                        // Set the settings binding
                        resource: settings_binding.clone(),
                    },
                ],
            });

        // Begin the render pass
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("post_process_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                // We need to specify the post process destination view here
                // to make sure we write to the appropriate texture.
                view: post_process.destination,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
        });

        // This is mostly just wgpu boilerplate for drawing a fullscreen triangle,
        // using the pipeline/bind_group created above
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

// This contains global data used by the render pipeline. This will be created once on startup.
#[derive(Resource)]
struct PostProcessPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for PostProcessPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        // We need to define the bind group layout used for our pipeline
        let layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("post_process_bind_group_layout"),
            entries: &[
                // The screen texture
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // The sampler that will be used to sample the screen texture
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                // The settings uniform that will control the effect
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: bevy::render::render_resource::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // We can create the sampler here since it won't change at runtime and doesn't depend on the view
        let sampler = render_device.create_sampler(&SamplerDescriptor::default());

        // Get the shader handle
        let shader = world
            .resource::<AssetServer>()
            .load("shaders/color_blindness.wgsl");

        let pipeline_id = world
            .resource_mut::<PipelineCache>()
            // This will add the pipeline to the cache and queue it's creation
            .queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("post_process_pipeline".into()),
                layout: vec![layout.clone()],
                // This will setup a fullscreen triangle for the vertex state
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader,
                    shader_defs: vec![],
                    // Make sure this matches the entry point of your shader.
                    // It can be anything as long as it matches here and in the shader.
                    entry_point: "fragment".into(),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::bevy_default(),
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                // All of the following property are not important for this effect so just use the default values.
                // This struct doesn't have the Default trai implemented because not all field can have a default value.
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                push_constant_ranges: vec![],
            });

        Self {
            layout,
            sampler,
            pipeline_id,
        }
    }
}

/// updates the percentages in the post processing material when the values in `ColorBlindnessCamera` change
fn update_percentages(
    mut settings: Query<
        (&mut ColorBlindnessPostProcess, &ColorBlindnessCamera),
        Changed<ColorBlindnessCamera>,
    >,
) {
    for (mut settings, camera) in &mut settings {
        let mode = if camera.enabled {
            &camera.mode
        } else {
            &ColorBlindnessMode::Normal
        };
        dbg!("test");
        settings.percentages = mode.percentages();
    }
}

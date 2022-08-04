//! Small demo of how to use the plugin
//! Shows a small scene, with four different cubes
//!
//! Holding the Space key enables the simulation
//! Pressing N cycles through the modes

use bevy::{prelude::*, window::close_on_esc};
use bevy_color_blindness::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // you can select the color blindness simulation mode with the `ColorBlindnessParams` resource
        .insert_resource(ColorBlindnessParams {
            mode: Mode::Deuteranomaly,
            ..default()
        })
        // add the plugin
        .add_plugin(ColorBlindnessPlugin)
        .add_startup_system(setup)
        .add_system(close_on_esc)
        .add_system(change_mode)
        .run();
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // create a small world
    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane { size: 5.0 })),
        material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
        ..default()
    });
    let cube = meshes.add(Mesh::from(shape::Cube { size: 0.5 }));
    commands.spawn_bundle(PbrBundle {
        mesh: cube.clone(),
        material: materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
        transform: Transform::from_xyz(0.0, 0.5, 0.0),
        ..default()
    });
    commands.spawn_bundle(PbrBundle {
        mesh: cube.clone(),
        material: materials.add(Color::rgb(1.0, 0.0, 0.0).into()),
        transform: Transform::from_xyz(2.0, 0.5, 0.0),
        ..default()
    });
    commands.spawn_bundle(PbrBundle {
        mesh: cube.clone(),
        material: materials.add(Color::rgb(0.0, 1.0, 0.0).into()),
        transform: Transform::from_xyz(3.0, 0.5, 0.0),
        ..default()
    });
    commands.spawn_bundle(PbrBundle {
        mesh: cube,
        material: materials.add(Color::rgb(0.0, 0.0, 1.0).into()),
        transform: Transform::from_xyz(4.0, 0.5, 0.0),
        ..default()
    });
    commands.spawn_bundle(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });

    // create the camera
    commands
        .spawn_bundle(Camera3dBundle {
            transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        })
        // IMPORTANT: add this component to your main camera
        .insert(ColorBlindnessCamera);
}

fn change_mode(input: Res<Input<KeyCode>>, mut params: ResMut<ColorBlindnessParams>) {
    // cycle through the modes by pressing N
    if input.just_pressed(KeyCode::N) {
        params.mode = match params.mode {
            Mode::Normal => Mode::Protanopia,
            Mode::Protanopia => Mode::Protanomaly,
            Mode::Protanomaly => Mode::Deuteranopia,
            Mode::Deuteranopia => Mode::Deuteranomaly,
            Mode::Deuteranomaly => Mode::Tritanopia,
            Mode::Tritanopia => Mode::Tritanomaly,
            Mode::Tritanomaly => Mode::Achromatopsia,
            Mode::Achromatopsia => Mode::Achromatomaly,
            Mode::Achromatomaly => Mode::Normal,
        };

        println!("Changed to {:?}", params.mode);
    }

    params.enable = input.pressed(KeyCode::Space);
}
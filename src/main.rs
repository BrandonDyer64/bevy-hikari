use crate::voxel_cone_tracing::Volume;
use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};
use std::f32::consts::PI;

mod voxel_cone_tracing;

fn main() {
    let mut app = App::new();

    app.insert_resource(Msaa { samples: 4 })
        .add_plugins(DefaultPlugins)
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(voxel_cone_tracing::VoxelConeTracingPlugin)
        .add_startup_system(setup)
        .add_system(keyboard_input_system)
        .add_system(light_rotate_system);

    // bevy_mod_debugdump::print_render_graph(&mut app);

    app.run();
}

#[derive(Component)]
struct Controller;

/// Set up a simple 3D scene
fn setup(
    mut commands: Commands,
    _asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Plane
    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane { size: 5.0 })),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.3, 0.5, 0.3),
            perceptual_roughness: 0.9,
            ..Default::default()
        }),
        ..Default::default()
    });

    // Cube
    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube::default())),
            material: materials.add(StandardMaterial {
                base_color: Color::rgb(0.8, 0.7, 0.6),
                perceptual_roughness: 0.9,
                ..Default::default()
            }),
            transform: Transform::from_xyz(0.0, 0.5, 0.0),
            ..Default::default()
        })
        .insert(Controller);

    const HALF_SIZE: f32 = 5.0;
    commands.spawn_bundle(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 10000.0,
            shadow_projection: OrthographicProjection {
                left: -HALF_SIZE,
                right: HALF_SIZE,
                bottom: -HALF_SIZE,
                top: HALF_SIZE,
                near: -10.0 * HALF_SIZE,
                far: 10.0 * HALF_SIZE,
                ..Default::default()
            },
            shadows_enabled: true,
            ..Default::default()
        },
        transform: Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -PI / 4.0,
            -PI / 4.0,
            0.0,
        )),
        ..Default::default()
    });

    // commands.spawn_scene(asset_server.load("models/FlightHelmet/FlightHelmet.gltf#Scene0"));

    // Camera
    commands
        .spawn_bundle(PerspectiveCameraBundle {
            transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..Default::default()
        })
        .insert(Volume::new(
            Vec3::new(-2.5, -2.5, -2.5),
            Vec3::new(2.5, 2.5, 2.5),
        ));

    commands.spawn_bundle(UiCameraBundle::default());
}

fn keyboard_input_system(
    keyboard_input: Res<Input<KeyCode>>,
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Controller>>,
) {
    for mut transform in query.iter_mut() {
        if keyboard_input.pressed(KeyCode::W) {
            transform.translation += Vec3::Z * time.delta_seconds();
        }
        if keyboard_input.pressed(KeyCode::A) {
            transform.translation += Vec3::X * time.delta_seconds();
        }
        if keyboard_input.pressed(KeyCode::S) {
            transform.translation -= Vec3::Z * time.delta_seconds();
        }
        if keyboard_input.pressed(KeyCode::D) {
            transform.translation -= Vec3::X * time.delta_seconds();
        }
        if keyboard_input.pressed(KeyCode::E) {
            transform.translation += Vec3::Y * time.delta_seconds();
        }
        if keyboard_input.pressed(KeyCode::Q) {
            transform.translation -= Vec3::Y * time.delta_seconds();
        }

        let speed = 0.2;
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            speed * time.delta_seconds(),
            speed * time.delta_seconds(),
            speed * time.delta_seconds(),
        ) * transform.rotation;
    }
}

fn light_rotate_system(time: Res<Time>, mut query: Query<&mut Transform, With<DirectionalLight>>) {
    for mut transform in query.iter_mut() {
        let speed = 0.25;
        transform.rotation =
            Quat::from_euler(EulerRot::XYZ, 0.0, speed * time.delta_seconds(), 0.0)
                * transform.rotation;
    }
}

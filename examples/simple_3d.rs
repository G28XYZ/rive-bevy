//! Rive state machine on a rotating 3D cube, including pointer input.
mod common;
use bevy::prelude::*;
use rive_bevy::{create_rive_image, MeshEntity, RiveTarget, SceneTarget, StateMachine};
fn main() {
    common::app()
        .add_systems(Startup, setup)
        .add_systems(Update, rotate)
        .run();
}
#[derive(Component)]
struct Cube;
fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    server: Res<AssetServer>,
) {
    let image = images.add(create_rive_image(512, 512));
    let cube = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(4., 4., 4.))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(image.clone()),
                unlit: true,
                ..default()
            })),
            Transform::default(),
            Cube,
        ))
        .id();
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0., 0., 12.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        StateMachine {
            riv: server.load("rating-animation.riv"),
            ..default()
        },
        SceneTarget {
            image: RiveTarget(image),
            mesh: MeshEntity { entity: Some(cube) },
            ..default()
        },
    ));
}
fn rotate(time: Res<Time>, mut cubes: Query<&mut Transform, With<Cube>>) {
    for mut transform in &mut cubes {
        transform.rotate_y(time.delta_secs() * 0.2);
    }
}

use std::f32::consts::{PI, TAU};

use bevy::{
    math::primitives::Sphere,
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology, SphereKind},
        render_asset::RenderAssetUsages,
    },
};
use fmc_protocol::messages;

use crate::{
    assets::AssetState, game_state::GameState, player::Player, rendering::materials, utils,
};

use super::materials::SkyMaterial;

const RADIUS: f32 = 500.0;

pub struct SkyPlugin;
impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AssetState::Loading), setup)
            .add_systems(OnEnter(GameState::Launcher), cleanup)
            .add_systems(Update, pass_time.run_if(in_state(GameState::Playing)));
    }
}

#[derive(Component)]
struct SkyBox;

#[derive(Component)]
struct Sun;

#[derive(Component)]
struct Moon;

fn cleanup(mut commands: Commands, skybox: Query<Entity, With<SkyBox>>) {
    if let Ok(entity) = skybox.get_single() {
        commands.entity(entity).despawn_recursive();
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    player_query: Query<Entity, With<Player>>,
    mut sky_materials: ResMut<Assets<materials::SkyMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands
        .entity(player_query.single())
        .with_children(|parent| {
            parent
                .spawn((
                    Mesh3d(meshes.add(Sphere::new(RADIUS).mesh().kind(SphereKind::Uv {
                        // TODO: No clue what reasonable values are
                        sectors: 50,
                        stacks: 50,
                    }))),
                    MeshMaterial3d(sky_materials.add(materials::SkyMaterial::skybox())),
                    SkyBox,
                ))
                .with_children(|parent| {
                    // The sun and moon are moved by following the rotation of the skybox
                    let cube = meshes.add(cube_mesh());

                    parent.spawn((
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(sky_materials.add(materials::SkyMaterial::sun(
                            asset_server.load("server_assets/active/textures/sun.png"),
                        ))),
                        Transform::from_xyz(RADIUS - 25.0, 0.0, 0.0)
                            .with_rotation(
                                Quat::from_rotation_y(PI / 6.0) * Quat::from_rotation_z(PI / 6.0),
                            )
                            .with_scale(Vec3::splat(50.0)),
                        Sun,
                    ));

                    parent.spawn((
                        Mesh3d(cube),
                        MeshMaterial3d(sky_materials.add(materials::SkyMaterial::moon(
                            asset_server.load("server_assets/active/textures/moon.png"),
                        ))),
                        Transform::from_xyz(-RADIUS + 100.0, 0.0, 0.0)
                            .with_rotation(
                                Quat::from_rotation_y(-PI / 6.0 + PI)
                                    * Quat::from_rotation_z(-PI / 6.0),
                            )
                            .with_scale(Vec3::splat(50.0)),
                        Moon,
                    ));

                    stars(&mut meshes, &mut sky_materials, parent);
                });
        });

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 1.0,
    });
}

fn pass_time(
    time: Res<Time>,
    mut sky_materials: ResMut<Assets<SkyMaterial>>,
    mut ambient_light: ResMut<AmbientLight>,
    mut sky_box_query: Query<&mut Transform, With<SkyBox>>,
    mut sun_query: Query<&mut Transform, (With<Sun>, Without<SkyBox>, Without<Moon>)>,
    mut moon_query: Query<&mut Transform, (With<Moon>, Without<SkyBox>, Without<Sun>)>,
    mut server_time_events: EventReader<messages::Time>,
) {
    let angle = if let Some(t) = server_time_events.read().last() {
        // TODO: Should probably disconnect if above TAU to force the server to be compliant.
        t.angle % TAU
    } else {
        return;
    };

    // TODO: Is it bad to update materials like this performancewise?
    for (_, material) in sky_materials.iter_mut() {
        material.sun_angle = angle;
    }

    let mut sky_transform = sky_box_query.single_mut();
    sky_transform.rotation = Quat::from_rotation_z(angle);
    // Sun/moon's rotation around its' own axis
    // One rotation per 5000 seconds
    let mut sun_transform = sun_query.single_mut();
    sun_transform.rotation *= Quat::from_rotation_z(TAU / 5000.0 * time.delta_secs());
    let mut moon_transform = moon_query.single_mut();
    moon_transform.rotation *= Quat::from_rotation_z(TAU / 5000.0 * time.delta_secs());

    // Max is lower so that the brightness peaks early and fades late
    let max = 0.2;
    let min = 0.3;
    let range: f32 = 1.0 - (angle.sin().min(max).max(-min) + min) / (min + max);
    // Exponential decay so that the brightness will decrease rapdidly come sunset to warn the
    // player, and to let the light linger until the halo of the sun is completely gone.
    // The exponent is chosen so that the brightness will bottom out at ~0.02 where it is just
    // bright enough to see.
    ambient_light.brightness = (range * -4.0).exp();
}

fn cube_mesh() -> Mesh {
    let min = Vec3::splat(-0.5);
    let max = Vec3::splat(0.5);

    // Inverted since the sky material renders clockwise, bevy 0.15 will have a function that does
    // it for Mesh.
    let vertices = &[
        // Front
        ([max.x, max.y, max.z], [3.0, 1.0]),
        ([max.x, min.y, max.z], [3.0, 2.0]),
        ([min.x, max.y, max.z], [2.0, 1.0]),
        ([min.x, min.y, max.z], [2.0, 2.0]),
        // Back
        ([min.x, max.y, min.z], [1.0, 1.0]),
        ([min.x, min.y, min.z], [1.0, 2.0]),
        ([max.x, max.y, min.z], [0.0, 1.0]),
        ([max.x, min.y, min.z], [0.0, 2.0]),
        // Right
        ([max.x, max.y, min.z], [4.0, 1.0]),
        ([max.x, min.y, min.z], [4.0, 2.0]),
        ([max.x, max.y, max.z], [3.0, 1.0]),
        ([max.x, min.y, max.z], [3.0, 2.0]),
        // Left
        ([min.x, max.y, max.z], [2.0, 1.0]),
        ([min.x, min.y, max.z], [2.0, 2.0]),
        ([min.x, max.y, min.z], [1.0, 1.0]),
        ([min.x, min.y, min.z], [1.0, 2.0]),
        // Top
        ([max.x, max.y, min.z], [3.0, 0.0]),
        ([max.x, max.y, max.z], [3.0, 1.0]),
        ([min.x, max.y, min.z], [2.0, 0.0]),
        ([min.x, max.y, max.z], [2.0, 1.0]),
        // Bottom
        ([max.x, min.y, max.z], [3.0, 2.0]),
        ([max.x, min.y, min.z], [3.0, 3.0]),
        ([min.x, min.y, max.z], [2.0, 2.0]),
        ([min.x, min.y, min.z], [2.0, 3.0]),
    ];

    let positions: Vec<_> = vertices.iter().map(|(p, _)| *p).collect();

    // TODO: There's has to be some proper way to do this offset stuff.
    //
    // Offsets remove black seams from sampling over the edge of the texture.
    let mut x_switch = 0;
    let mut x_offset = -0.005;
    let mut y_offset = 0.005;
    let uvs: Vec<_> = vertices
        .iter()
        .map(|(_, uv)| {
            // One less than actual width/height because of 0 indexing
            let x = uv[0] / 4.0 + x_offset;
            let y = uv[1] / 3.0 + y_offset;

            x_switch += 1;
            if x_switch % 2 == 0 {
                x_offset = -x_offset;
            }
            y_offset = -y_offset;

            [x, y]
        })
        .collect();

    let triangle_list = &[0, 1, 2, 2, 1, 3];
    let mut indices = Vec::new();
    for i in 0..=5 {
        indices.extend(triangle_list.map(|index| index + 4 * i));
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

fn stars(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<SkyMaterial>,
    parent: &mut ChildBuilder,
) {
    let min = Vec3::splat(-0.5);
    let max = Vec3::splat(0.5);

    // Vertices inverted since the sky material renders clockwise, bevy 0.15 has a function that does it
    // for the mesh.
    let vertices = vec![
        // Front
        [max.x, max.y, max.z],
        [max.x, min.y, max.z],
        [min.x, max.y, max.z],
        [min.x, min.y, max.z],
        // Back
        [min.x, max.y, min.z],
        [min.x, min.y, min.z],
        [max.x, max.y, min.z],
        [max.x, min.y, min.z],
        // Right
        [max.x, max.y, min.z],
        [max.x, min.y, min.z],
        [max.x, max.y, max.z],
        [max.x, min.y, max.z],
        // Left
        [min.x, max.y, max.z],
        [min.x, min.y, max.z],
        [min.x, max.y, min.z],
        [min.x, min.y, min.z],
        // Top
        [max.x, max.y, min.z],
        [max.x, max.y, max.z],
        [min.x, max.y, min.z],
        [min.x, max.y, max.z],
        // Bottom
        [max.x, min.y, max.z],
        [max.x, min.y, min.z],
        [min.x, min.y, max.z],
        [min.x, min.y, min.z],
    ];

    let triangle_list = &[0, 1, 2, 2, 1, 3];
    let mut indices = Vec::new();
    for i in 0..=5 {
        indices.extend(triangle_list.map(|index| index + 4 * i));
    }

    let star_mesh = meshes.add(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
        .with_inserted_indices(Indices::U32(indices)),
    );

    let material = materials.add(SkyMaterial::star());

    // Seed picked so that no stars are at the edge of the moon glow (as they would be cut in half)
    let mut rng = utils::Rng::new(1);
    let radius = RADIUS - 2.0;
    for _ in 0..700 {
        let direction = Vec3::new(rng.next_f32(), rng.next_f32(), rng.next_f32()) - 0.5;
        let position = direction.normalize() * radius;
        parent.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(position)
                .with_scale(Vec3::splat(1.0 + rng.next_f32() * 2.0))
                .with_rotation(
                    Quat::from_rotation_x(rng.next_f32() * PI)
                        * Quat::from_rotation_y(rng.next_f32() * PI)
                        * Quat::from_rotation_z(rng.next_f32() * PI),
                ),
        ));
    }
}

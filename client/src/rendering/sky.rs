// TODO: The sky should be fully defined by the server, the client should get nothing but position
// updates for celestial objects.

use bevy::{
    math::{primitives::Sphere, Vec3A},
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use fmc_protocol::messages;

use crate::{game_state::GameState, player::PlayerState, rendering::materials};

pub struct SkyPlugin;
impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostStartup,
            // This is just a hacky way to have it run after player setup, all of this should
            // be removed when the sky is defined by the server.
            setup,
        )
        .add_systems(Update, pass_time.run_if(in_state(GameState::Playing)));
    }
}

fn setup(
    mut commands: Commands,
    player_query: Query<Entity, With<PlayerState>>,
    mut sky_materials: ResMut<Assets<materials::SkyMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let player_id = player_query.single();

    let sky_entity = commands
        .spawn(MaterialMeshBundle {
            mesh: meshes.add(Sphere::new(4900.0).mesh().ico(5).unwrap()),
            material: sky_materials.add(materials::SkyMaterial::default()),
            ..Default::default()
        })
        .insert(NotShadowCaster)
        .insert(NotShadowReceiver)
        .id();

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 1.0,
    });

    commands.entity(player_id).push_children(&[sky_entity]);
}

fn pass_time(
    sky_material_query: Query<&Handle<materials::SkyMaterial>>,
    mut ambient_light: ResMut<AmbientLight>,
    mut materials: ResMut<Assets<materials::SkyMaterial>>,
    mut server_time_events: EventReader<messages::Time>,
) {
    let angle = if let Some(t) = server_time_events.read().last() {
        t.angle % std::f32::consts::TAU
    } else {
        return;
    };

    const MAX_ANGLE: f32 = std::f32::consts::PI * 1.0 / 20.0;
    ambient_light.brightness = if angle > 0.0 && angle < std::f32::consts::PI {
        if angle < MAX_ANGLE {
            angle / MAX_ANGLE
        } else if angle > std::f32::consts::PI - MAX_ANGLE {
            (std::f32::consts::PI - angle) / MAX_ANGLE
        } else {
            1.0
        }
    } else {
        0.0
    };

    let position = Vec3::new(angle.cos(), angle.sin(), 0.0);
    let handle = sky_material_query.single();
    let material = materials.get_mut(handle).unwrap();

    material.sun_position.x = position.x;
    material.sun_position.y = position.y;
    material.sun_position.z = position.z;
}

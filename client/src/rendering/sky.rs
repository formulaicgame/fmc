// TODO: The sky should be fully defined by the server, the client should get nothing but position
// updates for celestial objects.

use bevy::{
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use fmc_networking::{messages, NetworkData};

use crate::{game_state::GameState, player::PlayerState, rendering::materials};

const BRIGHTNESS: f32 = 1.0;

pub struct SkyPlugin;
impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostStartup,
            // This is just a hacky way to have it run after player setup, all of this should
            // be removed when the sky is defined by the server.
            setup,
        )
        .add_systems(Update, pass_time.run_if(GameState::in_game));
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
            mesh: meshes.add(
                Mesh::try_from(shape::Icosphere {
                    radius: 4900.0,
                    subdivisions: 5,
                })
                .unwrap(),
            ),
            material: sky_materials.add(materials::SkyMaterial::default()),
            ..Default::default()
        })
        .insert(NotShadowCaster)
        .insert(NotShadowReceiver)
        .id();

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: BRIGHTNESS,
    });

    commands.entity(player_id).push_children(&[sky_entity]);
}

fn pass_time(
    sky_material_query: Query<&Handle<materials::SkyMaterial>>,
    mut ambient_light: ResMut<AmbientLight>,
    mut materials: ResMut<Assets<materials::SkyMaterial>>,
    mut server_time_events: EventReader<NetworkData<messages::Time>>,
) {
    let angle = if let Some(t) = server_time_events.read().last() {
        t.angle
    } else {
        return;
    };

    ambient_light.brightness = angle.sin() * BRIGHTNESS;

    let position = Vec3::new(angle.cos(), angle.sin(), 0.0);
    let handle = sky_material_query.single();
    let material = materials.get_mut(handle).unwrap();

    material.sun_position.x = position.x;
    material.sun_position.y = position.y;
    material.sun_position.z = position.z;
}

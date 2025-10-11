use bevy::{math::DVec3, prelude::*};

use crate::{game_state::GameState, player::Head};

pub mod blocks;
pub mod models;
pub mod world_map;

pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(world_map::WorldMapPlugin)
            .add_plugins(models::ModelPlugin)
            .insert_resource(Origin(IVec3::ZERO))
            .add_systems(
                PostUpdate,
                update_origin.run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnEnter(GameState::Launcher), cleanup);
    }
}

fn cleanup(
    mut commands: Commands,
    mut world_map: ResMut<world_map::WorldMap>,
    mut models: ResMut<models::ModelEntities>,
) {
    for (_, chunk) in world_map.chunks.drain() {
        if let Some(entity) = chunk.entity() {
            commands.entity(entity).despawn();
        }
    }

    for entity in models.drain() {
        commands.entity(entity).despawn();
    }
}

// TODO: Transforms could have been made to be f64 as with the server, but I don't know
// enough about the rendering stuff to replace Transform. Instead this litters conversions all over
// the place.
//
// For entities that use a Transform, an offset is needed to preserve the precision of f32s. This
// is updated to be the chunk position of the player every time the player moves between chunk
// borders.
#[derive(Resource, Deref, DerefMut, Clone, Copy)]
pub struct Origin(pub IVec3);

impl Origin {
    pub fn to_local(&self, position: DVec3) -> Vec3 {
        (position - self.as_dvec3()).as_vec3()
    }

    pub fn to_global(&self, position: Vec3) -> DVec3 {
        self.as_dvec3() + position.as_dvec3()
    }
}

#[derive(Component)]
pub struct MovesWithOrigin;

fn update_origin(
    mut origin: ResMut<Origin>,
    mut positions: ParamSet<(
        Query<&GlobalTransform, (Changed<GlobalTransform>, With<Head>)>,
        Query<&mut Transform, With<MovesWithOrigin>>,
    )>,
) {
    let for_lifetime = positions.p0();
    let player_transform = if let Ok(t) = for_lifetime.single() {
        t
    } else {
        return;
    };

    let true_translation = origin.to_global(player_transform.translation());
    let new_origin = crate::utils::world_position_to_chunk_pos(true_translation.floor().as_ivec3());

    if new_origin == origin.0 {
        return;
    };

    let change = (new_origin - origin.0).as_vec3();
    for mut transform in positions.p1().iter_mut() {
        transform.translation -= change;
    }

    origin.0 = new_origin;
}

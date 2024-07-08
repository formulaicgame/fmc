use bevy::prelude::*;

use crate::{
    game_state::GameState,
    player::{PlayerCameraMarker, PlayerState},
};

pub mod blocks;
pub mod world_map;

pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(world_map::WorldMapPlugin);

        app.insert_resource(Origin(IVec3::ZERO));
        app.add_systems(PostUpdate, update_origin.run_if(GameState::in_game));
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

#[derive(Component)]
pub struct MovesWithOrigin;

fn update_origin(
    mut origin: ResMut<Origin>,
    mut positions: ParamSet<(
        Query<&GlobalTransform, (Changed<GlobalTransform>, With<PlayerCameraMarker>)>,
        Query<&mut Transform, With<MovesWithOrigin>>,
    )>,
) {
    let for_lifetime = positions.p0();
    let player_transform = if let Ok(t) = for_lifetime.get_single() {
        t
    } else {
        return;
    };

    let true_translation = player_transform.translation().as_dvec3() + origin.0.as_dvec3();
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

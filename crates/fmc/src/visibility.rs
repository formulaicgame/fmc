use std::{
    collections::{HashMap, HashSet},
    hash::RandomState,
};

use bevy::prelude::*;

use crate::{
    bevy_extensions::f64_transform::{GlobalTransform, Transform, TransformSystems},
    world::chunk::ChunkPosition,
};

pub struct VisibilityPlugin;
impl Plugin for VisibilityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EntityMap::default())
            .add_systems(
                PostUpdate,
                update_entity_map
                    .after(TransformSystems::Propagate)
                    .in_set(VisibilitySystems),
            )
            .add_systems(Last, clean_entity_map);
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct VisibilitySystems;

#[derive(Component, Default)]
#[require(Transform)]
pub enum Visibility {
    Hidden,
    #[default]
    Visible,
}

impl Visibility {
    pub fn is_visible(&self) -> bool {
        matches!(self, Self::Visible)
    }
}

/// Limits which players can see an entity
#[derive(Component, Default)]
pub struct Observers {
    whitelist: HashSet<Entity>,
    blacklist: HashSet<Entity>,
}

impl Observers {
    pub fn include(&mut self, player: Entity) {
        self.whitelist.insert(player);
        self.blacklist.clear();
    }

    pub fn exclude(&mut self, player: Entity) {
        if self.whitelist.is_empty() {
            self.blacklist.insert(player);
        }
    }

    pub fn is_included(&self, player: Entity) -> bool {
        if !self.whitelist.is_empty() {
            self.whitelist.contains(&player)
        } else {
            !self.blacklist.contains(&player)
        }
    }

    pub(crate) fn filter<'a>(&'a self, players: &'a HashSet<Entity>) -> ObserversIter<'a> {
        if !self.whitelist.is_empty() {
            ObserversIter::Union(self.whitelist.union(players))
        } else {
            ObserversIter::Difference(players.difference(&self.blacklist))
        }
    }
}

pub(crate) enum ObserversIter<'a> {
    Union(std::collections::hash_set::Union<'a, Entity, RandomState>),
    Difference(std::collections::hash_set::Difference<'a, Entity, RandomState>),
}

impl<'a> Iterator for ObserversIter<'a> {
    type Item = &'a Entity;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ObserversIter::Union(iter) => iter.next(),
            ObserversIter::Difference(iter) => iter.next(),
        }
    }
}

/// Keeps track of which chunk every entity with a [Visibility] is currently in.
#[derive(Default, Resource)]
pub struct EntityMap {
    position2entity: HashMap<ChunkPosition, HashSet<Entity>>,
    entity2position: HashMap<Entity, ChunkPosition>,
}

impl EntityMap {
    pub fn iter_entities(&self, chunk_position: &ChunkPosition) -> impl Iterator<Item = Entity> {
        self.position2entity
            .get(chunk_position)
            .into_iter()
            .flat_map(|entities| entities.iter().cloned())
    }

    pub fn chunk_position(&self, entity: &Entity) -> Option<ChunkPosition> {
        self.entity2position.get(entity).copied()
    }

    fn insert_or_move(&mut self, chunk_position: ChunkPosition, entity: Entity) {
        if let Some(current_chunk_pos) = self.entity2position.get(&entity) {
            // Move entity from one chunk to another
            if current_chunk_pos == &chunk_position {
                return;
            } else {
                self.position2entity
                    .get_mut(current_chunk_pos)
                    .unwrap()
                    .remove(&entity);

                self.position2entity
                    .entry(chunk_position)
                    .or_insert(HashSet::new())
                    .insert(entity);

                self.entity2position.insert(entity, chunk_position);
            }
        } else {
            // First time seeing entity, insert it normally
            self.position2entity
                .entry(chunk_position)
                .or_insert(HashSet::new())
                .insert(entity);
            self.entity2position.insert(entity, chunk_position);
        }
    }

    fn remove(&mut self, entity: &Entity) {
        let Some(chunk_position) = self.entity2position.remove(entity) else {
            return;
        };

        let chunk = self.position2entity.get_mut(&chunk_position).unwrap();
        chunk.remove(entity);

        if chunk.is_empty() {
            self.position2entity.remove(&chunk_position);
        }
    }
}

fn update_entity_map(
    mut entity_map: ResMut<EntityMap>,
    moved_entities: Query<(Entity, &GlobalTransform), (With<Visibility>, Changed<GlobalTransform>)>,
) {
    for (entity, global_transform) in moved_entities.iter() {
        let chunk_position = ChunkPosition::from(global_transform.translation());
        entity_map.insert_or_move(chunk_position, entity);
    }
}

fn clean_entity_map(
    mut entity_map: ResMut<EntityMap>,
    mut removed_entities: RemovedComponents<Visibility>,
) {
    for entity in removed_entities.read() {
        entity_map.remove(&entity);
    }
}

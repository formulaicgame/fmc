use std::collections::HashMap;

use bevy::prelude::*;
use fmc_networking::{messages, NetworkData, NetworkServer};

use crate::{items::ItemStack, players::Player};

pub struct InterfacePlugin;
impl Plugin for InterfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<RegisterInterfaceProvider>()
            .add_systems(Update, sort_item_updates.in_set(InterfaceEventRegistration))
            .add_systems(Update, (insert_held_item, register_item_interfaces));
    }
}

// SystemSet used to order event handling. Use .after(InterfaceEventRegistration) for systems that
// should handle interface events.
#[derive(SystemSet, Clone, PartialEq, Eq, Debug, Hash)]
pub struct InterfaceEventRegistration;

#[derive(Component, Deref, DerefMut)]
pub struct HeldInterfaceItem {
    pub item_stack: ItemStack,
}

// When interface interactions are received from a player, this maps where they should be sent. For
// example, a crafting table may want to share its unique interface between all
// players. When a player clicks a crafting table, it can respond by sending an event mapping
// "crafting_table" to the block's entity. When the server now receives updates for the
// "crafting_table" interface node, it will add them to the entity as an InterfaceEvents component.
#[derive(Component, Deref, DerefMut, Default)]
pub(crate) struct InterfaceNodes(HashMap<String, Entity>);

#[derive(Event)]
pub struct RegisterInterfaceProvider {
    /// The player the item node should be registered for.
    pub player_entity: Entity,
    /// The node path. E.g. "inventory/crafting_table"
    pub node_path: String,
    /// The entity interface events should be sent to when the node is interacted with.
    pub node_entity: Entity,
}

#[derive(Component)]
pub struct InterfaceInteractionEvents(pub Vec<NetworkData<messages::InterfaceInteraction>>);

impl InterfaceInteractionEvents {
    pub fn read(
        &mut self,
    ) -> impl Iterator<Item = NetworkData<messages::InterfaceInteraction>> + '_ {
        self.0.drain(..)
    }
}

fn register_item_interfaces(
    mut player_query: Query<&mut InterfaceNodes, With<Player>>,
    mut registration_events: EventReader<RegisterInterfaceProvider>,
) {
    for registration in registration_events.read() {
        let mut interface_nodes = player_query.get_mut(registration.player_entity).unwrap();
        interface_nodes.insert(registration.node_path.clone(), registration.node_entity);
    }
}

fn sort_item_updates(
    mut commands: Commands,
    net: Res<NetworkServer>,
    active_nodes: Query<&InterfaceNodes>,
    mut interface_events: Query<&mut InterfaceInteractionEvents>,
    mut move_events: ResMut<Events<NetworkData<messages::InterfaceInteraction>>>,
) {
    for move_event in move_events.drain() {
        let interface_path = match &*move_event {
            messages::InterfaceInteraction::TakeItem { interface_path, .. } => interface_path,
            messages::InterfaceInteraction::PlaceItem { interface_path, .. } => interface_path,
            messages::InterfaceInteraction::Button { interface_path } => interface_path,
        };

        let Some(item_node_entity) = active_nodes
            .get(move_event.source.entity())
            .map_or(None, |active| active.get(interface_path))
        else {
            // TODO: This error message presents to the player, but means nothing to someone who
            // donsn't know.
            net.send_one(move_event.source, messages::Disconnect {
                message: format!("The client tried to move an item in the '{}' interface, but the server hasn't registered that interface to the client.", interface_path)
            });
            net.disconnect(move_event.source);
            continue;
        };

        if let Ok(mut interface_events) = interface_events.get_mut(*item_node_entity) {
            interface_events.0.push(move_event);
        } else {
            commands
                .entity(*item_node_entity)
                .insert(InterfaceInteractionEvents(vec![move_event]));
        }
    }
}

fn insert_held_item(mut commands: Commands, player_query: Query<Entity, Added<Player>>) {
    for entity in player_query.iter() {
        commands.entity(entity).insert(HeldInterfaceItem {
            item_stack: ItemStack::default(),
        });
    }
}

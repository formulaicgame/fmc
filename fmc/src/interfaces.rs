use std::collections::HashMap;

use bevy::prelude::*;
use fmc_protocol::messages;

use crate::{
    items::ItemStack,
    networking::{NetworkMessage, Server},
    players::Player,
};

pub struct InterfacePlugin;
impl Plugin for InterfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<RegisterInterfaceNode>()
            .add_systems(
                Update,
                sort_interface_interactions.in_set(InterfaceEventRegistration),
            )
            .add_systems(Update, (insert_held_item, register_interface_nodes));
    }
}

/// SystemSet used to order event handling. Use .after(InterfaceEventRegistration) for systems that
/// should handle interface events.
#[derive(SystemSet, Clone, PartialEq, Eq, Debug, Hash)]
pub struct InterfaceEventRegistration;

/// The item stack currently held by the cursor
#[derive(Component, Deref, DerefMut)]
pub struct HeldInterfaceStack {
    pub item_stack: ItemStack,
}

// When interface interactions are received from a player, this maps where they should be sent. For
// example, a crafting table may want to have a unique interface for each player. When a player
// clicks a crafting table, it can respond by sending a RegisterInterfaceNode event mapping
// "crafting_table" to the block's entity. When the server now receives updates for the
// "crafting_table" interface node, it will add them to the entity as an InterfaceEvents component.
#[derive(Component, Deref, DerefMut, Default)]
pub(crate) struct InterfaceNodes(HashMap<String, Entity>);

/// Register a interface node for a player to an entity, so that when the player interacts with the
/// node, the entity is notified of the interaction.
#[derive(Event)]
pub struct RegisterInterfaceNode {
    /// The player the node should be registered to.
    pub player_entity: Entity,
    /// The node path. E.g. "inventory/crafting_table"
    pub node_path: String,
    /// The entity interface events should be sent to when the node is interacted with.
    pub node_entity: Entity,
}

/// Component that lets an entity read interface interactions
#[derive(Component)]
pub struct InterfaceEvents(Vec<NetworkMessage<messages::InterfaceInteraction>>);

impl InterfaceEvents {
    pub fn read(
        &mut self,
    ) -> impl Iterator<Item = NetworkMessage<messages::InterfaceInteraction>> + '_ {
        self.0.drain(..)
    }
}

fn register_interface_nodes(
    mut player_query: Query<&mut InterfaceNodes, With<Player>>,
    mut registration_events: EventReader<RegisterInterfaceNode>,
) {
    for registration in registration_events.read() {
        let mut interface_nodes = player_query.get_mut(registration.player_entity).unwrap();
        interface_nodes.insert(registration.node_path.clone(), registration.node_entity);
    }
}

fn sort_interface_interactions(
    mut commands: Commands,
    net: Res<Server>,
    interface_nodes: Query<&InterfaceNodes>,
    mut interface_events: Query<&mut InterfaceEvents>,
    mut interface_interactions: ResMut<Events<NetworkMessage<messages::InterfaceInteraction>>>,
) {
    for interaction in interface_interactions.drain() {
        let interface_path = match &*interaction {
            messages::InterfaceInteraction::TakeItem { interface_path, .. } => interface_path,
            messages::InterfaceInteraction::PlaceItem { interface_path, .. } => interface_path,
            messages::InterfaceInteraction::Button { interface_path } => interface_path,
        };

        let Some(node_enity) = interface_nodes
            .get(interaction.player_entity)
            .map_or(None, |active| active.get(interface_path))
        else {
            net.send_one(interaction.player_entity, messages::Disconnect {
                message: format!("The client tried to interact with the '{}' interface, but the server hasn't registered that interface for interaction.", interface_path)
            });
            net.disconnect(interaction.player_entity);
            continue;
        };

        if let Ok(mut interface_events) = interface_events.get_mut(*node_enity) {
            interface_events.0.push(interaction);
        } else {
            commands
                .entity(*node_enity)
                .insert(InterfaceEvents(vec![interaction]));
        }
    }
}

fn insert_held_item(mut commands: Commands, player_query: Query<Entity, Added<Player>>) {
    for entity in player_query.iter() {
        commands.entity(entity).insert(HeldInterfaceStack {
            item_stack: ItemStack::default(),
        });
    }
}

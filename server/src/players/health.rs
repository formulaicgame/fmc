use fmc::{
    interfaces::{
        InterfaceEventRegistration, InterfaceInteractionEvents, RegisterInterfaceProvider,
    },
    players::Player,
    prelude::*,
};

use fmc_networking::{messages, ConnectionId, NetworkData, NetworkServer};
use serde::{Deserialize, Serialize};

use super::RespawnEvent;

pub struct HealthPlugin;
impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DamageEvent>()
            .add_event::<HealEvent>()
            .add_systems(
                Update,
                (
                    add_new_players,
                    change_health,
                    fall_damage.before(change_health),
                    death_interface.after(InterfaceEventRegistration),
                ),
            );
    }
}

fn add_new_players(
    mut commands: Commands,
    new_player_query: Query<Entity, Added<Player>>,
    mut registration_events: EventWriter<RegisterInterfaceProvider>,
) {
    for player_entity in new_player_query.iter() {
        commands
            .entity(player_entity)
            .insert(FallDamage(0))
            .with_children(|parent| {
                let death_interface_entity = parent.spawn(DeathInterface).id();
                registration_events.send(RegisterInterfaceProvider {
                    player_entity,
                    node_path: String::from("death_interface"),
                    node_entity: death_interface_entity,
                });
            });
    }
}

#[derive(Component, Default, Serialize, Deserialize, Clone)]
pub struct Health {
    pub hearts: u32,
    pub max: u32,
}

impl Health {
    pub fn take_damage(&mut self, damage: u32) -> messages::InterfaceVisibilityUpdate {
        let old_hearts = self.hearts;
        self.hearts = self.hearts.saturating_sub(damage);

        let mut image_update = messages::InterfaceVisibilityUpdate::default();
        for i in self.hearts..old_hearts {
            image_update.set_hidden(format!("hotbar/health/{}", i + 1));
        }

        image_update
    }

    pub fn heal(&mut self, healing: u32) -> messages::InterfaceVisibilityUpdate {
        let old_hearts = self.hearts;
        self.hearts = self.hearts.saturating_add(healing).min(self.max);

        let mut image_update = messages::InterfaceVisibilityUpdate::default();
        for i in old_hearts..self.hearts {
            image_update.set_visible(format!("hotbar/health/{}", i + 1));
        }

        image_update
    }
}

#[derive(Component)]
pub struct FallDamage(u32);

#[derive(Event)]
struct DamageEvent {
    entity: Entity,
    damage: u32,
}

#[derive(Event)]
struct HealEvent {
    entity: Entity,
    healing: u32,
}

fn fall_damage(
    mut fall_damage_query: Query<(Entity, &mut FallDamage), With<Player>>,
    mut position_events: EventReader<NetworkData<messages::PlayerPosition>>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    for position_update in position_events.read() {
        let (entity, mut fall_damage) = fall_damage_query
            .get_mut(position_update.source.entity())
            .unwrap();

        if fall_damage.0 != 0 && position_update.velocity.y > -0.1 {
            //damage_events.send(DamageEvent {
            //    entity,
            //    damage: fall_damage.0,
            //});
            fall_damage.0 = 0;
        } else if position_update.velocity.y < 0.0 {
            fall_damage.0 = (position_update.velocity.y.abs() as u32).saturating_sub(15);
        }
    }
}

fn change_health(
    net: Res<NetworkServer>,
    mut health_query: Query<(&mut Health, &ConnectionId)>,
    mut damage_events: EventReader<DamageEvent>,
    mut heal_events: EventReader<HealEvent>,
) {
    for damage_event in damage_events.read() {
        let (mut health, connection_id) = health_query.get_mut(damage_event.entity).unwrap();
        let interface_update = health.take_damage(damage_event.damage);
        net.send_one(*connection_id, interface_update);

        if health.hearts == 0 {
            net.send_one(
                *connection_id,
                messages::InterfaceOpen {
                    interface_path: "death_screen".to_owned(),
                },
            );
        }
    }

    for event in heal_events.read() {
        let (mut health, connection_id) = health_query.get_mut(event.entity).unwrap();
        let interface_update = health.heal(event.healing);
        net.send_one(*connection_id, interface_update);
    }
}

#[derive(Component)]
struct DeathInterface;

// TODO: This should test that your health is zero. The parent of the DeathInterface is the player
// it belongs to, just query for parent.
fn death_interface(
    net: Res<NetworkServer>,
    mut interface_query: Query<
        &mut InterfaceInteractionEvents,
        (Changed<InterfaceInteractionEvents>, With<DeathInterface>),
    >,
    mut respawn_events: EventWriter<RespawnEvent>,
    mut heal_events: EventWriter<HealEvent>,
) {
    for mut interface_events in interface_query.iter_mut() {
        for interface_interaction in interface_events.read() {
            if !matches!(
                *interface_interaction,
                messages::InterfaceInteraction::Button { .. }
            ) {
                continue;
            }

            respawn_events.send(RespawnEvent {
                entity: interface_interaction.source.entity(),
            });
            heal_events.send(HealEvent {
                entity: interface_interaction.source.entity(),
                healing: u32::MAX,
            });
            net.send_one(
                interface_interaction.source,
                messages::InterfaceClose {
                    interface_path: "death_screen".to_owned(),
                },
            );
        }
    }
}

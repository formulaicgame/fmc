use crate::bevy::{ecs::system::SystemParam, math::DVec3, prelude::*};
use fmc_protocol::messages;

use crate::{
    networking::Server,
    world::{ChunkSubscriptions, chunk::ChunkPosition},
};

#[derive(Clone, Debug)]
pub struct Sound {
    name: String,
    volume: f32,
    speed: f32,
}

impl Sound {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            volume: 1.0,
            speed: 1.0,
        }
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume;
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    fn to_message(self, position: Option<DVec3>) -> messages::Sound {
        messages::Sound {
            position,
            volume: self.volume,
            speed: self.speed,
            sound: self.name,
        }
    }
}

#[derive(SystemParam)]
pub struct AudioPlayer<'w> {
    net: Res<'w, Server>,
    chunk_subscriptions: Res<'w, ChunkSubscriptions>,
}

impl<'w> AudioPlayer<'w> {
    pub fn play_at(&self, position: DVec3, sound: Sound) {
        let chunk_position = ChunkPosition::from(position);
        let Some(subscribers) = self.chunk_subscriptions.get_subscribers(&chunk_position) else {
            return;
        };
        self.net.send_many(subscribers, sound.to_message(Some(position)));
    }

    pub fn play_for(&self, player_entity: Entity, sound: Sound) {
        self.net.send_one(player_entity, sound.to_message(None));
    }
}

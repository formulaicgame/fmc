use fmc::prelude::*;
use fmc_networking::{messages, NetworkServer};

pub struct SkyPlugin;
impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Clock::default())
            .add_systems(Update, day_night_cycle);
    }
}

#[derive(Resource, DerefMut, Deref)]
struct Clock(f32);

impl Default for Clock {
    fn default() -> Self {
        // Start a little after the sun has risen so it's brighter.
        Self(20.0)
    }
}

// time == 0, dawn
// time == 600, dusk
const DAY_LENGTH: f32 = 1200.0;

fn day_night_cycle(time: Res<Time>, net: Res<NetworkServer>, mut clock: ResMut<Clock>) {
    clock.0 += time.delta_seconds();
    clock.0 %= DAY_LENGTH;

    let message = messages::Time {
        angle: clock.0 * std::f32::consts::TAU / DAY_LENGTH,
    };
    net.broadcast(message);
}

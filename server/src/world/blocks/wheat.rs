use fmc::{
    bevy::ecs::system::EntityCommands,
    blocks::{BlockData, BlockPosition, Blocks},
    prelude::*,
    world::BlockUpdate,
};

pub struct WheatPlugin;
impl Plugin for WheatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, grow);
    }
}

#[derive(Component)]
struct Wheat {
    stage: u8,
    tick: u8,
}

fn setup(mut blocks: ResMut<Blocks>) {
    let block_id = blocks.get_id("wheat_0");
    let block = blocks.get_config_mut(&block_id);
    block.set_spawn_function(spawn_wheat_0);

    let block_id = blocks.get_id("wheat_1");
    let block = blocks.get_config_mut(&block_id);
    block.set_spawn_function(spawn_wheat_1);

    let block_id = blocks.get_id("wheat_2");
    let block = blocks.get_config_mut(&block_id);
    block.set_spawn_function(spawn_wheat_2);

    let block_id = blocks.get_id("wheat_3");
    let block = blocks.get_config_mut(&block_id);
    block.set_spawn_function(spawn_wheat_3);

    let block_id = blocks.get_id("wheat_4");
    let block = blocks.get_config_mut(&block_id);
    block.set_spawn_function(spawn_wheat_4);

    let block_id = blocks.get_id("wheat_5");
    let block = blocks.get_config_mut(&block_id);
    block.set_spawn_function(spawn_wheat_5);

    let block_id = blocks.get_id("wheat_6");
    let block = blocks.get_config_mut(&block_id);
    block.set_spawn_function(spawn_wheat_6);
}

fn spawn_wheat_0(commands: &mut EntityCommands, _block_data: Option<&BlockData>) {
    commands.insert(Wheat { stage: 0, tick: 0 });
}
fn spawn_wheat_1(commands: &mut EntityCommands, _block_data: Option<&BlockData>) {
    commands.insert(Wheat { stage: 1, tick: 0 });
}
fn spawn_wheat_2(commands: &mut EntityCommands, _block_data: Option<&BlockData>) {
    commands.insert(Wheat { stage: 2, tick: 0 });
}
fn spawn_wheat_3(commands: &mut EntityCommands, _block_data: Option<&BlockData>) {
    commands.insert(Wheat { stage: 3, tick: 0 });
}
fn spawn_wheat_4(commands: &mut EntityCommands, _block_data: Option<&BlockData>) {
    commands.insert(Wheat { stage: 4, tick: 0 });
}
fn spawn_wheat_5(commands: &mut EntityCommands, _block_data: Option<&BlockData>) {
    commands.insert(Wheat { stage: 5, tick: 0 });
}
fn spawn_wheat_6(commands: &mut EntityCommands, _block_data: Option<&BlockData>) {
    commands.insert(Wheat { stage: 6, tick: 0 });
}

// TODO: Make 'tick' increment randomly.
// TODO: Only run this function at daytime?
fn grow(
    mut growing: Query<(&mut Wheat, &BlockPosition)>,
    mut block_update_writer: EventWriter<BlockUpdate>,
) {
    for (mut wheat, block_position) in growing.iter_mut() {
        if !wheat.tick != 30 {
            wheat.tick += 1;
            continue;
        }

        let blocks = Blocks::get();
        let block_id = match wheat.stage {
            0 => blocks.get_id("wheat_1"),
            1 => blocks.get_id("wheat_2"),
            2 => blocks.get_id("wheat_3"),
            3 => blocks.get_id("wheat_4"),
            4 => blocks.get_id("wheat_5"),
            5 => blocks.get_id("wheat_6"),
            6 => blocks.get_id("wheat_7"),
            _ => unreachable!(),
        };

        block_update_writer.send(BlockUpdate::Change {
            position: **block_position,
            block_id,
            block_state: None,
        });
    }
}

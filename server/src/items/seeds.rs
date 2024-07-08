use fmc::{
    blocks::{BlockId, Blocks},
    items::Items,
    players::{Camera, Player},
    prelude::*,
    world::{BlockUpdate, WorldMap},
};

use super::{ItemUses, UsableItems};

pub struct SeedPlugin;
impl Plugin for SeedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, register_seeds)
            .add_systems(Update, use_seeds.after(super::RegisterItemUse));
    }
}

fn register_seeds(
    mut commands: Commands,
    blocks: Res<Blocks>,
    items: Res<Items>,
    mut usable_items: ResMut<UsableItems>,
) {
    usable_items.insert(
        items.get_id("wheat_seeds").unwrap(),
        commands
            .spawn((
                ItemUses::default(),
                SeedConfig {
                    air: blocks.get_id("air"),
                    soil: blocks.get_id("soil"),
                },
            ))
            .id(),
    );
}

#[derive(Component)]
struct SeedConfig {
    pub air: BlockId,
    pub soil: BlockId,
}

pub fn use_seeds(
    world_map: Res<WorldMap>,
    mut hoe_uses: Query<(&mut ItemUses, &SeedConfig), Changed<ItemUses>>,
    mut block_update_writer: EventWriter<BlockUpdate>,
) {
    let Ok((mut uses, config)) = hoe_uses.get_single_mut() else {
        return;
    };

    for seed_use in uses.read() {
        let Some((block_id, block_position)) = seed_use.block else {
            continue;
        };

        if block_id != config.soil {
            continue;
        }

        if let Some(above_block) = world_map.get_block(block_position + IVec3::Y) {
            if above_block != config.air {
                continue;
            }
        } else {
            continue;
        }

        block_update_writer.send(BlockUpdate::Change {
            position: block_position + IVec3::Y,
            block_id: Blocks::get().get_id("wheat_0"),
            block_state: None,
        });
    }
}

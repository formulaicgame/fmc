use std::collections::HashMap;

use fmc::{blocks::BlockId, items::ItemId, prelude::*};

pub mod crafting;
mod ground_items;

mod bread;
mod hoes;
mod seeds;

pub use ground_items::GroundItemBundle;

pub struct ItemPlugin;
impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(UsableItems::default())
            .add_plugins(ground_items::GroundItemPlugin)
            .add_plugins(crafting::CraftingPlugin)
            .add_plugins(hoes::HoePlugin)
            .add_plugins(seeds::SeedPlugin);
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegisterItemUse;

#[derive(Resource, Deref, DerefMut, Default)]
pub struct UsableItems(HashMap<ItemId, Entity>);

struct ItemUse {
    // Player that used the item
    player_entity: Entity,
    // Block the item was used on
    block: Option<(BlockId, IVec3)>,
}

// TODO: Some items might be able to interact with multiple types of blocks. Having one
// component hold all uses makes it so you have to handle all of them in one system.
// A better approach might be to register relationships, for example, ("hoe": "dirt") and
// ("hoe": "wheat") and have these be separate entities with marker components.
//
// List of player entities that have used the item during the last tick.
#[derive(Component, Default)]
pub struct ItemUses(Vec<ItemUse>);

impl ItemUses {
    fn read(&mut self) -> impl Iterator<Item = ItemUse> + '_ {
        self.0.drain(..)
    }

    pub fn push(&mut self, player_entity: Entity, block: Option<(BlockId, IVec3)>) {
        self.0.push(ItemUse {
            player_entity,
            block,
        });
    }
}

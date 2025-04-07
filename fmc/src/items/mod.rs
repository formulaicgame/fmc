use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    assets::AssetSet,
    blocks::{BlockConfig, BlockId},
    database::Database,
    models::ModelAssetId,
};

pub type ItemId = u32;
pub const ITEM_CONFIG_PATH: &str = "assets/client/items/configurations/";

pub struct ItemPlugin;
impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_items.in_set(AssetSet::Items));
    }
}

fn load_items(mut commands: Commands, database: Res<Database>) {
    let mut items = Items {
        configs: HashMap::new(),
        ids: database.load_item_ids(),
    };

    for (filename, id) in items.ids.iter() {
        let file_path = ITEM_CONFIG_PATH.to_owned() + filename + ".json";

        let file = match std::fs::File::open(&file_path) {
            Ok(f) => f,
            Err(e) => panic!(
                "Failed to open item config at: {}\nError: {}",
                &file_path, e
            ),
        };

        let json: ItemConfigJson = match serde_json::from_reader(&file) {
            Ok(c) => c,
            Err(e) => panic!(
                "Couldn't read item config from '{}'\nError: {}",
                &file_path, e
            ),
        };

        let blocks = database.load_block_ids();
        let block = if let Some(block) = json.block {
            match blocks.get(&block) {
                Some(block_id) => Some(*block_id),
                None => panic!(
                    "Failed to parse item config at: {}\nError: Missing block by the name: {}",
                    &file_path, &block
                ),
            }
        } else {
            None
        };

        // TODO: I don't remember why this was necessary, but it would be nice if this function
        // could just wait for models to be loaded. Then database.load_models could return a vec
        // too.
        let models = database.load_models();
        let model_id = match models.get_index_of(&json.equip_model) {
            Some(id) => id as ModelAssetId,
            None => panic!(
                "Failed to parse item config at: {}\nError: Missing model by the name: {}",
                &file_path, &json.equip_model
            ),
        };

        items.configs.insert(
            *id,
            ItemConfig {
                id: *id,
                name: json.name,
                block,
                model_id,
                max_stack_size: json.stack_size,
                categories: json.categories,
                tool: json.tool,
                properties: json.properties,
            },
        );
    }

    commands.insert_resource(items);
}

pub struct ItemConfig {
    /// The id of the item's ItemConfig
    pub id: ItemId,
    /// Name shown in interfaces
    pub name: String,
    /// Block placed by the item
    pub block: Option<BlockId>,
    /// Model used to render the item
    pub model_id: ModelAssetId,
    /// The max amount a stack of this item can store
    pub max_stack_size: u32,
    /// Names used to categorize the item, e.g "helmet". Used to restrict item placement in
    /// interfaces.
    pub categories: HashSet<String>,
    /// Present if the item is used as a tool to break blocks
    pub tool: Option<Tool>,
    /// Properties unique to the item
    pub properties: serde_json::Map<String, serde_json::Value>,
}

impl ItemConfig {
    pub fn tool_efficiency(&self, block_config: &BlockConfig) -> f32 {
        if let Some(tool) = &self.tool {
            block_config
                .tools
                .contains(&tool.name)
                .then_some(tool.efficiency)
                .unwrap_or(1.0)
        } else {
            1.0
        }
    }
}

#[derive(Deserialize)]
struct ItemConfigJson {
    name: String,
    /// Block name of the block this item can place.
    block: Option<String>,
    /// Item model filename
    equip_model: String,
    stack_size: u32,
    #[serde(default)]
    categories: HashSet<String>,
    tool: Option<Tool>,
    #[serde(flatten)]
    properties: serde_json::Map<String, serde_json::Value>,
}

/// Names and configs of all the items in the game.
#[derive(Resource)]
pub struct Items {
    configs: HashMap<ItemId, ItemConfig>,
    // Map from filename/item name to item id.
    ids: HashMap<String, ItemId>,
}

impl Items {
    #[track_caller]
    pub fn get_config(&self, item_id: &ItemId) -> &ItemConfig {
        return self.configs.get(item_id).unwrap();
    }

    pub fn get_config_by_name(&self, item_name: &str) -> Option<&ItemConfig> {
        let id = self.ids.get(item_name)?;
        return self.configs.get(id);
    }

    pub fn get_id(&self, name: &str) -> Option<ItemId> {
        return self.ids.get(name).cloned();
    }

    pub fn asset_ids(&self) -> HashMap<String, ItemId> {
        return self.ids.clone();
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Item {
    /// Id of item in [`Items`]
    pub id: ItemId,
    /// Unique properties of the item. Separate from the shared properties of the ItemConfig.
    pub properties: serde_json::Value,
}

impl Item {
    pub fn new(id: ItemId) -> Self {
        return Self {
            id,
            properties: serde_json::Value::default(),
        };
    }
}

// Items with unique properties are incapabale of being equal to other items.
impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.properties == serde_json::Value::Null
            && other.properties == serde_json::Value::Null
    }
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct ItemStack {
    // The item occupying the stack
    item: Option<Item>,
    // Current stack size.
    size: u32,
    // Maximum amount storable in the stack.
    capacity: u32,
    // Override the capacity set by the item's ItemConfig
    custom_capacity: Option<u32>,
}

impl ItemStack {
    pub fn new(item_config: &ItemConfig, size: u32) -> Self {
        return Self {
            capacity: item_config.max_stack_size,
            custom_capacity: None,
            item: Some(Item::new(item_config.id)),
            size,
        };
    }

    pub fn item(&self) -> Option<&Item> {
        return self.item.as_ref();
    }

    /// Returns the total capacity minus its current size
    pub fn remaining_capacity(&self) -> u32 {
        return self
            .custom_capacity
            .unwrap_or(self.capacity)
            .saturating_sub(self.size);
    }

    /// Return the total capacity
    pub fn capacity(&self) -> u32 {
        return self.custom_capacity.unwrap_or(self.capacity);
    }

    pub fn size(&self) -> u32 {
        return self.size;
    }

    pub fn set_size(&mut self, size: u32) -> &mut Self {
        if size == 0 {
            *self = Self::default();
        }
        self.size = size;
        self
    }

    /// Set a custom capacity. The item stack's initial capacity will be preserved if transfered to
    /// an empty stack.
    pub fn set_capacity(&mut self, capacity: u32) -> &mut Self {
        self.custom_capacity = Some(capacity);
        self
    }

    /// Combine two item stacks, returns the leftover
    ///
    /// # Panics
    ///
    /// Panics if the two stacks don't contain the same item
    #[track_caller]
    pub fn add(&mut self, mut other: ItemStack) -> ItemStack {
        if self.item != other.item {
            panic!();
        }

        let amount = other.size.min(self.remaining_capacity());
        self.size += amount;
        other.take(amount);

        return other;
    }

    /// Take the given amount of items out of the item stack
    pub fn take(&mut self, amount: u32) -> ItemStack {
        let taken = ItemStack {
            item: self.item.clone(),
            size: amount.min(self.size),
            capacity: self.capacity,
            custom_capacity: None,
        };

        self.size -= taken.size;
        if self.size == 0 {
            *self = ItemStack::default();
        }

        taken
    }

    /// Move items from this stack into another, if the items do not match, swap them.
    pub fn transfer_to(&mut self, other: &mut ItemStack, amount: u32) {
        if self.is_empty() {
            return;
        } else if &self.item == &other.item {
            // Take out the requested amount if that many are available
            let to_transfer = self.take(amount);
            // Add as much as other can hold
            let mut leftover = other.add(to_transfer);
            // Transfer what's left back
            if !leftover.is_empty() {
                leftover.transfer_to(self, leftover.size());
            }
        } else if other.is_empty() {
            *other = self.take(amount);
        } else {
            self.swap(other);
        }
    }

    pub fn swap(&mut self, other: &mut ItemStack) {
        std::mem::swap(self, other);
    }

    pub fn is_empty(&self) -> bool {
        return self.item.is_none();
    }
}

#[derive(Deserialize)]
pub struct Tool {
    pub name: String,
    pub efficiency: f32,
}

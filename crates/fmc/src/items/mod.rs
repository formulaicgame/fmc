use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{blocks::BlockId, database::Database, models::ModelId};

pub type ItemId = u32;
pub const ITEM_CONFIG_PATH: &str = "resources/client/items/configurations/";

pub struct ItemPlugin;
impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_items);
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
            Some(id) => id as ModelId,
            None => panic!(
                "Failed to parse item config at: {}\nError: Missing model by the name: {}",
                &file_path, &json.equip_model
            ),
        };

        items.configs.insert(
            *id,
            ItemConfig {
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
    /// Name shown in interfaces
    pub name: String,
    /// Block placed by the item
    pub block: Option<BlockId>,
    /// Model used to render the item
    pub model_id: ModelId,
    /// The max amount a stack of this item can store
    pub max_stack_size: u32,
    /// Names used to categorize the item, e.g "helmet". Used to restrict item placement in
    /// interfaces.
    pub categories: HashSet<String>,
    /// If the item is used as a tool to break blocks faster
    pub tool: Option<Tool>,
    /// Properties unique to the item
    pub properties: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
pub struct ItemConfigJson {
    name: String,
    /// Block name of the block this item can place.
    block: Option<String>,
    /// Item model filename
    equip_model: String,
    stack_size: u32,
    #[serde(default)]
    categories: HashSet<String>,
    #[serde(default)]
    properties: serde_json::Map<String, serde_json::Value>,
    tool: Option<Tool>,
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

    pub fn get_id(&self, name: &str) -> Option<ItemId> {
        return self.ids.get(name).cloned();
    }

    pub fn asset_ids(&self) -> HashMap<String, ItemId> {
        return self.ids.clone();
    }
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Item {
    /// Id assigned to this item type, can be used to lookup properties specific to the item type.
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

// TODO: None of these members should be public, it will cause headache, did for debug
/// An ItemStack holds several of the same item. Used in interfaces.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ItemStack {
    /// The item occupying the stack
    item: Option<Item>,
    /// Current stack size.
    size: u32,
    /// Maximum amount storable in the stack.
    capacity: u32,
}

impl ItemStack {
    pub fn new(item: Item, size: u32, capacity: u32) -> Self {
        return Self {
            item: Some(item),
            size,
            capacity,
        };
    }

    pub fn item(&self) -> Option<&Item> {
        return self.item.as_ref();
    }

    pub fn capacity(&self) -> u32 {
        return self.capacity;
    }

    pub fn size(&self) -> u32 {
        return self.size;
    }

    pub fn add(&mut self, amount: u32) {
        self.size += amount;
    }

    pub fn subtract(&mut self, amount: u32) {
        self.size -= amount;
        if self.size == 0 {
            self.item = None;
            self.capacity = 0;
        }
    }

    /// Move items from this stack into another, if the items to not match, swap them.
    #[track_caller]
    pub fn transfer(&mut self, other: &mut ItemStack, mut amount: u32) {
        if self.is_empty() {
            return;
        } else if &self.item == &other.item {
            // Transfer as much as is requested, as much as there's room for, or as much as is
            // available.
            amount = std::cmp::min(amount, other.capacity - other.size);
            amount = std::cmp::min(amount, self.size);
            other.add(amount);
            self.subtract(amount);
        } else if other.is_empty() {
            other.item = self.item.clone();
            other.capacity = self.capacity;

            amount = std::cmp::min(amount, self.size);

            other.add(amount);
            self.subtract(amount);
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

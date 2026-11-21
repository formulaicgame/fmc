use std::collections::HashMap;

use crate::bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{assets::AssetSet, database::Database};

pub mod drop_table;

pub type ItemId = u32;
pub const ITEM_CONFIG_PATH: &str = "assets/client/items/configurations/";

// Items are stored as a static so that ItemStack's can be constructed with only indirect access to
// the configs through the static.
static ITEMS: once_cell::sync::OnceCell<Items> = once_cell::sync::OnceCell::new();

pub struct ItemPlugin;
impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ItemConfigEvent>()
            .add_systems(PreStartup, load_items.in_set(AssetSet::Items));
    }
}

/// Contains the parsed json of an item. Use it to extract unique configuration values.
/// Sent during PreStartup and gone after PostStartup
#[derive(Message, Clone, Debug)]
pub struct ItemConfigEvent {
    pub item_id: ItemId,
    pub name: String,
    pub json: serde_json::Value,
}

fn load_items(database: Res<Database>, mut item_config_events: MessageWriter<ItemConfigEvent>) {
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

        let json: serde_json::Value = match serde_json::from_reader(&file) {
            Ok(c) => c,
            Err(e) => panic!(
                "Couldn't read item config from '{}'\nError: {}",
                &file_path, e
            ),
        };

        let config_json = match ItemConfigJson::deserialize(&json) {
            Ok(c) => c,
            Err(e) => panic!(
                "Couldn't read item config from '{}'\nError: {}",
                &file_path, e
            ),
        };

        items.configs.insert(
            *id,
            ItemConfig {
                stack_size: config_json.stack_size,
            },
        );

        item_config_events.write(ItemConfigEvent {
            item_id: *id,
            name: filename.clone(),
            json,
        });
    }

    ITEMS.set(items).ok();
}

pub(crate) struct ItemConfig {
    /// The max amount a stack of this item can store
    pub stack_size: u32,
}

#[derive(Deserialize)]
struct ItemConfigJson {
    stack_size: u32,
}

/// Names and configs of all the items in the game.
pub(crate) struct Items {
    configs: HashMap<ItemId, ItemConfig>,
    // Map from filename/item name to item id.
    ids: HashMap<String, ItemId>,
}

impl Items {
    #[track_caller]
    pub fn get() -> &'static Self {
        ITEMS.get().unwrap()
    }

    #[track_caller]
    pub fn get_config(&self, item_id: &ItemId) -> &ItemConfig {
        return self.configs.get(item_id).unwrap();
    }

    pub fn get_id(&self, name: &str) -> Option<ItemId> {
        return self.ids.get(name).cloned();
    }

    pub fn ids(&self) -> &HashMap<String, ItemId> {
        return &self.ids;
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Item {
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
}

impl ItemStack {
    #[track_caller]
    pub fn new(item_id: ItemId, size: u32) -> Self {
        assert!(size != 0, "Item stacks can't be created with a size of 0");

        let item_config = Items::get().get_config(&item_id);
        return Self {
            capacity: item_config.stack_size,
            item: Some(Item::new(item_id)),
            size,
        };
    }

    pub fn item(&self) -> Option<&Item> {
        return self.item.as_ref();
    }

    /// Returns the total capacity minus its current size
    pub fn remaining_capacity(&self) -> u32 {
        return self.capacity.saturating_sub(self.size);
    }

    /// Return the total capacity
    pub fn capacity(&self) -> u32 {
        return self.capacity;
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

    /// Take the given amount of items out of the item stack
    pub fn take(&mut self, amount: u32) -> ItemStack {
        let size = amount.min(self.size).min(self.capacity);
        if size == 0 {
            return ItemStack::default();
        }

        let taken = ItemStack {
            item: self.item.clone(),
            size,
            capacity: self.capacity,
        };

        self.size -= size;
        if self.size == 0 {
            *self = ItemStack::default();
        }

        taken
    }

    /// Put items from this stack into another, if the items do not match, nothing is moved.
    pub fn put(&mut self, other: &mut ItemStack, amount: u32) {
        if other.is_empty() {
            *other = self.take(amount);
        } else if self.item == other.item {
            let taken = self.take(amount.min(other.remaining_capacity()));
            other.size += taken.size;
        }
    }

    pub fn pop(&mut self) -> Option<ItemStack> {
        if self.is_empty() {
            return None;
        }

        return Some(self.take(1));
    }

    pub fn is_empty(&self) -> bool {
        return self.item.is_none();
    }
}

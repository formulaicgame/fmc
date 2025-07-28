use bevy::prelude::Event;
use fmc_protocol_derive::{ClientBound, ServerBound};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Change the visibility of an interface
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone)]
pub struct InterfaceVisibilityUpdate {
    pub interface_path: String,
    pub visible: bool,
}

/// Changes the visibility of nodes within interfaces
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone, Default)]
pub struct InterfaceNodeVisibilityUpdate {
    /// List of (interface node path, visibility[true for visible, false for hidden]).
    pub updates: Vec<(String, bool)>,
}

impl InterfaceNodeVisibilityUpdate {
    pub fn set_hidden(&mut self, interface_path: String) {
        self.updates.push((interface_path, false));
    }

    pub fn set_visible(&mut self, interface_path: String) {
        self.updates.push((interface_path, true));
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ItemBox {
    /// Index of the item box in the interface.
    pub index: u32,
    /// Item stack that should be used, if no item id is given, the box will be empty.
    pub item_stack: ItemStack,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct ItemStack {
    /// Item id
    pub item_id: Option<u32>,
    /// Number of items
    pub quantity: u32,
    /// Durability of item
    pub durability: Option<u32>,
    /// Description of item
    pub description: Option<String>,
}

/// Update the content of an interface.
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone, Default)]
pub struct InterfaceItemBoxUpdate {
    /// Remove the previous item boxes before adding these. If this is true, the updates are
    /// assumed to be ordered. The index will be ignored.
    pub replace: bool,
    /// The sections of the interface, containing the itemboxes to be updated.
    pub updates: HashMap<String, Vec<ItemBox>>,
}

impl InterfaceItemBoxUpdate {
    /// Place an item in an item box
    pub fn add_itembox(
        &mut self,
        name: &str,
        item_box_id: u32,
        item_id: u32,
        quantity: u32,
        durability: Option<u32>,
        description: Option<&str>,
    ) {
        if !self.updates.contains_key(name) {
            self.updates.insert(name.to_owned(), Vec::new());
        }

        self.updates.get_mut(name).unwrap().push(ItemBox {
            index: item_box_id,
            item_stack: ItemStack {
                item_id: Some(item_id),
                quantity,
                durability,
                description: description.map(|x| x.to_owned()),
            },
        })
    }

    /// Empty the contents of an itembox
    pub fn add_empty_itembox(&mut self, name: &str, item_box_id: u32) {
        if !self.updates.contains_key(name) {
            self.updates.insert(name.to_owned(), Vec::new());
        }
        self.updates.get_mut(name).unwrap().push(ItemBox {
            index: item_box_id,
            item_stack: ItemStack {
                item_id: None,
                quantity: 0,
                durability: None,
                description: None,
            },
        })
    }

    pub fn combine(&mut self, other: InterfaceItemBoxUpdate) {
        for (interface_path, mut updates) in other.updates.into_iter() {
            if self.updates.contains_key(&interface_path) {
                self.updates
                    .get_mut(&interface_path)
                    .unwrap()
                    .append(&mut updates);
            } else {
                self.updates.insert(interface_path, updates);
            }
        }
    }
}

/// The different ways a client can interact with an interface
#[derive(ServerBound, Serialize, Deserialize, Debug, Clone)]
pub enum InterfaceInteraction {
    TakeItem {
        /// Interface identifier, formatted like "root/child/grandchild/..etc", e.g.
        /// "inventory/crafting_table"
        interface_path: String,
        /// Index that the item should be removed from.
        index: u32,
        /// Quantity of the item that should be moved.
        quantity: u32,
    },
    PlaceItem {
        /// Interface identifier, formatted like "root/child/grandchild/..etc", e.g.
        /// "inventory/crafting_table"
        interface_path: String,
        /// Index that the item should be placed at.
        index: u32,
        /// Quantity of the item that should be moved.
        quantity: u32,
    },
    Button {
        /// Path of the button that was pressed.
        interface_path: String,
    },
}

/// Tell the server which item is held in the hand
#[derive(ServerBound, Serialize, Deserialize, Debug, Clone)]
pub struct InterfaceEquipItem {
    /// Interface identifier, formatted like "root/child/grandchild/..etc", e.g.
    /// "inventory/crafting_table"
    pub interface_path: String,
    /// Item box index
    pub index: u32,
}

// #[derive(Clone, Debug, Serialize, Deserialize, Default)]
// pub struct Text {
//     pub text: String,
//     pub font_size: f32,
//     // Hex, if it is malformed it will default to white.
//     pub color: String,
// }

// TODO: Same problem as above, should contain TextAlignment and BreakLineOn
// #[derive(Clone, Debug, Serialize, Deserialize, Default)]
// pub struct Line {
//     pub index: i32,
//     pub sections: Vec<Text>,
// }
//
// impl Line {
//     pub fn with_text(&mut self, text: String, font_size: f32, color: &str) -> &mut Self {
//         self.sections.push(Text {
//             text,
//             font_size,
//             color: color.to_owned(),
//         });
//         self
//     }
// }

/// A text update for a text box
#[derive(ClientBound, Event, Serialize, Deserialize, Debug, Clone, Default)]
pub struct InterfaceTextUpdate {
    /// Interface identifier, formatted like "root/child/grandchild/..etc", e.g.
    /// "chat/history"
    pub interface_path: String,
    /// Index in the interface it should be inserted at
    pub index: i32,
    /// Visible text
    pub text: String,
    /// Font size rendered at
    pub font_size: f32,
    /// Hex color, if it is malformed it will default to white.
    pub color: String,
}

// impl InterfaceTextUpdate {
//     pub fn new(interface_path: &str) -> Self {
//         Self {
//             interface_path: interface_path.to_owned(),
//             lines: Vec::new(),
//         }
//     }
//
//     /// Appends a line to the end of the textbox
//     pub fn append_line(&mut self) -> &mut Line {
//         self.lines.push(Line {
//             index: i32::MAX,
//             sections: Vec::new(),
//         });
//         self.lines.last_mut().unwrap()
//     }
//
//     /// Prepends a line to the beginning of the textbox
//     pub fn prepend_line(&mut self) -> &mut Line {
//         self.lines.push(Line {
//             index: -1,
//             sections: Vec::new(),
//         });
//         self.lines.last_mut().unwrap()
//     }
//
//     /// Changes the line at the supplied index.
//     pub fn change_line(&mut self, index: i32) -> &mut Line {
//         self.lines.push(Line {
//             index,
//             sections: Vec::new(),
//         });
//         self.lines.last_mut().unwrap()
//     }
//
//     pub fn remove_line(&mut self, index: i32) {
//         self.lines.push(Line {
//             index,
//             sections: Vec::new(),
//         });
//     }
// }

/// Send the server the content of a text box
#[derive(ServerBound, Serialize, Deserialize, Debug, Clone, Default)]
pub struct InterfaceTextInput {
    /// Path of the text box
    pub interface_path: String,
    /// The content of the text box
    pub text: String,
}

/// Set the value of a setting in the client's settings interface.
#[derive(ClientBound, ServerBound, Event, Serialize, Deserialize, Debug, Clone)]
pub enum GuiSetting {
    /// Input text box
    TextBox {
        /// Name of the setting
        name: String,
        /// New entry
        value: String,
    },
    /// List of buttons that can be selected between, ordered from left to right
    ButtonSelection {
        /// Name of the setting
        name: String,
        /// Index of the new button
        selected: usize,
    },
    Slider {
        /// Name of the setting
        name: String,
        /// New value
        value: f32,
    },
    Switch {
        /// Name of the setting
        name: String,
        /// If the switch should be on or off
        on: bool,
    },
    Dropdown {
        /// Name of the setting
        name: String,
        /// Index of the selected value
        selected: usize,
    },
}

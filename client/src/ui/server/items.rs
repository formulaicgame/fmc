use std::collections::{HashMap, HashSet};

use bevy::{prelude::*, text::FontSmoothing, ui::FocusPolicy};

use fmc_protocol::messages;
use serde::{Deserialize, Serialize};

use crate::{
    assets::models::{ModelAssetId, Models},
    game_state::GameState,
    networking::NetworkClient,
    ui::CursorVisibility,
    world::blocks::{BlockId, Blocks},
};

use super::{InterfaceConfig, InterfaceNode, InterfacePaths};

pub type ItemId = u32;

const ITEM_IMAGE_PATH: &str = "server_assets/active/textures/items/";

pub struct ItemPlugin;
impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_item_box_updates,
                initial_select_item_box,
                discard_items.after(super::handle_toggle_events),
                // TODO: Reacting to cursor visibility isn't proper.
                (left_click_item_box, right_click_item_box)
                    .run_if(|visibiltiy: Res<CursorVisibility>| visibiltiy.server),
                update_cursor_image.after(left_click_item_box),
                update_cursor_item_stack_position,
                keyboard_select_item_box,
            )
                .run_if(in_state(GameState::Playing)),
        );
    }
}

pub struct ItemConfig {
    /// Name shown in interfaces
    pub name: String,
    /// Image shown in the interface
    pub image_path: String,
    /// Model that is displayed when the item is equipped
    pub equip_model: ModelAssetId,
    /// The max amount of an item stack of this type
    pub stack_size: u32,
    /// Names used to categorize the item, e.g "helmet". Used to restrict item placement in ui's.
    pub categories: Option<HashSet<String>>,
    /// Block that is placed when the item is used on a surface.
    pub block: Option<BlockId>,
}

#[derive(Deserialize)]
struct ItemConfigJson {
    name: String,
    image: String,
    equip_model: String,
    stack_size: u32,
    categories: Option<HashSet<String>>,
    block: Option<String>,
    //properties: serde_json::Map<String, serde_json::Value>,
}

/// Holds the configs of all the items in the game.
#[derive(Resource)]
pub struct Items {
    pub configs: HashMap<ItemId, ItemConfig>,
}

impl Items {
    // TODO: Should this be implemented as Index? There is probably convention of returning Option
    // when the function has get in the name.
    #[track_caller]
    pub fn get(&self, id: &ItemId) -> &ItemConfig {
        return self.configs.get(id).unwrap();
    }
}

// TODO: Need to validate that the models used for equipping have a "left_click" and (maybe) a
// "right_click" animation. When this function runs, most of the gltf files haven't loaded yet.
// Need some way to register which model handles need to be checked when they finish loading.
// Errors will only happen to developers, so it's preferred to crash after entering gameplay, improving
// asset loading times. As opposed to blocking until the models are loaded, to then run this
// function.
pub fn load_items(
    mut commands: Commands,
    server_config: Res<messages::ServerConfig>,
    net: Res<NetworkClient>,
    models: Res<Models>,
) {
    let blocks = Blocks::get();
    let mut configs = HashMap::new();

    for (filename, id) in server_config.item_ids.iter() {
        let file_path =
            "server_assets/active/items/configurations/".to_owned() + filename + ".json";

        let file = match std::fs::File::open(&file_path) {
            Ok(f) => f,
            Err(e) => panic!(
                "Failed to open item config at path: {}\nError: {}",
                &file_path, e
            ),
        };

        let json_config: ItemConfigJson = match serde_json::from_reader(&file) {
            Ok(c) => c,
            Err(e) => {
                net.disconnect(&format!(
                    "Misconfigured assets: failed to read item config at: {}.\n\
                        Error: {}",
                    &file_path, e
                ));
                return;
            }
        };

        let equip_model = match models.get_id_by_filename(&json_config.equip_model) {
            Some(id) => id,
            None => {
                //Server didn't send the correct set of model ids, this should never happen,
                // as the server should read models from the same set of files.
                net.disconnect(&format!(
                    "Misconfigured assets: mismatch between model name and ids. \
                        Could not find id for model at path: {}",
                    &file_path
                ));
                return;
            }
        };

        let block_id = match json_config.block {
            Some(name) => match blocks.get_id(&name) {
                Some(block_id) => Some(*block_id),
                None => {
                    net.disconnect(&format!(
                        "Misconfigured assets: failed to read item config at: '{}'. \
                            No block with the name '{}'.",
                        &file_path, &name
                    ));
                    return;
                }
            },
            None => None,
        };

        let config = ItemConfig {
            name: json_config.name,
            image_path: ITEM_IMAGE_PATH.to_owned() + &json_config.image,
            equip_model,
            stack_size: json_config.stack_size,
            categories: json_config.categories,
            block: block_id,
        };

        if !std::path::Path::new(&config.image_path).exists() {
            net.disconnect(&format!(
                "Misconfigured assets: failed to read item config at: '{}', \
                    no item image by the name '{}' at '{}', make sure it is present.",
                &file_path, json_config.image, ITEM_IMAGE_PATH,
            ));
            return;
        }

        configs.insert(*id, config);
    }

    commands.insert_resource(Items { configs });
}

/// ItemStacks are used to represent the data part of an item box in an interface.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ItemStack {
    // The item occupying the stack
    item: Option<ItemId>,
    // Maximum amount of the item type that can currently be stored in the stack.
    max_size: Option<u32>,
    // Current stack size.
    size: u32,
}

impl ItemStack {
    pub fn new(item: ItemId, max_size: u32, size: u32) -> Self {
        return Self {
            item: Some(item),
            max_size: Some(max_size),
            size,
        };
    }

    pub fn item(&self) -> Option<ItemId> {
        self.item
    }

    pub fn take(&mut self, amount: u32) -> ItemStack {
        let taken = ItemStack {
            item: self.item.clone(),
            size: amount.min(self.size),
            max_size: self.max_size,
        };

        self.size -= taken.size;
        if self.size == 0 {
            *self = ItemStack::default();
        }

        taken
    }

    fn size(&self) -> u32 {
        self.size
    }

    fn add(&mut self, amount: u32) {
        self.size += amount;
    }

    fn subtract(&mut self, amount: u32) {
        self.size -= amount;
        if self.size == 0 {
            self.item = None;
            self.max_size = None;
        }
    }

    /// Move items from this stack. If the stack's items don't match, they will be swapped.
    #[track_caller]
    pub fn transfer_to(&mut self, other: &mut ItemStack, mut amount: u32) -> u32 {
        if self.is_empty() {
            panic!("Tried to transfer from a stack that is empty, this should be asserted by the caller");
        } else if &self.item == &other.item {
            amount = std::cmp::min(amount, other.max_size.unwrap() - other.size);
            other.add(amount);
            self.subtract(amount);
            return amount;
        } else if other.is_empty() {
            other.item = self.item.clone();
            other.max_size = self.max_size.clone();

            amount = std::cmp::min(amount, self.size);

            other.add(amount);
            self.subtract(amount);
            return amount;
        } else {
            std::mem::swap(self, other);
            return other.size;
        }
    }

    pub fn is_empty(&self) -> bool {
        return self.item.is_none();
    }
}

#[derive(Component)]
pub struct ItemBox {
    pub item_stack: ItemStack,
    // Box index in the section
    pub index: usize,
}

impl ItemBox {
    fn is_empty(&self) -> bool {
        self.item_stack.is_empty()
    }
}

// The item stack is unique, and shared between all interfaces(since only one can be open at a
// time). When the interface is closed, the item is returned to the interface it was taken from.
#[derive(Component, Default)]
pub struct CursorItemBox {
    item_stack: ItemStack,
}

impl CursorItemBox {
    fn is_empty(&self) -> bool {
        self.item_stack.is_empty()
    }
}

#[derive(Deserialize, Component, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct ItemBoxSection {
    /// If it is allowed to quick move to this section.
    allow_quick_place: bool,
    /// Which item types can be placed in this section.
    allowed_item_types: Option<HashSet<String>>,
    /// If the items can be moved by mouse/keyboard interaction
    movable_items: bool,
    /// Whether items should be equipped by the hand on selection.
    #[serde(rename = "equipment")]
    pub is_equipment: bool,
}

impl ItemBoxSection {
    fn can_contain(&self, item_config: &ItemConfig) -> bool {
        if let Some(allowed) = &self.allowed_item_types {
            if let Some(categories) = &item_config.categories {
                if allowed.is_disjoint(categories) {
                    false
                } else {
                    true
                }
            } else {
                false
            }
        } else {
            true
        }
    }
}

impl Default for ItemBoxSection {
    fn default() -> Self {
        Self {
            allow_quick_place: false,
            allowed_item_types: None,
            movable_items: true,
            is_equipment: false,
        }
    }
}

// Add content to the interface sent from the server.
fn handle_item_box_updates(
    mut commands: Commands,
    net: Res<NetworkClient>,
    asset_server: Res<AssetServer>,
    interface_paths: Res<InterfacePaths>,
    items: Res<Items>,
    interface_item_box_query: Query<Option<&Children>, With<ItemBoxSection>>,
    mut item_box_update_events: EventReader<messages::InterfaceItemBoxUpdate>,
) {
    for item_box_update in item_box_update_events.read() {
        for (interface_path, new_item_boxes) in item_box_update.updates.iter() {
            let interface_entities = match interface_paths.get(interface_path) {
                Some(i) => i,
                None => {
                    net.disconnect(&format!(
                        "Server sent item box update for interface with name: {}, but there is no interface by that name.",
                        &interface_path
                    ));
                    return;
                }
            };

            for entity in interface_entities.iter() {
                let children = match interface_item_box_query.get(*entity) {
                    Ok(c) => c,
                    Err(_) => {
                        net.disconnect(&format!(
                                "Server sent item box update for interface with name: {}, but the interface is not configured to contain item boxes.",
                                &interface_path
                                ));
                        return;
                    }
                };

                // TODO: This breaks the interface. Item images dissapear. I think it is a bug in the
                // AssetServer, when all handles to an image are dropped, the image is unloaded. If a
                // new handle is then created it will not load the image again.
                if item_box_update.replace {
                    todo!()
                    //commands.entity(*entity).despawn_descendants();
                }

                for item_box in new_item_boxes.iter() {
                    let item_stack = if let Some(item_id) = &item_box.item_stack.item_id {
                        let item_config = match items.configs.get(item_id) {
                            Some(i) => i,
                            None => {
                                net.disconnect(&format!(
                                        "While updating the '{}' interface the server sent an unrecognized item id {}",
                                        &interface_path,
                                        item_id
                                        ));
                                return;
                            }
                        };
                        ItemStack::new(
                            *item_id,
                            item_config.stack_size,
                            item_box.item_stack.quantity,
                        )
                    } else {
                        ItemStack::default()
                    };

                    let mut entity_commands = if item_box_update.replace || children.is_none() {
                        let mut entity_commands = commands.spawn_empty();
                        entity_commands.insert(ChildOf(*entity));
                        entity_commands
                    } else if let Some(child_entity) =
                        children.unwrap().get(item_box.index as usize)
                    {
                        let mut entity_commands = commands.entity(*child_entity);
                        entity_commands.despawn_related::<Children>();
                        entity_commands
                    } else {
                        let mut entity_commands = commands.spawn_empty();
                        entity_commands.insert(ChildOf(*entity));
                        entity_commands
                    };

                    let item_stack_size = item_stack.size;
                    entity_commands.insert((
                        ImageNode {
                            image: if let Some(item_id) = &item_stack.item {
                                asset_server.load(&items.get(item_id).image_path).into()
                            } else {
                                default()
                            },
                            color: if item_stack.item.is_some() {
                                Color::WHITE
                            } else {
                                Color::NONE
                            },
                            ..default()
                        },
                        Node {
                            width: Val::Px(14.0),
                            height: Val::Px(15.8),
                            margin: UiRect {
                                //left: Val::Px(0.0),
                                //right: Val::Px(0.0),
                                top: Val::Px(0.1),
                                bottom: Val::Px(0.1),
                                ..default()
                            },
                            // https://github.com/bevyengine/bevy/issues/6879
                            //padding: UiRect {
                            //    left: Val::Px(1.0),
                            //    right: Val::Auto,
                            //    top: Val::Px(1.0),
                            //    bottom: Val::Auto,
                            //},
                            // puts item count text in the bottom right corner
                            flex_direction: FlexDirection::ColumnReverse,
                            align_items: AlignItems::FlexEnd,
                            ..default()
                        },
                        // TODO: This doesn't actually block? Can't highlight items because of it.
                        FocusPolicy::Block,
                        Interaction::default(),
                        ItemBox {
                            item_stack,
                            index: item_box.index as usize,
                        },
                    ));

                    // Item count text
                    entity_commands.with_children(|parent| {
                        parent.spawn((
                            Node {
                                top: Val::Px(1.0),
                                left: Val::Px(2.0),
                                ..default()
                            },
                            Text(item_stack_size.to_string()),
                            TextColor::from(if item_stack_size > 1 {
                                Color::WHITE
                            } else {
                                Color::NONE
                            }),
                            TextFont {
                                font: asset_server.load("server_assets/active/font.otf"),
                                font_size: 8.0,
                                font_smoothing: FontSmoothing::None,
                                ..default()
                            },
                        ));
                    });
                }
            }
        }
    }
}

// TODO: This highlights, but there is a bug. Even though the highlight is spawned as a child
// entity of the item box and the item box has FocusPolicy::Block, it still triggers a
// Interaction change to Interaction::None, causing a loop.
//
//fn highlight_item_box_on_hover(
//    mut commands: Commands,
//    interaction_query: Query<(Entity, &Interaction), (Changed<Interaction>, With<ItemBox>)>,
//    mut highlighted_item_box: Local<Option<Entity>>,
//) {
//
//    // Iterate through all interactions to make sure the item box which was left gets its
//    // highlight cleared before a new one is added. If we go box -> box, it might try to add before
//    // it is removed.
//    for (_, interaction) in interaction_query.iter() {
//        if *interaction == Interaction::None {
//            match highlighted_item_box.take() {
//                // Have to do despawn_recursive here or it crashes for some reason.
//                // Even though it has not children (???) Wanted just "despawn".
//                // Perhaps related https://github.com/bevyengine/bevy/issues/267
//                Some(e) => commands.entity(e).despawn_recursive(),
//                None => (),
//            };
//        }
//    }
//    for (entity, interaction) in interaction_query.iter() {
//        if *interaction == Interaction::Hovered {
//            *highlighted_item_box = Some(commands.entity(entity).add_children(|parent| {
//                parent
//                    .spawn_bundle(NodeBundle {
//                        style: Style {
//                            position_type: PositionType::Absolute,
//                            size: Size {
//                                width: Val::Px(16.0),
//                                height: Val::Px(16.0),
//                            },
//                            ..default()
//                        },
//                        color: UiColor(Color::Rgba {
//                            red: 1.0,
//                            green: 1.0,
//                            blue: 1.0,
//                            alpha: 0.7,
//                        }),
//                        ..default()
//                    })
//                    .id()
//            }));
//            //dbg!(highlighted_item_box.unwrap());
//        }
//    }
//}

// TODO: Interaction only supports Clicked, so can't distinguish between left and right click for
//       fancy placement without hacking.
fn left_click_item_box(
    net: Res<NetworkClient>,
    items: Res<Items>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    item_box_section_query: Query<(&ItemBoxSection, &InterfaceNode)>,
    mut item_box_query: Query<(&mut ItemBox, &Interaction, &ChildOf), Changed<Interaction>>,
    mut cursor_item_box_query: Query<&mut CursorItemBox>,
    mut item_box_update_events: EventWriter<messages::InterfaceItemBoxUpdate>,
) {
    for (mut item_box, interaction, parent) in item_box_query.iter_mut() {
        if *interaction != Interaction::Pressed {
            return;
        }

        let mut cursor_box = cursor_item_box_query.single_mut().unwrap();
        let (item_box_section, interface_node) = item_box_section_query.get(parent.0).unwrap();

        if mouse_button_input.just_pressed(MouseButton::Left)
            && !keyboard_input.pressed(KeyCode::ShiftLeft)
        {
            if cursor_box.is_empty() && !item_box.is_empty() {
                // Take item from box
                let item_config = items.get(&item_box.item_stack.item.unwrap());

                let transfered = item_box
                    .item_stack
                    .transfer_to(&mut cursor_box.item_stack, item_config.stack_size);

                net.send_message(messages::InterfaceInteraction::TakeItem {
                    interface_path: interface_node.path.clone(),
                    index: item_box.index as u32,
                    quantity: transfered,
                })
            } else if !cursor_box.is_empty() {
                // place held item, swap if box is not empty
                let item_config = items.get(&cursor_box.item_stack.item.unwrap());
                if !item_box_section.can_contain(item_config) {
                    continue;
                }

                // TODO: When used directly in the function the borrow checker say bad, even though
                // good
                let size = cursor_box.item_stack.size;
                let transfered = cursor_box
                    .item_stack
                    .transfer_to(&mut item_box.item_stack, size);

                net.send_message(messages::InterfaceInteraction::PlaceItem {
                    interface_path: interface_node.path.clone(),
                    index: item_box.index as u32,
                    quantity: transfered,
                })
            }
        }

        let mut item_box_update = messages::InterfaceItemBoxUpdate::default();

        if item_box.item_stack.is_empty() {
            item_box_update.add_empty_itembox(&interface_node.path, item_box.index as u32);
        } else {
            item_box_update.add_itembox(
                &interface_node.path,
                item_box.index as u32,
                item_box.item_stack.item.unwrap(),
                item_box.item_stack.size,
                None,
                None,
            );
        }

        // Multiple interfaces might share the same content, like a hotbar will be
        // represented both in the players inventory, and at the bottom of the screen. We only
        // change one of the item stacks here, and we need the change to be reflected in both
        // interfaces. Instead of changing it in both places, we construct a false server update.
        // The server update is processed as normal, and the change will be shown in both
        // interfaces.
        item_box_update_events.write(item_box_update);
    }
}

fn right_click_item_box(
    net: Res<NetworkClient>,
    items: Res<Items>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    item_box_section_query: Query<(&ItemBoxSection, &InterfaceNode)>,
    mut item_box_query: Query<(&mut ItemBox, &Interaction, &ChildOf)>,
    mut cursor_item_box_query: Query<&mut CursorItemBox>,
    mut item_box_update_events: EventWriter<messages::InterfaceItemBoxUpdate>,
) {
    if !mouse_button_input.just_pressed(MouseButton::Right) {
        return;
    }
    // TODO: Hack until bevy merges bevy_mod_picking to master. Probably 0.15 lucky if 0.14
    let mut clicked = None;
    for entity in item_box_query.iter_mut() {
        if *entity.1 == Interaction::Hovered {
            clicked = Some(entity);
            break;
        }
    }
    let Some((mut item_box, _, parent)) = clicked else {
        return;
    };

    let mut cursor_box = cursor_item_box_query.single_mut().unwrap();
    let (item_box_section, interface_node) = item_box_section_query.get(parent.0).unwrap();

    if cursor_box.is_empty() && !item_box.is_empty() {
        // TODO: This is a special condition for item boxes that are considered
        // output-only. e.g. crafting output. Given all the different actions that can
        // be intended by a click I think it should be configured through the interface
        // config. (Some key combo) -> "place/take" etc
        let transfered = if item_box_section
            .allowed_item_types
            .as_ref()
            .is_some_and(|allowed_types| allowed_types.is_empty())
        {
            let size = item_box.item_stack.size;
            item_box
                .item_stack
                .transfer_to(&mut cursor_box.item_stack, size)
        } else {
            // If even, take half, if odd take half + 1
            let size = (item_box.item_stack.size + 1) / 2;
            item_box
                .item_stack
                .transfer_to(&mut cursor_box.item_stack, size)
        };

        net.send_message(messages::InterfaceInteraction::TakeItem {
            interface_path: interface_node.path.clone(),
            index: item_box.index as u32,
            quantity: transfered,
        })
    } else if !cursor_box.is_empty() {
        // place held item, swap if box is not empty
        let item_config = items.get(&cursor_box.item_stack.item.unwrap());

        if item_box_section
            .allowed_item_types
            .as_ref()
            .is_some_and(|allowed_types| allowed_types.is_empty())
            && cursor_box.item_stack.item == item_box.item_stack.item
        {
            let size = item_box.item_stack.size;
            let transfered = item_box
                .item_stack
                .transfer_to(&mut cursor_box.item_stack, size);

            net.send_message(messages::InterfaceInteraction::TakeItem {
                interface_path: interface_node.path.clone(),
                index: item_box.index as u32,
                quantity: transfered,
            });
        } else {
            if !item_box_section.can_contain(item_config) {
                return;
            }

            let transfered = cursor_box
                .item_stack
                .transfer_to(&mut item_box.item_stack, 1);

            net.send_message(messages::InterfaceInteraction::PlaceItem {
                interface_path: interface_node.path.clone(),
                index: item_box.index as u32,
                quantity: transfered,
            })
        };
    }

    let mut item_box_update = messages::InterfaceItemBoxUpdate::default();

    if item_box.item_stack.is_empty() {
        item_box_update.add_empty_itembox(&interface_node.path, item_box.index as u32);
    } else {
        item_box_update.add_itembox(
            &interface_node.path,
            item_box.index as u32,
            item_box.item_stack.item.unwrap(),
            item_box.item_stack.size,
            None,
            None,
        );
    }

    // Multiple interfaces might share the same content, like a hotbar will be
    // represented both in the players inventory, and at the bottom of the screen. We only
    // change one of the item stacks here, and we need the change to be reflected in both
    // interfaces. Instead of changing it in both places, we construct a fake server update.
    // The server update is processed as normal, and the change will be shown in both
    // interfaces.
    item_box_update_events.write(item_box_update);
}

fn update_cursor_item_stack_position(
    ui_scale: Res<UiScale>,
    mut cursor_move_event: EventReader<CursorMoved>,
    mut held_item_stack_query: Query<&mut Node, With<CursorItemBox>>,
) {
    for cursor_movement in cursor_move_event.read() {
        let mut node = held_item_stack_query.single_mut().unwrap();
        node.left = Val::Px(cursor_movement.position.x / ui_scale.0 as f32 - 8.0);
        node.top = Val::Px(cursor_movement.position.y / ui_scale.0 as f32 - 8.0);
    }
}

fn update_cursor_image(
    asset_server: Res<AssetServer>,
    items: Res<Items>,
    mut cursor_item_query: Query<(&mut ImageNode, &CursorItemBox, &Children)>,
    // TODO: Add marker component
    mut text_query: Query<(&mut Text, &mut TextColor, &mut TextFont)>,
) {
    for (mut image, cursor_box, children) in cursor_item_query.iter_mut() {
        if let Some(item_id) = cursor_box.item_stack.item {
            image.image = asset_server.load(&items.get(&item_id).image_path).into();
            image.color = Color::WHITE;

            let (mut text, mut color, mut font) = text_query.get_mut(children[0]).unwrap();
            *text = Text(cursor_box.item_stack.size.to_string());
            *font = TextFont {
                font: asset_server.load("server_assets/active/font.otf"),
                font_size: 8.0,
                font_smoothing: FontSmoothing::None,
                ..default()
            };
            *color = TextColor(if cursor_box.item_stack.size > 1 {
                Color::WHITE
            } else {
                Color::NONE
            });
        } else {
            // Instead of hiding the node through visibility we mask it with the color. This is
            // because the item box still needs to be interactable so items can be put into it.
            image.color = Color::NONE;
            let (mut text, mut color, mut font) = text_query.get_mut(children[0]).unwrap();
            *text = Text(cursor_box.item_stack.size.to_string());
            *font = TextFont {
                font: asset_server.load("server_assets/active/font.otf"),
                font_size: 6.0,
                font_smoothing: FontSmoothing::None,
                ..default()
            };
            *color = TextColor(Color::NONE);
        }
    }
}

// TODO: Getting ahead of myself, but the idea here is to append one of these to all interfaces
// that contain item boxes. This way it can be used both for equipping items and for navigating
// the item boxes through keyboard input.
//
// TODO: All interfaces with this component will render an outline around the selected item.
// An item is always selected, and it persists on open/close of the interface.
#[derive(Component)]
pub struct SelectedItemBox(pub Entity);

fn initial_select_item_box(
    mut commands: Commands,
    item_box_section_query: Query<&ItemBoxSection>,
    added_itembox_query: Query<(Entity, &ItemBox, &ChildOf), Added<ItemBox>>,
) {
    for (box_entity, item_box, parent) in added_itembox_query.iter() {
        if item_box.index == 0 {
            let item_box_section = item_box_section_query.get(parent.0).unwrap();
            if item_box_section.is_equipment {
                commands
                    .entity(parent.0)
                    .insert(SelectedItemBox(box_entity));
            }
        }
    }
}

fn keyboard_select_item_box(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut item_box_section_query: Query<
        (&Children, &Visibility, &mut SelectedItemBox),
        With<ItemBoxSection>,
    >,
) {
    for key in keyboard.get_just_pressed() {
        for (children, visibility, mut selected) in item_box_section_query.iter_mut() {
            if visibility == Visibility::Hidden {
                continue;
            }

            *selected = match key {
                KeyCode::Digit1 => match children.get(0) {
                    Some(entity) => SelectedItemBox(*entity),
                    None => continue,
                },
                KeyCode::Digit2 => match children.get(1) {
                    Some(entity) => SelectedItemBox(*entity),
                    None => continue,
                },
                KeyCode::Digit3 => match children.get(2) {
                    Some(entity) => SelectedItemBox(*entity),
                    None => continue,
                },
                KeyCode::Digit4 => match children.get(3) {
                    Some(entity) => SelectedItemBox(*entity),
                    None => continue,
                },
                KeyCode::Digit5 => match children.get(4) {
                    Some(entity) => SelectedItemBox(*entity),
                    None => continue,
                },
                KeyCode::Digit6 => match children.get(5) {
                    Some(entity) => SelectedItemBox(*entity),
                    None => continue,
                },
                KeyCode::Digit7 => match children.get(6) {
                    Some(entity) => SelectedItemBox(*entity),
                    None => continue,
                },
                KeyCode::Digit8 => match children.get(7) {
                    Some(entity) => SelectedItemBox(*entity),
                    None => continue,
                },
                KeyCode::Digit9 => match children.get(8) {
                    Some(entity) => SelectedItemBox(*entity),
                    None => continue,
                },
                _ => continue,
            };
        }
    }
}

// If an item is held by the cursor and the interface closes, or if you click outside the interface
// the item is considered "discarded".
fn discard_items(
    net: Res<NetworkClient>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    item_box_section_query: Query<&InterfaceConfig, Changed<InheritedVisibility>>,
    interaction_query: Query<&Interaction, With<ImageNode>>,
    mut cursor_box: Single<&mut CursorItemBox>,
) {
    if cursor_box.is_empty() {
        return;
    }

    if !item_box_section_query.is_empty() {
        net.send_message(messages::InterfaceInteraction::PlaceItem {
            interface_path: "".to_owned(),
            index: 0,
            quantity: cursor_box.item_stack.size(),
        });

        cursor_box.item_stack = ItemStack::default();
        return;
    }

    // If the cursor is over anything, there's nothing more to do
    for interaction in interaction_query.iter() {
        if *interaction != Interaction::None {
            return;
        }
    }

    for mouse_button in mouse_button_input.get_just_pressed() {
        let discarded = match mouse_button {
            MouseButton::Left => cursor_box.item_stack.take(u32::MAX),
            MouseButton::Right => cursor_box.item_stack.take(1),
            _ => continue,
        };

        net.send_message(messages::InterfaceInteraction::PlaceItem {
            interface_path: "".to_owned(),
            index: 0,
            quantity: discarded.size(),
        });
    }
}

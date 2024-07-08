use std::collections::{HashMap, HashSet};

use bevy::{gltf::Gltf, prelude::*};

use fmc_networking::{
    messages::{self, ServerConfig},
    ConnectionId, NetworkClient, NetworkData,
};
use serde::{Deserialize, Serialize};

use crate::{
    assets::models::Models,
    game_state::GameState,
    world::blocks::{BlockId, Blocks},
};

use super::{InterfaceNode, InterfacePaths};

pub type ItemId = u32;

const ITEM_IMAGE_PATH: &str = "server_assets/textures/items/";

pub struct ItemPlugin;
impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (
                    handle_item_box_updates,
                    initial_select_item_box,
                    return_cursor_item.after(super::handle_toggle_events),
                )
                    .run_if(GameState::in_game),
                (
                    left_click_item_box,
                    right_click_item_box,
                    update_cursor_image.after(left_click_item_box),
                    update_cursor_item_stack_position,
                    keyboard_select_item_box,
                )
                    // Don't run when paused
                    .run_if(in_state(GameState::Playing)),
            ),
        );
    }
}

pub struct ItemConfig {
    /// Name shown in interfaces
    pub name: String,
    /// Image shown in the interface
    pub image_path: String,
    /// Model id, used to identify item to be equipped
    pub model_handle: Handle<Gltf>,
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
    server_config: Res<ServerConfig>,
    net: Res<NetworkClient>,
    models: Res<Models>,
) {
    let blocks = Blocks::get();
    let mut configs = HashMap::new();

    for (filename, id) in server_config.item_ids.iter() {
        let file_path = "server_assets/items/configurations/".to_owned() + filename + ".json";

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
                    "Misconfigured resource pack: failed to read item config at: {}.\n\
                        Error: {}",
                    &file_path, e
                ));
                return;
            }
        };

        let model_handle = match models.get_id_by_filename(&json_config.equip_model) {
            Some(id) => models.get(&id).unwrap().handle.clone(),
            None => {
                //Server didn't send the correct set of model ids, this should never happen,
                // as the server should read models from the same set of files.
                net.disconnect(&format!(
                    "Misconfigured resource pack: mismatch between model name and ids. \
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
                        "Misconfigured resource pack: failed to read item config at: '{}'. \
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
            model_handle,
            stack_size: json_config.stack_size,
            categories: json_config.categories,
            block: block_id,
        };

        if !std::path::Path::new(&config.image_path).exists() {
            net.disconnect(&format!(
                "Misconfigured resource pack: failed to read item config at: '{}', \
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
    pub item: Option<ItemId>,
    // Maximum amount of the item type that can currently be stored in the stack.
    max_size: Option<u32>,
    // Current stack size.
    pub size: u32,
}

impl ItemStack {
    pub fn new(item: ItemId, max_size: u32, size: u32) -> Self {
        return Self {
            item: Some(item),
            max_size: Some(max_size),
            size,
        };
    }

    fn add(&mut self, amount: u32) {
        self.size += amount;
    }

    pub fn subtract(&mut self, amount: u32) {
        self.size -= amount;
        if self.size == 0 {
            self.item = None;
            self.max_size = None;
        }
    }

    /// Move items from this stack. If this stack already contains items, the stack's items need to
    /// match, otherwise they will be swapped.
    #[track_caller]
    pub fn transfer(&mut self, other: &mut ItemStack, mut amount: u32) -> u32 {
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
    mut item_box_update_events: EventReader<NetworkData<messages::InterfaceItemBoxUpdate>>,
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
                        entity_commands.set_parent(*entity);
                        entity_commands
                    } else if let Some(child_entity) =
                        children.unwrap().get(item_box.index as usize)
                    {
                        let mut entity_commands = commands.entity(*child_entity);
                        entity_commands.despawn_descendants();
                        entity_commands
                    } else {
                        let mut entity_commands = commands.spawn_empty();
                        entity_commands.set_parent(*entity);
                        entity_commands
                    };

                    // Item count text
                    entity_commands.with_children(|parent| {
                        parent.spawn(TextBundle {
                            text: Text::from_section(
                                item_stack.size.to_string(),
                                TextStyle {
                                    font: asset_server.load("server_assets/font.otf"),
                                    font_size: 8.0,
                                    color: if item_stack.size > 1 {
                                        Color::WHITE
                                    } else {
                                        Color::NONE
                                    },
                                    ..default()
                                },
                            ),
                            style: Style {
                                top: Val::Px(1.0),
                                left: Val::Px(2.0),
                                ..default()
                            },
                            ..default()
                        });
                    });

                    entity_commands
                        .insert(ImageBundle {
                            image: if let Some(item_id) = &item_stack.item {
                                asset_server.load(&items.get(item_id).image_path).into()
                            } else {
                                UiImage::default()
                            },
                            background_color: if item_stack.item.is_some() {
                                Color::WHITE.into()
                            } else {
                                Color::NONE.into()
                            },
                            // TODO: This doesn't actually block? Can't highlight items because of it.
                            focus_policy: bevy::ui::FocusPolicy::Block,
                            style: Style {
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
                            ..default()
                        })
                        .insert(Interaction::default())
                        .insert(ItemBox {
                            item_stack,
                            index: item_box.index as usize,
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
    mut item_box_query: Query<(&mut ItemBox, &Interaction, &Parent), Changed<Interaction>>,
    mut cursor_item_box_query: Query<&mut CursorItemBox>,
    mut item_box_update_events: EventWriter<NetworkData<messages::InterfaceItemBoxUpdate>>,
) {
    for (mut item_box, interaction, parent) in item_box_query.iter_mut() {
        if *interaction != Interaction::Pressed {
            return;
        }

        let mut cursor_box = cursor_item_box_query.single_mut();
        let (item_box_section, interface_node) = item_box_section_query.get(parent.get()).unwrap();

        if mouse_button_input.just_pressed(MouseButton::Left)
            && !keyboard_input.pressed(KeyCode::ShiftLeft)
        {
            if cursor_box.is_empty() && !item_box.is_empty() {
                // Take item from box
                let item_config = items.get(&item_box.item_stack.item.unwrap());

                let transfered = item_box
                    .item_stack
                    .transfer(&mut cursor_box.item_stack, item_config.stack_size);

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
                    .transfer(&mut item_box.item_stack, size);

                net.send_message(messages::InterfaceInteraction::PlaceItem {
                    interface_path: interface_node.path.clone(),
                    index: item_box.index as u32,
                    quantity: transfered,
                })
            }
        }

        let mut item_box_update = messages::InterfaceItemBoxUpdate::new();

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

        // Multiple interfaces might share the same content, like a traditional hotbar will be
        // represented both in the players inventory, and at the bottom of the screen. We only
        // change one of the item stacks here, and we need the change to be reflected in both
        // interfaces. Instead of changing it in both places, we construct a false server update.
        // The server update is processed as normal, and the change will be shown in both
        // interfaces.
        item_box_update_events.send(NetworkData::new(net.connection_id(), item_box_update));
    }
}

fn right_click_item_box(
    net: Res<NetworkClient>,
    items: Res<Items>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    item_box_section_query: Query<(&ItemBoxSection, &InterfaceNode)>,
    mut item_box_query: Query<(&mut ItemBox, &Interaction, &Parent)>,
    mut cursor_item_box_query: Query<&mut CursorItemBox>,
    mut item_box_update_events: EventWriter<NetworkData<messages::InterfaceItemBoxUpdate>>,
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

    let mut cursor_box = cursor_item_box_query.single_mut();
    let (item_box_section, interface_node) = item_box_section_query.get(parent.get()).unwrap();

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
                .transfer(&mut cursor_box.item_stack, size)
        } else {
            // If even, take half, if odd take half + 1
            let size = (item_box.item_stack.size + 1) / 2;
            item_box
                .item_stack
                .transfer(&mut cursor_box.item_stack, size)
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
                .transfer(&mut cursor_box.item_stack, size);

            net.send_message(messages::InterfaceInteraction::TakeItem {
                interface_path: interface_node.path.clone(),
                index: item_box.index as u32,
                quantity: transfered,
            });
        } else {
            if !item_box_section.can_contain(item_config) {
                return;
            }

            let transfered = cursor_box.item_stack.transfer(&mut item_box.item_stack, 1);

            net.send_message(messages::InterfaceInteraction::PlaceItem {
                interface_path: interface_node.path.clone(),
                index: item_box.index as u32,
                quantity: transfered,
            })
        };
    }

    let mut item_box_update = messages::InterfaceItemBoxUpdate::new();

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

    // Multiple interfaces might share the same content, like a traditional hotbar will be
    // represented both in the players inventory, and at the bottom of the screen. We only
    // change one of the item stacks here, and we need the change to be reflected in both
    // interfaces. Instead of changing it in both places, we construct a false server update.
    // The server update is processed as normal, and the change will be shown in both
    // interfaces.
    item_box_update_events.send(NetworkData::new(net.connection_id(), item_box_update));
}

fn update_cursor_item_stack_position(
    ui_scale: Res<UiScale>,
    mut cursor_move_event: EventReader<CursorMoved>,
    mut held_item_stack_query: Query<&mut Style, With<CursorItemBox>>,
) {
    for cursor_movement in cursor_move_event.read() {
        let mut style = held_item_stack_query.single_mut();
        style.left = Val::Px(cursor_movement.position.x / ui_scale.0 as f32 - 8.0);
        style.top = Val::Px(cursor_movement.position.y / ui_scale.0 as f32 - 8.0);
    }
}

fn update_cursor_image(
    asset_server: Res<AssetServer>,
    items: Res<Items>,
    mut cursor_item_query: Query<(
        &mut UiImage,
        &CursorItemBox,
        &mut BackgroundColor,
        &Children,
    )>,
    // TODO: Add marker component
    mut text_query: Query<&mut Text>,
) {
    for (mut image, cursor_box, mut color, children) in cursor_item_query.iter_mut() {
        if let Some(item_id) = cursor_box.item_stack.item {
            *image = asset_server.load(&items.get(&item_id).image_path).into();
            *color = BackgroundColor(Color::WHITE);

            let mut text = text_query.get_mut(children[0]).unwrap();
            *text = Text::from_section(
                cursor_box.item_stack.size.to_string(),
                TextStyle {
                    font: asset_server.load("server_assets/font.otf"),
                    font_size: 8.0,
                    color: if cursor_box.item_stack.size > 1 {
                        Color::WHITE
                    } else {
                        Color::NONE
                    },
                },
            );
        } else {
            // Instead of hiding the node through visibility we mask it with the color. This is
            // because the item box still needs to be interactable so items can be put into it.
            *color = BackgroundColor(Color::NONE);
            let mut text = text_query.get_mut(children[0]).unwrap();
            *text = Text::from_section(
                cursor_box.item_stack.size.to_string(),
                TextStyle {
                    font: asset_server.load("server_assets/font.otf"),
                    font_size: 6.0,
                    color: Color::NONE,
                },
            );
        }
    }
}

// TODO: Getting ahead of myself, but the idea here is to append one of these to all interfaces
// that contain item boxes. This way it can be used both for equipping items and for navigating
// the item boxes through keyboard input.
//
// All interfaces with this component will render an outline around the selected item.
// An item is always selected, and it persists on open/close of the interface.
#[derive(Component)]
pub struct SelectedItemBox(pub Entity);

fn initial_select_item_box(
    mut commands: Commands,
    item_box_section_query: Query<&ItemBoxSection>,
    added_itembox_query: Query<(Entity, &ItemBox, &Parent), Added<ItemBox>>,
) {
    for (box_entity, item_box, parent) in added_itembox_query.iter() {
        if item_box.index == 0 {
            let item_box_section = item_box_section_query.get(parent.get()).unwrap();
            if item_box_section.is_equipment {
                commands
                    .entity(parent.get())
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

// TODO: This is a crude 'If any interface changes visibility, return the item'. It will
// fail if there is no room. And I don't know what should happen if it's not an interface
// root that is toggled.
fn return_cursor_item(
    net: Res<NetworkClient>,
    items: Res<Items>,
    visibility_changed: Query<(), (Changed<Visibility>, With<InterfaceNode>)>,
    item_box_section_query: Query<(
        &ItemBoxSection,
        Option<&Children>,
        &InheritedVisibility,
        &InterfaceNode,
    )>,
    mut cursor_item_box_query: Query<&mut CursorItemBox>,
    mut item_box_query: Query<&mut ItemBox>,
) {
    if visibility_changed.iter().count() == 0 {
        return;
    }
    let mut cursor_box = cursor_item_box_query.single_mut();
    if !cursor_box.is_empty() {
        for (item_box_section, children, visibility, interface_node) in
            item_box_section_query.iter()
        {
            // Test that the item box section is part of the currently open interface
            if !visibility.get() {
                continue;
            }

            let item_config = items.get(&cursor_box.item_stack.item.unwrap());
            if !item_box_section.can_contain(item_config) {
                continue;
            }

            if let Some(children) = children {
                for item_box_entity in children.iter() {
                    let mut item_box = item_box_query.get_mut(*item_box_entity).unwrap();
                    if item_box.item_stack.item == cursor_box.item_stack.item {
                        let transfered = item_box
                            .item_stack
                            .transfer(&mut cursor_box.item_stack, u32::MAX);
                        net.send_message(messages::InterfaceInteraction::PlaceItem {
                            interface_path: interface_node.path.clone(),
                            index: item_box.index as u32,
                            quantity: transfered,
                        })
                    }

                    if cursor_box.is_empty() {
                        return;
                    }
                }

                // Has to be split from above because we first want it to fill up any existing
                // stacks before it begins on empty stacks.
                for item_box_entity in children.iter() {
                    let mut item_box = item_box_query.get_mut(*item_box_entity).unwrap();
                    if item_box.is_empty() {
                        let transfered = item_box
                            .item_stack
                            .transfer(&mut cursor_box.item_stack, u32::MAX);
                        net.send_message(messages::InterfaceInteraction::PlaceItem {
                            interface_path: interface_node.path.clone(),
                            index: item_box.index as u32,
                            quantity: transfered,
                        });

                        return;
                    }
                }
            }
        }
    }
}

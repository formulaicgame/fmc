use bevy::{
    asset::{load_internal_binary_asset, weak_handle, RenderAssetUsages},
    ecs::system::EntityCommands,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    },
    prelude::*,
    ui::{FocusPolicy, RelativeCursorPosition},
};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::ui::{
    client::{GuiState, BASE_SIZE},
    text_input::TextBox,
    DEFAULT_FONT_HANDLE, DEFAULT_FONT_SIZE,
};

const BUTTON_TEXTURE: Handle<Image> = weak_handle!("fefe18ec-6ad9-47e2-b1a1-8a31e22495b2");
const TAB_TEXTURE: Handle<Image> = weak_handle!("d275b6df-1aec-4930-9802-54d5c06354e5");
const DROPDOWN_ARROW_TEXTURE: Handle<Image> = weak_handle!("4a62be12-67cf-44e5-84af-b3c58ad88a0e");

pub mod colors {
    use bevy::color::Color;

    pub const BUTTON_GREEN: Color = Color::srgb_u8(0, 220, 0);
    pub const BUTTON_RED: Color = Color::srgb_u8(220, 0, 0);
    pub const TAB_ACTIVE: Color = Color::srgb_u8(31, 28, 25);
    pub const TAB_INACTIVE: Color = Color::srgb_u8(20, 18, 16);
    pub(super) const TAB_SECONDARY: Color = Color::srgb_u8(57, 50, 47);
    pub const HEADER_DARK: Color = Color::BLACK;
    pub const HEADER_LIGHT: Color = TAB_SECONDARY;
    pub const FOOTER_DARK: Color = HEADER_DARK;
    pub const FOOTER_LIGHT: Color = HEADER_LIGHT;
}

pub struct WidgetPlugin;
impl Plugin for WidgetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                toggle_switch,
                change_frame_color,
                set_slider_value,
                dropdown_interaction,
                button_selection,
                update_settings_slider_text,
            )
                .run_if(not(in_state(GuiState::None))),
        )
        .add_systems(
            PostUpdate,
            move_slider_thumb.after(bevy::ui::UiSystem::Layout),
        );

        let load_image = |bytes: &[u8], _path: String| -> Image {
            Image::from_buffer(
                bytes,
                ImageType::Format(ImageFormat::Png),
                CompressedImageFormats::NONE,
                true,
                ImageSampler::nearest(),
                RenderAssetUsages::RENDER_WORLD,
            )
            .expect("Failed to load image")
        };
        load_internal_binary_asset!(
            app,
            BUTTON_TEXTURE,
            "../../../assets/ui/button.png",
            load_image
        );
        load_internal_binary_asset!(app, TAB_TEXTURE, "../../../assets/ui/tab.png", load_image);
        load_internal_binary_asset!(
            app,
            DROPDOWN_ARROW_TEXTURE,
            "../../../assets/ui/dropdown_arrow.png",
            load_image
        );
    }
}

pub struct ButtonStyle {
    pub width: Val,
    pub height: Val,
    /// Main color of the button
    pub color: Color,
    /// Image that can be shown alongside the text
    pub image: Option<Handle<Image>>,
    /// Which side of the text the image should be shown.
    pub flex_direction: FlexDirection,
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            color: Color::default(),
            image: None,
            flex_direction: FlexDirection::default(),
        }
    }
}

#[derive(Debug)]
struct ColorSet {
    body: Color,
    light_border: Color,
    dark_border: Color,
    shadow: Color,
}

impl ColorSet {
    fn new(color: Color) -> Self {
        Self {
            body: color.mix(&Color::BLACK, 0.2),
            light_border: color.mix(&Color::BLACK, 0.0),
            dark_border: color.mix(&Color::BLACK, 0.1),
            shadow: color.mix(&Color::BLACK, 0.4),
        }
    }
}

// Inserted on Buttons and switches to make it possible to change their colors
#[derive(Component)]
pub struct FrameColor {
    colors: ColorSet,
    light_border: Entity,
    dark_border: Entity,
    body: Entity,
    shadow: Entity,
}

impl FrameColor {
    // The entities are filled in when spawning
    fn new(color: Color) -> Self {
        Self {
            colors: ColorSet::new(color),
            light_border: Entity::PLACEHOLDER,
            dark_border: Entity::PLACEHOLDER,
            body: Entity::PLACEHOLDER,
            shadow: Entity::PLACEHOLDER,
        }
    }

    pub fn set(&mut self, color: Color) {
        self.colors = ColorSet::new(color);
    }
}

#[derive(Component)]
pub struct Switch {
    // Entity of the ui element that flips back and forth
    switch: Entity,
    on: bool,
    transition: Option<f32>,
}

impl Switch {
    pub fn on(&self) -> bool {
        self.on
    }

    fn toggle(&mut self) {
        self.on = !self.on;
        self.transition = Some(0.0);
    }
}

#[derive(Component)]
pub struct Slider {
    value: f32,
    min: f32,
    max: f32,
    decimals: usize,
    thumb: Entity,
    rail: Entity,
    unit: String,
}

impl Slider {
    pub fn set_value(&mut self, value: f32) {
        self.value = (value.max(self.min).min(self.max) - self.min) / (self.max - self.min);
    }

    pub fn value(&self) -> f32 {
        ((self.segments() * self.value).round() / self.segments()) * (self.max - self.min)
            + self.min
    }

    fn segments(&self) -> f32 {
        (self.max - self.min) * 10.0f32.powi(self.decimals as i32)
    }
}

pub struct SliderStyle {
    pub width: Val,
    pub height: Val,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub decimals: usize,
    pub unit: String,
}

impl Default for SliderStyle {
    fn default() -> Self {
        Self {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            value: 0.5,
            min: 0.0,
            max: 1.0,
            decimals: 0,
            unit: String::new(),
        }
    }
}

// The square that moves inside the slider
#[derive(Component)]
struct SliderThumb;

#[derive(Component)]
struct SliderRail;

#[derive(Component)]
struct Dropdown {
    menu_entity: Entity,
    current_choice_entity: Entity,
    choice_entities: Vec<Entity>,
    choices: Vec<String>,
    selected: usize,
}

impl Dropdown {
    pub fn selected(&self) -> &str {
        &self.choices[self.selected]
    }
}

#[derive(Default)]
pub struct DropdownStyle {
    pub width: Val,
    pub height: Val,
}

#[derive(Component)]
struct DropdownChoice {
    index: usize,
    // The dropdown it's part of
    parent: Entity,
    // Overlay texture
    texture: Entity,
}

pub trait Widgets {
    /// A rectangular button with a centered label
    fn spawn_button<'a>(&'a mut self, text: &str, style: ButtonStyle) -> EntityCommands<'a>;
    /// A Switch that can be turned on or off
    fn spawn_switch<'a>(&'a mut self, on: bool) -> EntityCommands<'a>;
    /// A horizontal slider used to choose numerical values
    fn spawn_slider<'a>(&'a mut self, style: SliderStyle) -> EntityCommands<'a>;
    /// A dropdown to select between option. The choice Type will be used as the generic type in
    /// the Dropdown<T> inserted on the entity.
    fn spawn_dropdown<'a>(
        &'a mut self,
        choices: Vec<String>,
        selected: usize,
        style: DropdownStyle,
    ) -> EntityCommands<'a>;
    /// A rectangular textbox for text input
    fn spawn_textbox<'a>(&'a mut self, textbox: TextBox) -> EntityCommands<'a>;
    /// Spawns text with shadow
    fn spawn_text<'a>(&'a mut self, text: &str) -> EntityCommands<'a>;
    /// Spawn a tab, management done separately
    fn spawn_tab<'a>(&'a mut self, text: &str, width: Val, active: bool) -> EntityCommands<'a>;
    /// Lines at the top of the interface
    fn spawn_header<'a>(&'a mut self) -> EntityCommands<'a>;
    /// Lines at the bottom of the interface
    fn spawn_footer<'a>(&'a mut self) -> EntityCommands<'a>;
}

impl Widgets for ChildSpawnerCommands<'_> {
    fn spawn_button<'a>(&'a mut self, text: &str, style: ButtonStyle) -> EntityCommands<'a> {
        let mut frame = FrameColor::new(style.color);

        let mut entity_commands = self.spawn((
            Node {
                width: style.width,
                height: style.height,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            FocusPolicy::Block,
            Interaction::default(),
            // XXX: https://github.com/DioxusLabs/taffy/issues/834
            // This camouflages the gap
            BackgroundColor::from(frame.colors.shadow),
        ));

        frame.shadow = entity_commands.id();

        entity_commands.with_children(|parent| {
            parent
                .spawn(Node {
                    height: Val::Percent(100.0),
                    width: Val::Percent(100.0),
                    ..default()
                })
                .with_children(|parent| {
                    // Borders
                    frame.light_border = parent
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border: UiRect {
                                    left: BASE_SIZE,
                                    top: BASE_SIZE,
                                    ..default()
                                },
                                ..default()
                            },
                            BorderColor::from(frame.colors.light_border),
                        ))
                        .id();
                    frame.dark_border = parent
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border: UiRect {
                                    right: BASE_SIZE,
                                    bottom: BASE_SIZE,
                                    ..default()
                                },
                                ..default()
                            },
                            BorderColor::from(frame.colors.dark_border),
                        ))
                        .id();

                    // Content
                    frame.body = parent
                        .spawn((
                            Node {
                                height: Val::Percent(100.0),
                                width: Val::Percent(100.0),
                                border: UiRect::all(BASE_SIZE),
                                overflow: Overflow::clip_x(),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                flex_direction: style.flex_direction,
                                column_gap: BASE_SIZE * 4.0,
                                padding: UiRect::top(BASE_SIZE * 2.0).with_bottom(BASE_SIZE * 2.0),
                                row_gap: BASE_SIZE * 2.0,
                                ..default()
                            },
                            ImageNode {
                                // TODO: https://github.com/bevyengine/bevy/issues/9213
                                // Pre-mix until transparency is fixed I guess, doesn't really
                                // matter.
                                color: frame.colors.body.mix(&Color::WHITE, 0.07),
                                image: BUTTON_TEXTURE,
                                image_mode: NodeImageMode::Sliced(TextureSlicer {
                                    border: BorderRect {
                                        left: 7.0,
                                        right: 9.0,
                                        top: 2.0,
                                        bottom: 3.0,
                                    },
                                    // Removes some extra pixels that are displayed when stretched,
                                    // idk why. Probably has something to do with top and bottom
                                    // connecting, so it's not really a 9 slice but a 6 slice.
                                    sides_scale_mode: SliceScaleMode::Tile { stretch_value: 0.0 },
                                    // This makes the image scale to fit the container
                                    max_corner_scale: f32::MAX,
                                    ..default()
                                }),
                                ..default()
                            },
                            BackgroundColor::from(frame.colors.body),
                        ))
                        .with_children(|parent| {
                            if let Some(image) = style.image {
                                parent.spawn((
                                    Node {
                                        height: match style.flex_direction {
                                            // TODO: It's important for scaling that this is a
                                            // multiple of 2 for good image scaling. Linking it
                                            // like this to the font scale is no good.
                                            FlexDirection::Row => Val::Px(DEFAULT_FONT_SIZE + 1.0),
                                            FlexDirection::RowReverse => {
                                                Val::Px(DEFAULT_FONT_SIZE + 1.0)
                                            }
                                            FlexDirection::Column => Val::Auto,
                                            FlexDirection::ColumnReverse => Val::Auto,
                                        },
                                        ..default()
                                    },
                                    ImageNode {
                                        color: Color::WHITE,
                                        image,
                                        ..default()
                                    },
                                ));
                            }
                            parent.spawn_text(text);
                        })
                        .id();
                });

            parent.spawn((
                Node {
                    height: BASE_SIZE * 2.0,
                    width: Val::Percent(100.0),
                    ..default()
                },
                // The shadow is already colored in by the parent, the node is just needed to
                // take up space.
                // BackgroundColor::from(frame.colors.shadow),
            ));
        });

        entity_commands.insert(frame);

        return entity_commands;
    }

    fn spawn_textbox<'a>(&'a mut self, mut text_box: TextBox) -> EntityCommands<'a> {
        let mut entity_commands = self.spawn((
            Node {
                width: text_box.width,
                height: text_box.height,
                // flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Interaction::default(),
            BackgroundColor::from(Srgba::rgba_u8(53, 60, 74, 100)),
        ));

        entity_commands.with_children(|parent| {
            parent
                .spawn(Node {
                    height: Val::Percent(100.0),
                    width: Val::Percent(100.0),
                    ..default()
                })
                .with_children(|parent| {
                    // Borders
                    parent.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border: UiRect {
                                left: BASE_SIZE,
                                top: BASE_SIZE,
                                ..default()
                            },
                            ..default()
                        },
                        BorderColor::from(Srgba::rgb_u8(156, 156, 156)),
                    ));
                    parent.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border: UiRect {
                                right: BASE_SIZE,
                                bottom: BASE_SIZE,
                                ..default()
                            },
                            ..default()
                        },
                        BorderColor::from(Srgba::rgb_u8(146, 146, 146)),
                    ));

                    // Content
                    parent
                        .spawn(Node {
                            border: UiRect::all(BASE_SIZE),
                            height: Val::Percent(100.0),
                            width: Val::Percent(100.0),
                            padding: UiRect::left(Val::Percent(2.0)),
                            overflow: Overflow::clip_x(),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|parent| {
                            text_box.text_entity = if !text_box.text.is_empty() {
                                parent.spawn_text(&text_box.text).id()
                            } else {
                                parent.spawn_text(&text_box.placeholder_text).id()
                            };
                        });
                });

            parent.spawn((
                Node {
                    height: BASE_SIZE * 2.0,
                    width: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor::from(Srgba::rgb_u8(52, 52, 52)),
            ));
        });

        entity_commands.insert(text_box);

        return entity_commands;
    }

    fn spawn_switch<'a>(&'a mut self, on: bool) -> EntityCommands<'a> {
        let mut frame = if on {
            FrameColor::new(colors::BUTTON_GREEN)
        } else {
            FrameColor::new(Color::srgb_u8(102, 97, 95))
        };

        let mut entity_commands = self.spawn((
            Node {
                width: BASE_SIZE * 18.0,
                height: BASE_SIZE * 12.0,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Interaction::default(),
            BackgroundColor::default(),
        ));

        frame.shadow = entity_commands.id();
        let mut switch_entity = Entity::PLACEHOLDER;

        entity_commands.with_children(|parent| {
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    height: BASE_SIZE * 10.0,
                    ..default()
                })
                .with_children(|parent| {
                    // Borders
                    frame.light_border = parent
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border: UiRect {
                                    left: BASE_SIZE,
                                    top: BASE_SIZE,
                                    ..default()
                                },
                                ..default()
                            },
                            BorderColor::default(),
                        ))
                        .id();
                    frame.dark_border = parent
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border: UiRect {
                                    right: BASE_SIZE,
                                    bottom: BASE_SIZE,
                                    ..default()
                                },
                                ..default()
                            },
                            BorderColor::default(),
                        ))
                        .id();

                    // Content
                    frame.body = parent
                        .spawn(Node {
                            height: Val::Percent(100.0),
                            width: Val::Percent(100.0),
                            border: UiRect::all(BASE_SIZE),
                            // XXX: Manually centered with margins instead!
                            // Using JustifyContent will not center because it aligns by node edges
                            // instead of node centers.
                            // justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|parent| {
                            // on marker
                            parent.spawn((
                                Node {
                                    margin: UiRect::left(BASE_SIZE * 3.5),
                                    width: BASE_SIZE,
                                    height: BASE_SIZE * 2.0,
                                    ..default()
                                },
                                BackgroundColor::from(Color::srgb_u8(216, 216, 216)),
                            ));

                            // off marker
                            parent.spawn((
                                Node {
                                    margin: UiRect::left(BASE_SIZE * 6.0),
                                    width: BASE_SIZE * 3.0,
                                    height: BASE_SIZE * 3.0,
                                    border: UiRect::all(BASE_SIZE),
                                    ..default()
                                },
                                BorderColor::from(Color::srgb_u8(216, 216, 216)),
                            ));

                            // switch
                            switch_entity = parent
                                .spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        margin: if on {
                                            UiRect::left(Val::Percent(50.0))
                                        } else {
                                            UiRect::left(Val::Percent(0.0))
                                        },
                                        width: BASE_SIZE * 8.0,
                                        height: BASE_SIZE * 8.0,
                                        ..default()
                                    },
                                    BackgroundColor::from(Color::srgb_u8(216, 216, 216)),
                                ))
                                .id();
                        })
                        .id();
                });

            parent.spawn((
                Node {
                    height: BASE_SIZE * 2.0,
                    width: Val::Percent(100.0),
                    ..default()
                },
                // The shadow is already colored in by the parent, the node is just needed to
                // take up space.
                // BackgroundColor::from(frame.colors.shadow),
            ));
        });

        entity_commands.insert((
            Switch {
                switch: switch_entity,
                on,
                transition: None,
            },
            frame,
        ));

        return entity_commands;
    }

    fn spawn_slider<'a>(&'a mut self, style: SliderStyle) -> EntityCommands<'a> {
        let frame = FrameColor::new(colors::FOOTER_LIGHT);
        let mut slider = Slider {
            value: (style.value - style.min) / (style.max - style.min),
            min: style.min,
            max: style.max,
            decimals: style.decimals,
            thumb: Entity::PLACEHOLDER,
            rail: Entity::PLACEHOLDER,
            unit: style.unit,
        };

        let mut entity_commands = self.spawn((
            Node {
                width: style.width,
                height: style.height,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor::from(frame.colors.shadow),
            Interaction::default(),
        ));

        entity_commands.with_children(|parent| {
            parent
                .spawn(Node {
                    height: Val::Percent(100.0),
                    width: Val::Percent(100.0),
                    ..default()
                })
                .with_children(|parent| {
                    // Borders
                    parent.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border: UiRect {
                                left: BASE_SIZE,
                                top: BASE_SIZE,
                                ..default()
                            },
                            ..default()
                        },
                        BorderColor::from(frame.colors.light_border),
                    ));
                    parent.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border: UiRect {
                                right: BASE_SIZE,
                                bottom: BASE_SIZE,
                                ..default()
                            },
                            ..default()
                        },
                        BorderColor::from(frame.colors.dark_border),
                    ));

                    // Content
                    parent
                        .spawn((
                            Node {
                                height: Val::Percent(100.0),
                                width: Val::Percent(100.0),
                                border: UiRect::all(BASE_SIZE),
                                padding: UiRect::top(BASE_SIZE * 0.25)
                                    .with_bottom(BASE_SIZE * 0.25),
                                ..default()
                            },
                            BackgroundColor::from(frame.colors.body),
                        ))
                        .with_children(|parent| {
                            let colors = ColorSet::new(colors::BUTTON_GREEN);

                            // Rail
                            slider.rail = parent
                                .spawn((
                                    SliderRail,
                                    RelativeCursorPosition::default(),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        height: Val::Percent(100.0),
                                        width: Val::Percent(100.0),
                                        ..default()
                                    },
                                ))
                                .id();

                            // Thumb
                            slider.thumb = parent
                                .spawn((
                                    SliderThumb,
                                    Node {
                                        height: Val::Percent(100.0),
                                        width: Val::Percent((100.0 / slider.segments()).max(10.0)),
                                        // aspect_ratio: Some(0.8),
                                        // The layout engine might want to shrink the thumb if the
                                        // left margin is too high to fit it, causing the thumb
                                        // width to shrink, causing the margin to grow again and so
                                        // on. Shrinking is not allowed.
                                        flex_shrink: 0.0,
                                        flex_direction: FlexDirection::Column,
                                        ..default()
                                    },
                                    BackgroundColor::from(colors.shadow),
                                ))
                                .with_children(|parent| {
                                    parent
                                        .spawn((
                                            Node {
                                                width: Val::Percent(100.0),
                                                height: Val::Percent(100.0),
                                                ..default()
                                            },
                                            BackgroundColor::from(colors.body),
                                        ))
                                        .with_children(|parent| {
                                            parent.spawn((
                                                Node {
                                                    position_type: PositionType::Absolute,
                                                    width: Val::Percent(100.0),
                                                    height: Val::Percent(100.0),
                                                    border: UiRect {
                                                        left: BASE_SIZE,
                                                        top: BASE_SIZE,
                                                        ..default()
                                                    },
                                                    ..default()
                                                },
                                                BorderColor::from(colors.light_border),
                                            ));
                                            parent.spawn((
                                                Node {
                                                    position_type: PositionType::Absolute,
                                                    width: Val::Percent(100.0),
                                                    height: Val::Percent(100.0),
                                                    border: UiRect {
                                                        right: BASE_SIZE,
                                                        bottom: BASE_SIZE,
                                                        ..default()
                                                    },
                                                    ..default()
                                                },
                                                BorderColor::from(colors.dark_border),
                                            ));
                                        });
                                    parent.spawn((
                                        Node {
                                            height: BASE_SIZE * 2.0,
                                            width: Val::Percent(100.0),
                                            ..default()
                                        },
                                        // The shadow is already colored in by the parent, the node
                                        // is just needed to take up space.
                                        // BackgroundColor::from(frame.colors.shadow),
                                    ));
                                })
                                .id();
                        });
                });

            parent.spawn((
                Node {
                    height: BASE_SIZE * 2.0,
                    width: Val::Percent(100.0),
                    ..default()
                },
                // The shadow is already colored in by the parent, the node is just needed to
                // take up space.
                // BackgroundColor::from(frame.colors.shadow),
            ));
        });

        entity_commands.insert(slider);

        entity_commands
    }

    fn spawn_dropdown<'a>(
        &'a mut self,
        choices: Vec<String>,
        selected: usize,
        style: DropdownStyle,
    ) -> EntityCommands<'a> {
        let mut frame = FrameColor::new(colors::FOOTER_LIGHT);

        let mut entity_commands = self.spawn((
            Node {
                width: style.width,
                height: style.height,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            FocusPolicy::Block,
            Interaction::default(),
            // XXX: https://github.com/DioxusLabs/taffy/issues/834
            // This camouflages the gap
            BackgroundColor::from(frame.colors.shadow),
        ));

        // frame.shadow = entity_commands.id();

        let dropdown_entity = entity_commands.id();
        let mut dropdown = Dropdown {
            menu_entity: Entity::PLACEHOLDER,
            current_choice_entity: Entity::PLACEHOLDER,
            choice_entities: Vec::new(),
            choices,
            selected,
        };

        entity_commands.with_children(|parent| {
            parent
                .spawn(Node {
                    height: Val::Percent(100.0),
                    width: Val::Percent(100.0),
                    // This is the box shadow / space for the dropdown choices
                    margin: UiRect::bottom(BASE_SIZE * 2.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|parent| {
                    // Borders
                    parent
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border: UiRect {
                                    left: BASE_SIZE,
                                    top: BASE_SIZE,
                                    ..default()
                                },
                                ..default()
                            },
                            BorderColor::from(frame.colors.light_border),
                        ))
                        .id();
                    parent
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border: UiRect {
                                    right: BASE_SIZE,
                                    bottom: BASE_SIZE,
                                    ..default()
                                },
                                ..default()
                            },
                            BorderColor::from(frame.colors.dark_border),
                        ))
                        .id();

                    // Content
                    parent
                        .spawn((
                            Node {
                                height: Val::Percent(100.0),
                                width: Val::Percent(100.0),
                                border: UiRect::all(BASE_SIZE),
                                overflow: Overflow::clip_x(),
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor::from(frame.colors.body),
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    right: Val::ZERO,
                                    height: Val::Percent(100.0),
                                    width: Val::Percent(60.0),
                                    ..default()
                                },
                                ImageNode {
                                    // TODO: https://github.com/bevyengine/bevy/issues/9213
                                    // Pre-mix until transparency is fixed I guess, doesn't really
                                    // matter.
                                    color: frame.colors.body.mix(&Color::WHITE, 0.07),
                                    image: BUTTON_TEXTURE,
                                    image_mode: NodeImageMode::Sliced(TextureSlicer {
                                        border: BorderRect {
                                            left: 7.0,
                                            right: 9.0,
                                            top: 2.0,
                                            bottom: 3.0,
                                        },
                                        // Removes some extra pixels that are displayed when stretched,
                                        // idk why. Probably has something to do with top and bottom
                                        // connecting, so it's not really a 9 slice but a 6 slice.
                                        sides_scale_mode: SliceScaleMode::Tile {
                                            stretch_value: 0.0,
                                        },
                                        // This makes the image scale to fit the container
                                        max_corner_scale: f32::MAX,
                                        ..default()
                                    }),
                                    ..default()
                                },
                            ));
                            dropdown.current_choice_entity = parent
                                .spawn_text(&dropdown.selected())
                                .insert(Node {
                                    margin: UiRect::left(Val::Percent(4.0)),
                                    ..default()
                                })
                                .id();
                            parent.spawn((
                                Node {
                                    width: BASE_SIZE * 7.0,
                                    height: BASE_SIZE * 6.0,
                                    margin: UiRect::right(Val::Percent(2.5)),
                                    ..default()
                                },
                                ImageNode {
                                    color: Color::WHITE,
                                    image: DROPDOWN_ARROW_TEXTURE,
                                    ..default()
                                },
                            ));
                        });
                });

            // Shadow / choices
            dropdown.menu_entity = parent
                .spawn((
                    GlobalZIndex(1),
                    Node {
                        display: Display::None,
                        position_type: PositionType::Absolute,
                        height: style.height * dropdown.choices.len() as f32,
                        width: Val::Percent(100.0),
                        top: Val::Percent(100.0),
                        // Overlap the shadow of the main entity.
                        // margin: UiRect::top(-BASE_SIZE * 2.0),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BackgroundColor::from(frame.colors.shadow),
                ))
                .with_children(|parent| {
                    parent
                        .spawn(Node {
                            height: Val::Percent(100.0),
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            // Shadow when the dropdown is open
                            margin: UiRect::bottom(BASE_SIZE * 2.0),
                            ..default()
                        })
                        .with_children(|parent| {
                            parent.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    border: UiRect {
                                        left: BASE_SIZE,
                                        top: BASE_SIZE,
                                        ..default()
                                    },
                                    ..default()
                                },
                                BorderColor::from(frame.colors.light_border),
                            ));
                            parent.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    border: UiRect {
                                        right: BASE_SIZE,
                                        bottom: BASE_SIZE,
                                        ..default()
                                    },
                                    ..default()
                                },
                                BorderColor::from(frame.colors.dark_border),
                            ));

                            for (i, choice) in dropdown.choices.iter().enumerate() {
                                let mut dropdown_choice = DropdownChoice {
                                    index: i,
                                    parent: dropdown_entity,
                                    texture: Entity::PLACEHOLDER,
                                };

                                let entity = parent
                                    .spawn((
                                        Node {
                                            height: Val::Percent(100.0),
                                            width: Val::Percent(100.0),
                                            border: UiRect::all(BASE_SIZE),
                                            overflow: Overflow::clip_x(),
                                            // padding: UiRect::left(Val::Percent(5.0))
                                            //     .with_right(Val::Percent(2.5)),
                                            justify_content: JustifyContent::SpaceBetween,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        },
                                        FocusPolicy::Block,
                                        Interaction::default(),
                                        if i == selected {
                                            BackgroundColor::from(frame.colors.body.darker(0.1))
                                        } else {
                                            BackgroundColor::from(frame.colors.body)
                                        },
                                    ))
                                    .with_children(|parent| {
                                        dropdown_choice.texture = parent
                                            .spawn((
                                                Node {
                                                    position_type: PositionType::Absolute,
                                                    right: Val::ZERO,
                                                    height: Val::Percent(100.0),
                                                    width: Val::Percent(60.0),
                                                    ..default()
                                                },
                                                ImageNode {
                                                    // TODO: https://github.com/bevyengine/bevy/issues/9213
                                                    // Pre-mix until transparency is fixed I guess,
                                                    // doesn't really matter.
                                                    color: frame
                                                        .colors
                                                        .body
                                                        .mix(&Color::WHITE, 0.07),
                                                    image: BUTTON_TEXTURE,
                                                    image_mode: NodeImageMode::Sliced(
                                                        TextureSlicer {
                                                            border: BorderRect {
                                                                left: 7.0,
                                                                right: 9.0,
                                                                top: 2.0,
                                                                bottom: 3.0,
                                                            },
                                                            // Removes some extra pixels that are
                                                            // displayed when stretched, idk why.
                                                            // Probably has something to do with
                                                            // top and bottom connecting, so it's
                                                            // not really a 9 slice but a 6 slice.
                                                            sides_scale_mode:
                                                                SliceScaleMode::Tile {
                                                                    stretch_value: 0.0,
                                                                },
                                                            // This makes the image scale to fit
                                                            // the container
                                                            max_corner_scale: f32::MAX,
                                                            ..default()
                                                        },
                                                    ),
                                                    ..default()
                                                },
                                            ))
                                            .id();
                                        parent.spawn_text(&choice).insert(Node {
                                            margin: UiRect::left(Val::Percent(4.0)),
                                            ..default()
                                        });
                                    })
                                    .insert(dropdown_choice)
                                    .id();

                                dropdown.choice_entities.push(entity);
                            }
                        });
                })
                .id();
        });

        entity_commands.insert(dropdown);
        entity_commands.insert(frame);

        return entity_commands;
    }

    fn spawn_text<'a>(&'a mut self, text: &str) -> EntityCommands<'a> {
        self.spawn((
            Text::new(text),
            TextFont {
                font_size: DEFAULT_FONT_SIZE,
                font: DEFAULT_FONT_HANDLE,
                ..default()
            },
            TextLayout::new_with_no_wrap(),
            TextColor(Color::WHITE),
            TextShadow {
                offset: Vec2::splat(DEFAULT_FONT_SIZE / 12.0),
                ..default()
            },
        ))
    }

    fn spawn_tab<'a>(&'a mut self, text: &str, width: Val, active: bool) -> EntityCommands<'a> {
        let mut entity_commands = if active {
            self.spawn((
                Button,
                Node {
                    width,
                    height: BASE_SIZE * 24.0,
                    margin: UiRect::top(BASE_SIZE * 3.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor::from(colors::TAB_ACTIVE),
                ImageNode {
                    image: TAB_TEXTURE,
                    color: colors::TAB_ACTIVE.mix(&Color::WHITE, 0.07),
                    image_mode: NodeImageMode::Sliced(TextureSlicer {
                        border: BorderRect {
                            left: 4.0,
                            right: 3.0,
                            top: 3.0,
                            bottom: 3.0,
                        },
                        // Removes some extra pixels that are displayed when stretched,
                        // idk why. Probably has something to do with top and bottom
                        // connecting, so it's not really a 9 slice but a 6 slice.
                        sides_scale_mode: SliceScaleMode::Tile { stretch_value: 0.0 },
                        // This makes the image scale to fit the container
                        max_corner_scale: f32::MAX,
                        ..default()
                    }),
                    ..default()
                },
            ))
        } else {
            self.spawn((
                Button,
                Node {
                    width,
                    height: BASE_SIZE * 24.0,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor::from(colors::TAB_INACTIVE),
                ImageNode {
                    image: TAB_TEXTURE,
                    color: colors::TAB_ACTIVE.mix(&Color::WHITE, 0.07),
                    image_mode: NodeImageMode::Sliced(TextureSlicer {
                        border: BorderRect {
                            left: 4.0,
                            right: 3.0,
                            top: 3.0,
                            bottom: 3.0,
                        },
                        // Removes some extra pixels that are displayed when stretched,
                        // idk why. Probably has something to do with top and bottom
                        // connecting, so it's not really a 9 slice but a 6 slice.
                        sides_scale_mode: SliceScaleMode::Tile { stretch_value: 0.0 },
                        // This makes the image scale to fit the container
                        max_corner_scale: f32::MAX,
                        ..default()
                    }),
                    ..default()
                },
            ))
        };

        entity_commands.with_children(|parent| {
            parent.spawn((
                Node {
                    bottom: Val::ZERO,
                    height: BASE_SIZE * 3.0,
                    width: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                BackgroundColor::from(colors::TAB_SECONDARY),
            ));
            parent.spawn_text(text);
        });

        return entity_commands;
    }

    fn spawn_header<'a>(&'a mut self) -> EntityCommands<'a> {
        let mut entity_commands = self.spawn(Node {
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            height: BASE_SIZE * 6.0,
            width: Val::Percent(100.0),
            ..default()
        });

        entity_commands.with_children(|parent| {
            parent.spawn((
                Node {
                    height: Val::Percent(50.0),
                    width: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor::from(Color::BLACK),
            ));
            parent.spawn((
                Node {
                    height: Val::Percent(50.0),
                    width: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor::from(colors::TAB_SECONDARY),
            ));
        });

        return entity_commands;
    }

    fn spawn_footer<'a>(&'a mut self) -> EntityCommands<'a> {
        let mut entity_commands = self.spawn(Node {
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            bottom: Val::Percent(0.0),
            height: BASE_SIZE * 7.0,
            width: Val::Percent(100.0),
            ..default()
        });

        entity_commands.with_children(|parent| {
            parent.spawn((
                Node {
                    height: BASE_SIZE * 3.0,
                    width: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor::from(Srgba::rgba_u8(109, 99, 89, 255)),
            ));
            parent.spawn((
                Node {
                    // One taller on purpose
                    height: BASE_SIZE * 4.0,
                    width: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor::from(Color::BLACK),
            ));
        });

        return entity_commands;
    }
}

fn toggle_switch(
    time: Res<Time>,
    mut switch_query: Query<(Ref<Interaction>, Mut<Switch>, &mut FrameColor)>,
    mut switch_button_query: Query<&mut Node>,
) {
    for (interaction, mut switch, mut frame_color) in switch_query.iter_mut() {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            switch.toggle();
        }

        // Bypass so we can listen for Changed<Switch>
        if let Some(transition) = &mut switch.bypass_change_detection().transition {
            *transition += time.delta_secs();
            let percent_completion = (*transition / 0.1).min(1.0);
            // Invert based on which way it is to be switched.
            let completion = if switch.on() {
                percent_completion
            } else {
                1.0 - percent_completion
            };

            frame_color.set(Color::srgb_u8(102, 97, 95).mix(&colors::BUTTON_GREEN, completion));

            if let Ok(mut node) = switch_button_query.get_mut(switch.switch) {
                node.margin = UiRect::left(Val::Percent(50.0) * completion);
            }

            if percent_completion == 1.0 {
                switch.transition = None;
            }
        }
    }
}

fn change_frame_color(
    frame_query: Query<&FrameColor, Changed<FrameColor>>,
    mut image_query: Query<&mut ImageNode>,
    mut border_query: Query<&mut BorderColor>,
    mut background_query: Query<&mut BackgroundColor>,
) {
    for frame in frame_query.iter() {
        if let Ok(mut light_border) = border_query.get_mut(frame.light_border) {
            *light_border = BorderColor::from(frame.colors.light_border);
        }

        if let Ok(mut dark_border) = border_query.get_mut(frame.dark_border) {
            *dark_border = BorderColor::from(frame.colors.dark_border);
        }

        if let Ok(mut body_color) = background_query.get_mut(frame.body) {
            *body_color = BackgroundColor::from(frame.colors.body);
        }

        if let Ok(mut shadow_color) = background_query.get_mut(frame.shadow) {
            *shadow_color = BackgroundColor::from(frame.colors.shadow);
        }

        if let Ok(mut image_node) = image_query.get_mut(frame.body) {
            image_node.color = frame.colors.body.mix(&Color::WHITE, 0.07);
        }
    }
}

fn set_slider_value(
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut sliders: Query<(Entity, Ref<Interaction>, &mut Slider)>,
    mut rails: Query<(&RelativeCursorPosition, &ComputedNode), With<SliderRail>>,
    mut slider_thumbs: Query<(&mut Node, &ComputedNode), With<SliderThumb>>,
    mut selected: Local<Option<Entity>>,
) {
    if mouse_button_input.just_released(MouseButton::Left) {
        selected.take();
    }

    for (slider_entity, interaction, _) in sliders.iter() {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            selected.insert(slider_entity);
        }
    }

    let Some(slider_entity) = *selected else {
        return;
    };

    let (_, _, mut slider) = sliders.get_mut(slider_entity).unwrap();
    let (rail_cursor_position, rail_computed_node) = rails.get(slider.rail).unwrap();
    let (mut thumb_node, thumb_computed_node) = slider_thumbs.get_mut(slider.thumb).unwrap();

    let Some(rail_cursor_position) = rail_cursor_position.normalized else {
        // Not part of control flow, docs say the cursor position can be unknown, no reason given.
        return;
    };

    let thumb_width = thumb_computed_node.size.x / rail_computed_node.size.x;

    let min = thumb_width / 2.0;
    let max = 1.0 - min;
    let new_value = ((rail_cursor_position.x - min) / (1.0 - thumb_width))
        .min(1.0)
        .max(0.0);
    // Change detection guard
    if slider.value != new_value {
        slider.value = new_value;
    }
}

// Moves the slider thumb in response to value changes.
fn move_slider_thumb(
    mut sliders: Query<&Slider, Changed<Slider>>,
    slider_rails: Query<&ComputedNode, With<SliderRail>>,
    mut slider_thumbs: Query<(&mut Node, &ComputedNode), With<SliderThumb>>,
) {
    for slider in sliders.iter() {
        let (mut thumb_node, thumb_computed_node) = slider_thumbs.get_mut(slider.thumb).unwrap();
        let rail_computed_node = slider_rails.get(slider.rail).unwrap();

        let value = (slider.segments() * slider.value).round() / slider.segments();
        let Val::Percent(thumb_width) = thumb_node.width else {
            return;
        };
        let cursor_position = value * (100.0 - thumb_width);

        if cursor_position.is_finite() {
            thumb_node.margin.left = Val::Percent(cursor_position);
        }
    }
}

fn dropdown_interaction(
    mut dropdowns: Query<(Ref<Interaction>, &mut Dropdown)>,
    mut dropdown_choices: Query<
        (&Interaction, &DropdownChoice, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut nodes: Query<&mut Node>,
    mut text: Query<&mut Text>,
    mut textures: Query<&mut ImageNode>,
) {
    for (interaction, dropdown) in dropdowns.iter() {
        if !interaction.is_changed() || *interaction != Interaction::Pressed {
            continue;
        }

        let mut menu_node = nodes.get_mut(dropdown.menu_entity).unwrap();
        if menu_node.display == Display::None {
            menu_node.display = Display::Flex;
        } else {
            menu_node.display = Display::None;
        }
    }

    for (interaction, choice, mut color) in dropdown_choices.iter_mut() {
        match interaction {
            Interaction::Pressed => {
                let (_, mut dropdown) = dropdowns.get_mut(choice.parent).unwrap();
                dropdown.selected = choice.index;

                let mut text = text.get_mut(dropdown.current_choice_entity).unwrap();
                text.0 = dropdown.choices[dropdown.selected].clone();

                let mut menu_node = nodes.get_mut(dropdown.menu_entity).unwrap();
                menu_node.display = Display::None;
            }
            Interaction::Hovered => {
                let colors = ColorSet::new(colors::FOOTER_LIGHT);
                color.0 = colors.body.darker(0.1);
                let mut image_node = textures.get_mut(choice.texture).unwrap();
                image_node.color = color.0.mix(&Color::WHITE, 0.07);
            }
            Interaction::None => {
                let colors = ColorSet::new(colors::FOOTER_LIGHT);

                let mut image_node = textures.get_mut(choice.texture).unwrap();
                let (_, dropdown) = dropdowns.get(choice.parent).unwrap();
                if dropdown.selected == choice.index {
                    color.0 = colors.body.darker(0.01);
                    image_node.color = color.0.mix(&Color::WHITE, 0.07);
                } else {
                    color.0 = colors.body;
                    image_node.color = color.0.mix(&Color::WHITE, 0.07);
                }
            }
        }
    }
}

#[derive(Component)]
pub struct ButtonSelection {
    entries: Vec<String>,
    selected: usize,
}

impl ButtonSelection {
    pub fn set_selected(&mut self, selected: usize) {
        self.selected = selected.min(self.entries.len());
    }

    pub fn selected(&self) -> &str {
        &self.entries[self.selected]
    }

    pub fn index(&self) -> usize {
        self.selected
    }
}

#[derive(Component)]
struct ButtonSelectionNode {
    index: usize,
}

#[derive(Component)]
struct SettingsSliderText {
    text_entity: Entity,
}

#[derive(Deserialize, Serialize)]
pub enum SettingsWidget {
    /// A slider with a range of values
    Slider {
        name: String,
        /// Explanation of what it configures
        description: Option<String>,
        /// Minimum value
        min: f32,
        /// Maximum value
        max: f32,
        /// Current value
        value: f32,
        /// Increment of the slider in decimal places. 0 will increment by integers, 1 will
        /// increment by 0.1, etc...
        decimals: usize,
        /// Unit displayed after the value in the slider text
        #[serde(default)]
        unit: String,
    },
    /// An on/off button
    Switch {
        name: String,
        /// Explanation of what it configures
        description: Option<String>,
        /// Starting value
        default_on: bool,
    },
    /// Text input
    TextBox {
        name: String,
        /// Pre-inserted text
        text: Option<String>,
        /// Explanation of what it configures
        description: Option<String>,
        /// Placeholder text displayed while there is no input
        placeholder: Option<String>,
    },
    /// A dropdown menu you can select an item from
    Dropdown {
        name: String,
        /// Explanation of what it configures
        description: Option<String>,
        /// The entries that can be selected between
        entries: Vec<String>,
        /// Which entry is currently selected
        selected: usize,
    },
    /// A list of entries arranged as a set of buttons that can be selected between
    ButtonSelection {
        name: String,
        /// Explanation of what it configures
        description: Option<String>,
        /// Which entry is currently selected
        selected: usize,
        /// The entries that can be chosen between
        entries: Vec<String>,
        /// Images displayed on the entry buttons
        #[serde(default)]
        images: Vec<String>,
        // TODO: repalce flex_direction
        // enum ButtonSelectionStyle {
        //     Compact, -- No image, tightly packed buttons
        //     Small, -- small text adjacent image
        //     Large, -- prominent picture above text
        // }
        /// Which side of the text the images should be displayed at
        #[serde(default)]
        flex_direction: FlexDirection,
    },
}

impl SettingsWidget {
    /// The name as it is expected by the server, as opposed to how it is represented in the ui.
    /// All lowercase, spaces replaced by underscores
    pub fn normalized_name(&self) -> String {
        let name = match self {
            Self::Slider { name, .. } => name,
            Self::Switch { name, .. } => name,
            Self::TextBox { name, .. } => name,
            Self::Dropdown { name, .. } => name,
            Self::ButtonSelection { name, .. } => name,
        };

        name.to_lowercase().replace(" ", "_")
    }

    pub fn spawn<'a>(
        self,
        parent: &'a mut ChildSpawnerCommands,
        asset_server: &AssetServer,
        asset_path: &str,
    ) -> EntityCommands<'a> {
        match self {
            Self::TextBox {
                description,
                text,
                placeholder,
                name,
            } => {
                let mut entity = Entity::PLACEHOLDER;
                parent
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        height: BASE_SIZE * 29.0,
                        row_gap: BASE_SIZE * 4.0,
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|parent| {
                        parent.spawn_text(&name);
                        entity = parent
                            .spawn_textbox(TextBox {
                                text: text.unwrap_or_default(),
                                placeholder_text: placeholder.unwrap_or_default(),
                                ..default()
                            })
                            .id();
                    });
                parent.commands_mut().entity(entity)
            }
            Self::ButtonSelection {
                name,
                description,
                selected,
                entries,
                images,
                flex_direction,
            } => {
                let mut entity = Entity::PLACEHOLDER;
                parent
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        height: match flex_direction {
                            FlexDirection::Row | FlexDirection::RowReverse => BASE_SIZE * 29.0,
                            FlexDirection::Column | FlexDirection::ColumnReverse => Val::Auto,
                        },
                        row_gap: BASE_SIZE * 4.0,
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|parent| {
                        parent.spawn_text(&name);
                        entity = parent
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                flex_grow: 1.0,
                                column_gap: BASE_SIZE * 4.0,
                                ..default()
                            })
                            .with_children(|parent| {
                                for (i, text) in entries.iter().enumerate() {
                                    parent
                                        .spawn_button(
                                            text,
                                            ButtonStyle {
                                                image: images.get(i).map(|path| {
                                                    asset_server
                                                        .load(Path::new(&asset_path).join(path))
                                                }),
                                                flex_direction,
                                                color: if i == selected {
                                                    colors::BUTTON_GREEN
                                                } else {
                                                    Color::srgb_u8(62, 57, 55)
                                                },
                                                ..default()
                                            },
                                        )
                                        .insert((ButtonSelectionNode { index: i },));
                                }
                            })
                            .insert(ButtonSelection { entries, selected })
                            .id();
                    });

                parent.commands_mut().entity(entity)
            }
            Self::Switch {
                name,
                description,
                default_on,
            } => {
                let mut entity = Entity::PLACEHOLDER;
                parent
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    })
                    .with_children(|parent| {
                        parent.spawn_text(&name);
                        entity = parent.spawn_switch(default_on).id();
                    });

                parent.commands_mut().entity(entity)
            }
            Self::Dropdown {
                name,
                description,
                entries,
                selected,
            } => {
                let mut entity = Entity::PLACEHOLDER;
                parent
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        height: BASE_SIZE * 17.0,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|parent| {
                        parent.spawn_text(&name);
                        entity = parent
                            .spawn_dropdown(
                                entries,
                                selected,
                                DropdownStyle {
                                    width: Val::Percent(48.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                            )
                            .id();
                    });

                parent.commands_mut().entity(entity)
            }
            Self::Slider {
                name,
                description,
                min,
                max,
                value,
                decimals,
                unit,
            } => {
                let mut entity = Entity::PLACEHOLDER;
                parent
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        height: BASE_SIZE * 17.0,
                        // justify_content: JustifyContent::Stretch,
                        align_items: AlignItems::Center,
                        column_gap: Val::Percent(2.0),
                        ..default()
                    })
                    .with_children(|parent| {
                        parent.spawn_text(&name).insert(Node {
                            flex_grow: 1.0,
                            ..default()
                        });
                        let text_entity = parent.spawn_text("").id();
                        entity = parent
                            .spawn_slider(SliderStyle {
                                width: Val::Percent(48.0),
                                height: Val::Percent(100.0),
                                value,
                                min,
                                max,
                                decimals,
                                unit,
                            })
                            .insert(SettingsSliderText { text_entity })
                            .id();
                    });

                parent.commands_mut().entity(entity)
            }
        }
    }
}

fn button_selection(
    mut selection_query: Query<(Mut<ButtonSelection>, &Children)>,
    mut selection_node_query: Query<(
        &ButtonSelectionNode,
        &mut FrameColor,
        Ref<Interaction>,
        &ChildOf,
    )>,
) {
    // Changed by the server
    for (selection, children) in selection_query.iter() {
        if !selection.is_changed() {
            continue;
        }

        for child in children {
            if let Ok((selection_node, mut frame_color, _, _)) =
                selection_node_query.get_mut(*child)
            {
                if selection_node.index == selection.selected {
                    frame_color.set(colors::BUTTON_GREEN);
                } else {
                    frame_color.set(Color::srgb_u8(62, 57, 55));
                }
            }
        }
    }

    // Button clicks
    let mut parent = None;
    for (_, _, interaction, child_of) in selection_node_query.iter() {
        if *interaction == Interaction::Pressed && interaction.is_changed() {
            parent = Some(child_of.parent());
            break;
        }
    }

    let Some(parent) = parent else {
        return;
    };

    let (mut selection, children) = selection_query.get_mut(parent).unwrap();

    for child in children {
        if let Ok((selection_node, mut frame_color, interaction, _)) =
            selection_node_query.get_mut(*child)
        {
            if *interaction == Interaction::Pressed {
                selection.selected = selection_node.index;
                frame_color.set(colors::BUTTON_GREEN);
            } else {
                frame_color.set(Color::srgb_u8(62, 57, 55));
            }
        }
    }
}

fn update_settings_slider_text(
    sliders: Query<(&Slider, &SettingsSliderText), Changed<Slider>>,
    mut text: Query<&mut Text>,
) {
    for (slider, slider_text) in sliders.iter() {
        let mut text = text.get_mut(slider_text.text_entity).unwrap();
        let decimals = slider.decimals;
        text.0 = format!("{:.decimals$}{}", slider.value(), &slider.unit);
    }
}

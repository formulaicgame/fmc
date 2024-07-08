use std::collections::HashMap;

use bevy::{
    prelude::*,
    ui::FocusPolicy,
    window::{CursorGrabMode, PrimaryWindow},
};

use crate::{game_state::GameState, networking::Identity};

mod login;
mod main_menu;
mod multiplayer;
mod pause_menu;

pub struct GuiPlugin;
impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<UiState>()
            .insert_resource(Interfaces::default())
            .add_plugins((
                login::LoginPlugin,
                main_menu::MainMenuPlugin,
                multiplayer::MultiPlayerPlugin,
                pause_menu::PauseMenuPlugin,
            ))
            .add_systems(Startup, setup)
            .add_systems(Update, change_interface.run_if(state_changed::<UiState>))
            .add_systems(Update, enter_exit_ui.run_if(state_changed::<GameState>));
    }
}

fn setup(mut commands: Commands, mut interfaces: ResMut<Interfaces>) {
    // TODO: Slight hack, maybe better to let the server define this
    // In-game cursor
    let entity = commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Px(3.0),
                height: Val::Px(3.0),
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                bottom: Val::Percent(50.0),
                ..default()
            },
            background_color: BackgroundColor(Color::rgba(0.9, 0.9, 0.9, 0.3)),
            ..Default::default()
        })
        .id();

    interfaces.insert(UiState::None, entity);
}

// TODO: Make sub states(https://github.com/bevyengine/bevy/issues/8187)
// of the main GameState?
#[derive(States, PartialEq, Eq, Debug, Clone, Hash, Default)]
enum UiState {
    #[default]
    None,
    Login,
    MainMenu,
    MultiPlayer,
    PauseMenu,
}

#[derive(Resource, Deref, DerefMut, Default)]
struct Interfaces(HashMap<UiState, Entity>);

#[derive(Component)]
struct InterfaceMarker;

#[derive(Bundle)]
struct InterfaceBundle {
    /// Describes the logical size of the node
    node: Node,
    /// Styles which control the layout (size and position) of the node and it's children
    /// In some cases these styles also affect how the node drawn/painted.
    style: Style,
    /// The background color, which serves as a "fill" for this node
    background_color: BackgroundColor,
    /// The color of the Node's border
    border_color: BorderColor,
    /// Whether this node should block interaction with lower nodes
    focus_policy: FocusPolicy,
    /// The transform of the node
    ///
    /// This field is automatically managed by the UI layout system.
    /// To alter the position of the `NodeBundle`, use the properties of the [`Style`] component.
    transform: Transform,
    /// The global transform of the node
    ///
    /// This field is automatically managed by the UI layout system.
    /// To alter the position of the `NodeBundle`, use the properties of the [`Style`] component.
    global_transform: GlobalTransform,
    /// Describes the visibility properties of the node
    visibility_bundle: VisibilityBundle,
    /// Indicates the depth at which the node should appear in the UI
    z_index: ZIndex,
    /// Marker for interfaces
    interface_marker: InterfaceMarker,
}

impl Default for InterfaceBundle {
    fn default() -> Self {
        InterfaceBundle {
            // Transparent background
            background_color: Color::NONE.into(),
            border_color: Color::NONE.into(),
            node: Default::default(),
            style: Default::default(),
            focus_policy: Default::default(),
            transform: Default::default(),
            global_transform: Default::default(),
            visibility_bundle: Default::default(),
            z_index: Default::default(),
            interface_marker: InterfaceMarker,
        }
    }
}

fn change_interface(
    state: Res<State<UiState>>,
    interfaces: Res<Interfaces>,
    mut interface_query: Query<(Entity, &mut Visibility), With<InterfaceMarker>>,
) {
    let new_interface_entity = interfaces.get(state.get());
    for (interface_entity, mut visibility) in interface_query.iter_mut() {
        if new_interface_entity.is_some() && *new_interface_entity.unwrap() == interface_entity {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

fn enter_exit_ui(
    game_state: Res<State<GameState>>,
    identity: Res<Identity>,
    mut ui_state: ResMut<NextState<UiState>>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
) {
    match game_state.get() {
        GameState::MainMenu => {
            if identity.username.is_empty() {
                ui_state.set(UiState::Login);
            } else {
                ui_state.set(UiState::MainMenu);
            }
        }
        GameState::Connecting => (),
        GameState::Playing => ui_state.set(UiState::None),
        GameState::Paused => ui_state.set(UiState::PauseMenu),
    }

    let mut window = window.single_mut();

    match game_state.get() {
        GameState::Playing => {
            window.cursor.grab_mode = if cfg!(unix) {
                CursorGrabMode::Locked
            } else {
                CursorGrabMode::Confined
            };
            window.cursor.visible = false;
        }
        _ => {
            window.cursor.grab_mode = CursorGrabMode::None;
            window.cursor.visible = true;
            let position = Vec2::new(window.width() / 2.0, window.height() / 2.0);
            window.set_cursor_position(Some(position));
        }
    }
}

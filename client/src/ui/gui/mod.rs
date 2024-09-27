use std::collections::HashMap;

use bevy::{asset::embedded_asset, prelude::*, ui::FocusPolicy};

mod connecting;
mod login;
mod main_menu;
mod multiplayer;
mod pause_menu;

pub struct GuiPlugin;
impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GuiState>()
            .insert_resource(Interfaces::default())
            .add_plugins((
                login::LoginPlugin,
                main_menu::MainMenuPlugin,
                connecting::ConnectingPlugin,
                pause_menu::PauseMenuPlugin,
            ))
            .add_systems(Startup, setup)
            .add_systems(Update, change_interface.run_if(state_changed::<GuiState>));

        embedded_asset!(app, "assets/background.png");
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

    interfaces.insert(GuiState::None, entity);
}

// Decides which gui interface is shown
#[derive(States, PartialEq, Eq, Debug, Clone, Hash, Default)]
pub(super) enum GuiState {
    None,
    Login,
    #[default]
    MainMenu,
    Connecting,
    PauseMenu,
}

// To link the GuiState to the entity holding the layout it must be registered here.
#[derive(Resource, Deref, DerefMut, Default)]
struct Interfaces(HashMap<GuiState, Entity>);

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
    state: Res<State<GuiState>>,
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

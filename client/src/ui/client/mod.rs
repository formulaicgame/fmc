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
    // In-game cursor
    let entity = commands
        .spawn((
            Node {
                width: Val::Px(3.0),
                height: Val::Px(3.0),
                position_type: PositionType::Absolute,
                margin: UiRect::all(Val::Auto),
                ..Default::default()
            },
            BackgroundColor(Color::srgba(0.9, 0.9, 0.9, 0.3)),
        ))
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
#[require(Node)]
struct Interface;

fn change_interface(
    state: Res<State<GuiState>>,
    interfaces: Res<Interfaces>,
    mut interface_query: Query<(Entity, &mut Visibility), With<Interface>>,
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

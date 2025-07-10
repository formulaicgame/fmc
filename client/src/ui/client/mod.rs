use std::collections::HashMap;

use bevy::{
    asset::{load_internal_binary_asset, weak_handle, RenderAssetUsages},
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
};

use crate::game_state::GameState;

// The interfaces
mod connecting;
mod login;
mod main_menu;
mod pause;
mod world_configuration;

// Widgets used by the interfaces
mod widgets;

// TODO: This should be moved into the ui module and be changed to val::px(4) to replace the
// current default ui scale of 4. Then use this const for the server interfaces too.
const BASE_SIZE: Val = Val::Px(1.0);

// Background used for all interfaces
const BACKGROUND: Handle<Image> = weak_handle!("65ff3831-5ee8-4815-b32b-95ea615b2248");

pub struct GuiPlugin;
impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GuiState>()
            .insert_resource(Interfaces::default())
            .add_plugins((
                widgets::WidgetPlugin,
                login::LoginPlugin,
                main_menu::MainMenuPlugin,
                connecting::ConnectingPlugin,
                pause::PausePlugin,
                world_configuration::WorldConfigurationPlugin,
            ))
            .add_systems(Startup, setup)
            // .add_systems(OnEnter(GameState::Launcher), go_to_main_menu_on_quit)
            .add_systems(Update, change_interface.run_if(state_changed::<GuiState>));

        load_internal_binary_asset!(
            app,
            BACKGROUND,
            "../../../assets/ui/background.png",
            |bytes: &[u8], _path: String| -> Image {
                Image::from_buffer(
                    bytes,
                    ImageType::Format(ImageFormat::Png),
                    CompressedImageFormats::NONE,
                    true,
                    ImageSampler::nearest(),
                    RenderAssetUsages::RENDER_WORLD,
                )
                .expect("Failed to load image")
            }
        );
    }
}

fn setup(mut commands: Commands, mut interfaces: ResMut<Interfaces>) {
    // In-game cursor
    let entity = commands
        .spawn((
            Interface,
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

#[derive(Resource, Deref, DerefMut, Default)]
struct Interfaces(HashMap<GuiState, Entity>);

// Decides which gui interface is active. [Interfaces] maps each variant to an entity
// containing the interface layout that will be switched to when the state changes.
#[derive(States, PartialEq, Eq, Debug, Clone, Hash, Default)]
pub enum GuiState {
    // Gui shown when in game, used for the cursor.
    None,
    // Let's you log into a user account
    Login,
    #[default]
    MainMenu,
    // TODO: Rename to "Status"
    // Interface shown while connecting, but is used to show all types of arbitrary messages to the
    // player. Debug info, disconnection errors...
    Connecting,
    // Shown when paused in game
    PauseMenu,
    // Interface for editing the settings of a world, or create a new world if it doesn't exist.
    WorldConfiguration,
}

// Marker struct that is added to the top level entity of each interface.
#[derive(Component)]
#[require(Node)]
struct Interface;

fn change_interface(
    state: Res<State<GuiState>>,
    interfaces: Res<Interfaces>,
    mut interface_query: Query<(Entity, &mut Node), With<Interface>>,
) {
    let new_interface_entity = interfaces.get(state.get()).unwrap();

    for (interface_entity, mut node) in interface_query.iter_mut() {
        if *new_interface_entity == interface_entity {
            node.display = Display::Flex;
        } else {
            node.display = Display::None;
        }
    }
}

fn go_to_main_menu_on_quit(mut gui_state: ResMut<NextState<GuiState>>) {
    gui_state.set(GuiState::MainMenu);
}

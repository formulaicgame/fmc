use bevy::{
    asset::load_internal_binary_asset,
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow, WindowResized},
    winit::WinitWindows,
};

// The ui module handles two different ui systems. The 'server' system which handles in-game ui
// sent by the server that's constructed at runtime, and the 'gui' system which handles 'client' ui
// e.g. the main menu, the server list and the pause menu.

// TODO: This should not be part of the ui module, remnant from not wanting to expose the server module.
mod hand;

mod client;
pub mod server;
// Common widgets used by both ui systems.
mod widgets;

pub const DEFAULT_FONT_HANDLE: Handle<Font> = Handle::weak_from_u128(1491772431825224041);
const UI_SCALE: f32 = 4.0;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            widgets::WidgetPlugin,
            client::GuiPlugin,
            hand::HandPlugin,
            server::ServerInterfacesPlugin,
        ))
        .add_systems(Startup, scaling_setup)
        .add_systems(
            Update,
            (
                scale_ui.run_if(on_event::<WindowResized>),
                change_ui_state.run_if(state_changed::<client::GuiState>),
                cursor_visibiltiy.run_if(resource_changed::<CursorVisibility>),
            ),
        );

        app.init_state::<UiState>()
            .insert_resource(CursorVisibility::default());

        // TODO: It would be nice to overwrite bevy's default handle instead, so it never has to be
        // specified by an entity, but doing it increases compile time by a lot. Maybe because it
        // reaches into the bevy crates.
        load_internal_binary_asset!(
            app,
            DEFAULT_FONT_HANDLE,
            "../../assets/ui/font.otf",
            |bytes: &[u8], _path: String| { Font::try_from_bytes(bytes.to_vec()).unwrap() }
        );
    }
}

#[derive(States, PartialEq, Eq, Debug, Clone, Hash, Default)]
enum UiState {
    #[default]
    // The client gui is shown
    Gui,
    // The interfaces provided by the server are shown
    ServerInterfaces,
}

#[derive(Resource)]
struct LogicalMonitorWidth {
    width: f32,
}

fn scaling_setup(
    mut commands: Commands,
    winit_windows: NonSend<WinitWindows>,
    windows: Query<Entity, With<Window>>,
) {
    let entity = windows.single();
    let id = winit_windows.entity_to_winit.get(&entity).unwrap();
    let monitor = winit_windows.windows.get(id).unwrap();
    let monitor = monitor.available_monitors().next().unwrap();
    let resolution = monitor.size().to_logical(monitor.scale_factor());
    commands.insert_resource(LogicalMonitorWidth {
        width: resolution.width,
    });
}

// TODO: Scaling like this uses a lot of memory because of how font sizes are stored.
// https://github.com/bevyengine/bevy/issues/5636
// It was fixed, but then reversed. Haven't found anyone discussing it afterwards.
fn scale_ui(
    mut ui_scale: ResMut<UiScale>,
    resolution: Res<LogicalMonitorWidth>,
    window: Query<&Window>,
) {
    let window = window.single();
    let scale = window.resolution.width() / resolution.width;
    ui_scale.0 = UI_SCALE * scale;
}

fn change_ui_state(
    mut ui_state: ResMut<NextState<UiState>>,
    gui_state: Res<State<client::GuiState>>,
    mut cursor_visibility: ResMut<CursorVisibility>,
) {
    if *gui_state.get() == client::GuiState::None {
        cursor_visibility.gui = false;
        ui_state.set(UiState::ServerInterfaces);
    } else {
        cursor_visibility.gui = true;
        ui_state.set(UiState::Gui);
    }
}

#[derive(Resource, Default)]
struct CursorVisibility {
    gui: bool,
    server: bool,
}

fn cursor_visibiltiy(
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    cursor_visibility: Res<CursorVisibility>,
) {
    let should_be_visible = cursor_visibility.gui || cursor_visibility.server;
    let mut window = window.single_mut();

    if should_be_visible && !window.cursor_options.visible {
        window.cursor_options.visible = true;
        let position = Vec2::new(window.width() / 2.0, window.height() / 2.0);
        window.set_cursor_position(Some(position));
        window.cursor_options.grab_mode = CursorGrabMode::None;
    } else if !should_be_visible && window.cursor_options.visible {
        window.cursor_options.visible = false;
        window.cursor_options.grab_mode = if cfg!(unix) {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::Confined
        };
    }
}

use bevy::{
    asset::{load_internal_binary_asset, weak_handle},
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow, WindowResized},
    winit::WinitWindows,
};

use crate::settings::Settings;

// TODO: This should not be part of the ui module, remnant from not wanting to expose the server module.
mod hand;

// The client gui
mod client;
// Ui constructed from server assets
pub mod server;
pub mod text_input;

pub const DEFAULT_FONT_HANDLE: Handle<Font> = weak_handle!("2b53f27a-6c3b-4e46-b83c-048d60c035a7");
pub const DEFAULT_FONT_SIZE: f32 = 7.0;
const DOUBLE_CLICK_DELAY: f32 = 0.4;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            text_input::TextInputPlugin,
            client::GuiPlugin,
            hand::HandPlugin,
            server::ServerInterfacesPlugin,
        ))
        .add_systems(Startup, scaling_setup)
        .add_systems(
            Update,
            (
                scale_ui.run_if(on_event::<WindowResized>.or(resource_changed::<Scale>)),
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
struct Scale {
    base_scale: f32,
    variable_scale: f32,
    resolution_width: f32,
    resolution_height: f32,
}

impl Scale {
    fn scale(&self) -> f32 {
        self.base_scale * self.variable_scale
    }

    fn set_scale(&mut self, scale: f32) {
        self.variable_scale = scale;
    }
}

fn scaling_setup(
    mut commands: Commands,
    settings: Res<Settings>,
    winit_windows: NonSend<WinitWindows>,
    windows: Query<Entity, With<Window>>,
) {
    let entity = windows.single().unwrap();
    let id = winit_windows.entity_to_winit.get(&entity).unwrap();
    let monitor = winit_windows.windows.get(id).unwrap();
    let monitor = monitor.available_monitors().next().unwrap();
    let resolution = monitor.size().to_logical(monitor.scale_factor());
    commands.insert_resource(Scale {
        base_scale: 4.0 * resolution.width / 1920.0,
        variable_scale: 1.0,
        resolution_width: resolution.width,
        resolution_height: resolution.height,
    });
}

// TODO: Scaling like this uses a lot of memory because of how font sizes are stored.
// https://github.com/bevyengine/bevy/issues/5636
// It was fixed, but then reversed. Haven't found anyone discussing it afterwards.
fn scale_ui(mut bevy_ui_scale: ResMut<UiScale>, ui_scale: Res<Scale>, window: Query<&Window>) {
    let window = window.single().unwrap();
    let width = window.resolution.width() / ui_scale.resolution_width;
    let height = window.resolution.height() / ui_scale.resolution_height;
    let gap = (width - height).abs();
    // Weighs the scale more towards the minimum than taking the average would.
    // e.g a gap of 0.7 with width=0.3,height=1.0 gives scale=0.5 instead of 0.65(average)
    // Let's you distort the dimensions while still retaining a somewhat legible layout.
    let scale = width.max(height) * (-gap).exp();
    // let scale = width.min(height);
    bevy_ui_scale.0 = ui_scale.scale() * scale;
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
    let mut window = window.single_mut().unwrap();

    if should_be_visible && !window.cursor_options.visible {
        window.cursor_options.visible = true;
        window.cursor_options.grab_mode = CursorGrabMode::None;
        let position = Vec2::new(window.width() / 2.0, window.height() / 2.0);
        window.set_cursor_position(Some(position));
    } else if !should_be_visible && window.cursor_options.visible {
        window.cursor_options.visible = false;
        window.cursor_options.grab_mode = if cfg!(unix) {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::Confined
        };
    }
}

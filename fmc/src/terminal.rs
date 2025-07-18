use bevy::prelude::*;
use crossbeam::{Receiver, Sender};
use std::{
    io::{IsTerminal, Read},
    sync::LazyLock,
};

static CLI: LazyLock<Cli> = LazyLock::new(|| {
    let mut cli = Cli {
        world_path: None,
        extract_assets: false,
    };

    let mut args = std::env::args();
    // Skip the program name
    args.next();

    for (i, arg) in args.enumerate() {
        if i == 0 {
            if !arg.starts_with("-") {
                cli.world_path = Some(arg);
                continue;
            }
        }

        if arg == "--extract-assets" {
            cli.extract_assets = true;
        }
    }

    cli
});

/// The base cli arguments. These MUST be handled to enabled execution through the client.
pub struct Cli {
    world_path: Option<String>,
    extract_assets: bool,
}

impl Cli {
    // TODO: This is currently alway the first argument. For ergonomics this should really be the
    // last argument, but there's no way to parse it without knowing the structure of the entire
    // cli, and server implementers might want to add their own arguments.
    //
    /// Dictates the world path if Some, should not be overridden
    pub fn world_path() -> Option<&'static String> {
        CLI.world_path.as_ref()
    }

    /// If true, extract the assets folder and immediately terminate
    pub fn extract_assets() -> bool {
        CLI.extract_assets
    }
}

// TODO: Implement simple readline style tui.
pub struct TuiPlugin;
impl Plugin for TuiPlugin {
    fn build(&self, app: &mut App) {
        // Not needed yet when it's a terminal. Only for interaction locally with the client.
        if std::io::stdin().is_terminal() {
            return;
        }

        let (sender, receiver) = crossbeam::unbounded();

        // TODO: I don't know if this is the best way to do it. I'm under the impression that the
        // OS scheduler will take care of sleeping it.
        std::thread::spawn(move || loop {
            let mut buffer = String::new();
            if std::io::stdin().read_line(&mut buffer).is_err() {
                continue;
            };
            sender.send(buffer).unwrap();
        });

        app.insert_resource(Terminal { input: receiver })
            .add_systems(Update, read_input);
    }
}

#[derive(Resource)]
struct Terminal {
    input: Receiver<String>,
}

fn read_input(mut terminal: ResMut<Terminal>, mut app_exit: EventWriter<AppExit>) {
    if let Ok(input) = terminal.input.try_recv() {
        if input.trim() == "stop" {
            app_exit.write(AppExit::Success);
        }
    }
}

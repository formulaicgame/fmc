use bevy::prelude::*;
use crossbeam::{Receiver, Sender};
use std::io::{IsTerminal, Read};

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
        std::thread::spawn(move || {
            loop {
                let mut buffer = String::new();
                if std::io::stdin().read_line(&mut buffer).is_err() {
                    continue;
                };
                sender.send(buffer).unwrap();
            }
        });

        app.insert_resource(Terminal { input: receiver })
            .add_systems(Update, read_input);
    }
}

#[derive(Resource)]
struct Terminal {
    input: Receiver<String>,
}

fn read_input(mut terminal: ResMut<Terminal>, mut app_exit: MessageWriter<AppExit>) {
    if let Ok(input) = terminal.input.try_recv() {
        if input.trim() == "stop" {
            app_exit.write(AppExit::Success);
        }
    }
}

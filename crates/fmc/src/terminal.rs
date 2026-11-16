use bevy::prelude::*;
use crossbeam::Receiver;

use crate::commands::{Command, CommandError, CommandEvent, CommandRegistry, CommandSystems};
use crate::permissions::Roles;

pub struct TerminalPlugin;
impl Plugin for TerminalPlugin {
    fn build(&self, app: &mut App) {
        let (sender, receiver) = crossbeam::unbounded();

        std::thread::spawn(move || {
            for line in std::io::stdin().lines() {
                let Ok(line) = line else {
                    break;
                };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });

        let entity = app.world_mut().spawn(Roles::admin()).id();

        app.insert_resource(Terminal {
            input: receiver,
            entity,
        })
        .init_resource::<TerminalCommandRegistry>()
        .add_terminal_command::<HelpCommand>()
        .add_terminal_command::<StopCommand>()
        .add_systems(Startup, check_for_command_name_conflicts)
        .add_systems(
            Update,
            (
                read_input.before(CommandSystems::Dispatch),
                (handle_help_command, handle_stop_command).after(read_input),
            ),
        )
        .add_systems(PostUpdate, print_errors);
    }
}

#[derive(Resource)]
struct Terminal {
    input: Receiver<String>,
    entity: Entity,
}

// The terminal gets an exclusive set of commands that can't be reached as a player. Allows the
// addition of dangerous commands for mutation of the file system etc.
#[derive(Resource, Default, Deref, DerefMut)]
struct TerminalCommandRegistry(CommandRegistry);

/// Add a server command that can only be executed through a terminal
pub trait AddTerminalCommand {
    fn add_terminal_command<C: Command>(&mut self) -> &mut Self;
}

impl AddTerminalCommand for App {
    fn add_terminal_command<C: Command>(&mut self) -> &mut Self {
        self.add_message::<C>();
        self.world_mut()
            .get_resource_or_init::<TerminalCommandRegistry>()
            .insert::<C>();
        self
    }
}

fn check_for_command_name_conflicts(
    terminal_registry: Res<TerminalCommandRegistry>,
    registry: Res<CommandRegistry>,
) {
    for name in terminal_registry.names() {
        assert!(
            !registry.contains(name),
            "The command '{name}' is registered as both a terminal command and a regular command"
        );
    }
}

fn read_input(
    mut commands: Commands,
    terminal: Res<Terminal>,
    command_registry: Res<TerminalCommandRegistry>,
    mut command_events: MessageWriter<CommandEvent>,
) {
    while let Ok(text) = terminal.input.try_recv() {
        let words: Vec<&str> = text.split_whitespace().collect();
        let Some((&name, args)) = words.split_first() else {
            continue;
        };

        let Some(command) = command_registry.get(name) else {
            command_events.write(CommandEvent {
                source: terminal.entity,
                text: text.clone(),
            });
            continue;
        };

        if let Err(error) = command.run(&mut commands, terminal.entity, args) {
            error!("{error}");
        }
    }
}

fn print_errors(terminal: Res<Terminal>, mut errors: MessageReader<CommandError>) {
    for error in errors.read() {
        if error.source == terminal.entity {
            error!("{}", error.text);
        }
    }
}

#[derive(Message)]
struct HelpCommand {
    pub query: Option<String>,
}

impl Command for HelpCommand {
    const NAME: &'static str = "help";
    const USAGE: &'static str = "help [command] - List the available commands and their usage";

    fn parse(_source: Entity, args: &[&str]) -> Result<Self, String> {
        match args {
            [] => Ok(Self { query: None }),
            [query] => Ok(Self {
                query: Some(query.to_string()),
            }),
            _ => Err(Self::USAGE.to_owned()),
        }
    }
}

fn handle_help_command(
    terminal_registry: Res<TerminalCommandRegistry>,
    registry: Res<CommandRegistry>,
    mut help_commands: MessageReader<HelpCommand>,
) {
    for help_command in help_commands.read() {
        let query = help_command.query.as_deref().unwrap_or_default();

        let mut found = false;
        for command in terminal_registry.query(query).chain(registry.query(query)) {
            if !found {
                println!("\nCommands:");
                found = true;
            }
            println!("  {}", command.usage);
        }

        if found {
            println!();
        } else {
            error!("No commands start with '{query}'");
        }
    }
}

#[derive(Message)]
struct StopCommand;

impl Command for StopCommand {
    const NAME: &'static str = "stop";
    const USAGE: &'static str = "stop - Shut down the server";

    fn parse(_source: Entity, args: &[&str]) -> Result<Self, String> {
        match args {
            [] => Ok(Self),
            _ => Err(Self::USAGE.to_owned()),
        }
    }
}

fn handle_stop_command(
    mut stop_commands: MessageReader<StopCommand>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if stop_commands.read().count() > 0 {
        app_exit.write(AppExit::Success);
    }
}

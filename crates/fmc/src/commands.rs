use std::{collections::BTreeMap, ops::Bound};

use bevy::prelude::*;

use crate::permissions::{Permission, Permissions, Roles};

pub struct CommandPlugin;
impl Plugin for CommandPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandRegistry>()
            .add_message::<CommandEvent>()
            .add_message::<CommandError>()
            .add_systems(Update, dispatch_commands.in_set(CommandSystems::Dispatch));
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CommandSystems {
    Dispatch,
}

/// Send to execute a server command
#[derive(Message, Clone, Debug)]
pub struct CommandEvent {
    pub source: Entity,
    pub text: String,
}

/// Response to a server command failing to execute
#[derive(Message, Clone, Debug)]
pub struct CommandError {
    pub source: Entity,
    pub text: String,
}

/// All server commands must implement this trait. To enable it, it must be registered through [`AddCommand`]
pub trait Command: Message + Sized {
    /// Name used to invoke the command
    const NAME: &'static str;
    /// A usage string for help generation
    const USAGE: &'static str;
    /// The permission required to execute this command, if any.
    /// Make sure to register the necessary permission in [`Permissions`] before registering the
    /// command, or it will panic at startup!
    const PERMISSION: Option<Permission> = None;

    fn parse(source: Entity, args: &[&str]) -> Result<Self, String>;
}

/// Representation of a command when iterating over commands in the [`CommandRegistry`]
pub struct RegisteredCommand {
    pub name: &'static str,
    pub usage: &'static str,
    pub permission: Option<Permission>,
    dispatch: fn(&mut Commands, Entity, &[&str]) -> Result<(), String>,
}

impl RegisteredCommand {
    pub fn run(
        &self,
        commands: &mut Commands,
        source: Entity,
        args: &[&str],
    ) -> Result<(), String> {
        (self.dispatch)(commands, source, args)
    }
}

/// Contains all server commands that have been registered through [`AddCommand`]
#[derive(Resource, Default)]
pub struct CommandRegistry {
    commands: BTreeMap<&'static str, RegisteredCommand>,
}

impl CommandRegistry {
    pub(crate) fn insert<C: Command>(&mut self) {
        let previous = self.commands.insert(
            C::NAME,
            RegisteredCommand {
                name: C::NAME,
                usage: C::USAGE,
                permission: C::PERMISSION,
                dispatch: dispatch::<C>,
            },
        );

        assert!(
            previous.is_none(),
            "The command '{}' was registered twice",
            C::NAME
        );
    }

    pub fn get(&self, name: &str) -> Option<&RegisteredCommand> {
        self.commands.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> {
        self.commands.keys().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = &RegisteredCommand> {
        self.commands.values()
    }

    /// Returns an iterator over all commands with names that start with the given prefix
    pub fn query<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a RegisteredCommand> {
        self.commands
            .range::<str, _>((Bound::Included(prefix), Bound::Unbounded))
            .take_while(move |(name, _)| name.starts_with(prefix))
            .map(|(_, command)| command)
    }
}

fn dispatch<C: Command>(
    commands: &mut Commands,
    source: Entity,
    args: &[&str],
) -> Result<(), String> {
    commands.write_message(C::parse(source, args)?);
    Ok(())
}

/// Provides an extension method on [`App`] to register server commands
pub trait AddCommand {
    fn add_command<C: Command>(&mut self) -> &mut Self;
}

impl AddCommand for App {
    fn add_command<C: Command>(&mut self) -> &mut Self {
        if let Some(permission) = C::PERMISSION {
            assert!(
                self.world().resource::<Permissions>().contains(&permission),
                "The command '{}' requires the permission '{}', but no such permission exists.",
                C::NAME,
                permission.name()
            );
        }

        self.add_message::<C>();
        self.world_mut()
            .get_resource_or_init::<CommandRegistry>()
            .insert::<C>();
        self
    }
}

fn dispatch_commands(
    mut commands: Commands,
    command_registry: Res<CommandRegistry>,
    permissions: Res<Permissions>,
    roles_query: Query<&Roles>,
    mut command_events: MessageReader<CommandEvent>,
    mut errors: MessageWriter<CommandError>,
) {
    for command_event in command_events.read() {
        let source = command_event.source;
        let words: Vec<&str> = command_event.text.split_whitespace().collect();
        let Some((&name, args)) = words.split_first() else {
            continue;
        };

        let Some(command) = command_registry.get(name) else {
            errors.write(CommandError {
                source,
                text: format!("Unknown command '{name}'"),
            });
            continue;
        };

        let mut allowed = false;
        if let Some(permission) = command.permission {
            if let Ok(roles) = roles_query.get(source) {
                allowed = permissions.has_permission(roles, &permission);
            }
        } else {
            allowed = true;
        }

        if !allowed {
            errors.write(CommandError {
                source,
                text: format!("Unknown command '{name}'"),
            });
            continue;
        }

        if let Err(text) = command.run(&mut commands, source, args) {
            errors.write(CommandError { source, text });
        }
    }
}

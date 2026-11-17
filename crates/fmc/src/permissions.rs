use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
};

use crate::bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{database::Database, players::Player};

pub struct PermissionsPlugin;
impl Plugin for PermissionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Permissions>()
            .add_systems(PreUpdate, load_roles)
            .add_systems(PostUpdate, save_roles);
    }
}

// TODO: I have some idea of doing an interning style permission setup. You register each permission
// to some global, Permission::new can be checked directly from it so you can never create an
// invalid one. The Command trait would have to have required_permission method. Then maybe
// arc-mutex roles and share them around and then we don't need to keep 'roles' so close either.
// Very non-idiomatic but seems like it provides some better patterns maybe.
#[derive(Resource)]
pub struct Permissions {
    permissions: HashSet<Permission>,
    // We keep the roles here because you always need access to the registered permissions when
    // working with them. "Permissions" is an unfortunate name for the collection though.
    roles: HashMap<String, HashSet<Permission>>,
}

impl Default for Permissions {
    fn default() -> Self {
        let mut roles = HashMap::new();
        roles.insert(
            Roles::ADMIN.to_owned(),
            HashSet::from([Permission::WILDCARD]),
        );

        Self {
            permissions: HashSet::new(),
            roles,
        }
    }
}

impl Permissions {
    pub fn add(&mut self, permission: Permission) {
        assert!(
            permission != Permission::WILDCARD,
            "The permission name '{}' is reserved",
            permission.name()
        );
        self.permissions.insert(permission);
    }

    pub fn get(&self, name: &str) -> Option<Permission> {
        self.permissions.get(name).copied()
    }

    pub fn contains(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    pub fn add_role(&mut self, role: &str, permissions: &[Permission]) -> Result<(), String> {
        if role == Roles::ADMIN {
            return Err(format!("The role '{role}' cannot be modified"));
        }

        for permission in permissions {
            if !self.permissions.contains(permission) {
                return Err(format!(
                    "There is no permission named '{}'",
                    permission.name()
                ));
            }
        }

        self.roles
            .entry(role.to_owned())
            .or_default()
            .extend(permissions.iter().copied());

        Ok(())
    }

    pub fn has_permission(&self, roles: &Roles, permission: &Permission) -> bool {
        for role in roles.iter() {
            let Some(permissions) = self.roles.get(role) else {
                continue;
            };

            if permissions.contains(&Permission::WILDCARD) || permissions.contains(permission) {
                return true;
            }
        }

        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Permission(&'static str);

impl Permission {
    const WILDCARD: Permission = Permission("*");

    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    pub const fn name(&self) -> &'static str {
        self.0
    }
}

impl Borrow<str> for Permission {
    fn borrow(&self) -> &str {
        self.0
    }
}

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Roles(HashSet<String>);

impl Roles {
    const ADMIN: &str = "admin";

    pub fn admin() -> Self {
        Self(HashSet::from([Self::ADMIN.to_owned()]))
    }

    pub fn insert(&mut self, role: impl Into<String>) -> bool {
        self.0.insert(role.into())
    }

    pub fn remove(&mut self, role: &str) -> bool {
        self.0.remove(role)
    }

    pub fn contains(&self, role: &str) -> bool {
        self.0.contains(role)
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

fn save_roles(
    database: Res<Database>,
    changed_roles: Query<(&Player, Ref<Roles>), Changed<Roles>>,
) {
    for (player, roles) in changed_roles.iter() {
        if roles.is_added() {
            continue;
        }

        let json = serde_json::to_string(&*roles).expect("Failed to serialize player roles");
        let conn = database.get_write_connection();
        conn.execute(
            "INSERT OR REPLACE INTO roles VALUES (?,?)",
            [player.username.as_str(), json.as_str()],
        )
        .expect("Failed to save player roles to the database");
    }
}

fn load_roles(
    mut commands: Commands,
    database: Res<Database>,
    new_players: Query<(Entity, &Player), Added<Player>>,
) {
    if new_players.is_empty() {
        return;
    }

    let conn = database.get_read_connection();

    for (entity, player) in new_players.iter() {
        let result = conn.query_row(
            "SELECT roles FROM roles WHERE name = ?",
            [&player.username],
            |row| row.get::<_, String>(0),
        );

        let roles = match result {
            Ok(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
                error!("Malformed roles for player '{}': {e}", player.username);
                Roles::default()
            }),
            Err(rusqlite::Error::QueryReturnedNoRows) => Roles::default(),
            Err(e) => {
                error!("Failed to load roles for player '{}': {e}", player.username);
                Roles::default()
            }
        };

        commands.entity(entity).insert(roles);
    }
}

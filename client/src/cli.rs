use std::io;

use clap::Parser;

use crate::modding::server_builder::{Mod, ServerBuildConfig};

pub fn parse() -> bool {
    let cli = Cli::parse();

    if let Some(sub_command) = cli.sub_command {
        match sub_command {
            SubCommands::Build { template, path } => {
                if template {
                    create_server_template(&path);
                } else {
                    let build_config: ServerBuildConfig = match parse_server_build_config(&path) {
                        Ok(s) => s,
                        Err(e) => {
                            println!("Encountered error while reading server configuration: {e}");
                            return true;
                        }
                    };

                    build_config.build();
                }
            }
        }

        return true;
    } else {
        return false;
    }
}
#[derive(clap::Parser)]
pub struct Cli {
    #[command(subcommand)]
    sub_command: Option<SubCommands>,
}

#[derive(clap::Subcommand)]
enum SubCommands {
    #[command(about = "Build a server using a server configuration file")]
    Build {
        #[arg(long, help = "Create a server configuration template")]
        template: bool,
        #[arg(default_value = "server.conf", help = "Path to server configuration")]
        path: String,
    },
}

fn create_server_template(path: &str) {
    let template = r#"game = fmc_vanilla
version = 0.1.0

[mods]
# examples:
# mod from crates.io
# mod_name = 1.0.0
# mod from github
# mod_name = https://github.com/modder/mod_name
"#;

    if std::path::Path::new(path).exists() {
        println!("There is already a file at '{path}'");
        return;
    }

    if let Err(e) = std::fs::write(path, template) {
        println!("Failed to create server config, error: {e}");
    }
}

// TODO: Crate names/versions must be verified to exist, can't hit cargo error, bad user
// experience.
// TODO: It's probably worth making a serde deserializer for this format with good errors and such.
// I've been using it for the server side config as well.
// It's very close to the ini format, but keys should be "key" instead of "key=" when they're valueless.
// https://crates.io/crates/serde_ini was very easy to modify to allow this, but it is not good
// enough.
fn parse_server_build_config(path: &str) -> Result<ServerBuildConfig, String> {
    fn validate_version(version: &str) -> bool {
        let version = version.splitn(3, ".").collect::<Vec<&str>>();
        if version.len() != 3 {
            return false;
        }

        for version_number in version {
            if !version_number.chars().all(|c| char::is_ascii_digit(&c)) {
                return false;
            }
        }

        return true;
    }

    let config_str = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            return match e.kind() {
                io::ErrorKind::NotFound => Err(
                    "server.conf not found, you can create it with 'fmc build --template'"
                        .to_owned(),
                ),
                _ => Err(format!("Error while trying to read server.conf: {e}")),
            }
        }
    };

    let mut mod_section = false;
    let mut mods = Vec::new();
    let mut game_name = None;
    let mut game_version = None;

    for (n, line) in config_str.lines().enumerate() {
        let line = line.trim();
        if line == "[mods]" {
            mod_section = true;
            continue;
        }

        if line.is_empty() || line.starts_with("#") {
            continue;
        }

        let Some((key, value)) = line.split_once("=") else {
            return Err(format!(
                "line {n}: Entries must be in the format 'key = value', cannot be '{line}'"
            ));
        };

        let key = key.trim();
        let value = value.trim();

        if mod_section {
            let spec = if !validate_version(value) || !value.starts_with("https://") {
                format!("{{ version = \"{value}\" }}")
            } else if value.starts_with("https://") {
                format!("{{ git = \"{value}\" }}")
            } else {
                return Err(format!("line {n}: invalid spec {value}, must be either a version e.g. '1.0.0' or a git url e.g. 'https://github.com/..."));
            };

            mods.push(Mod::new(key.to_owned(), spec));
        } else {
            match key {
                "game" => game_name = Some(value.to_owned()),
                "version" => {
                    if !validate_version(value) {
                        return Err(format!("line {n}: Versions must be given in the format 'x.y.z' cannot be '{value}', e.g. 'version = 1.0.0'"));
                    }
                    game_version = Some(value.to_owned())
                }
                "git" => {
                    if !value.starts_with("https://") {
                        return Err(format!(
                            "line {n}, invalid git url {value}, must be in format 'git = https://..."
                        ));
                    }
                }
                _ => return Err(format!("line {n}: Invalid setting '{key}'")),
            }
        }
    }

    let Some(game_name) = game_name else {
        return Err(format!(
            "You must supply a game name like: 'game = fmc_vanilla'"
        ));
    };

    let Some(game_version) = game_version else {
        return Err(format!(
            "You must supply a game version like: 'version = 1.0.0', or a git url like: 'git = https://github.com/fmc/fmc_beta'"
        ));
    };

    let game = Mod::new(game_name, game_version);
    return Ok(ServerBuildConfig::new(game, mods));
}

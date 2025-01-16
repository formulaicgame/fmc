use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Mod {
    name: String,
    version: String,
}

impl Mod {
    pub fn new(name: String, version: String) -> Self {
        Self { name, version }
    }
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }
}

#[derive(Deserialize)]
pub struct ServerBuildConfig {
    game: Mod,
    #[serde(default)]
    mods: Vec<Mod>,
}

impl ServerBuildConfig {
    pub fn new(game: Mod, mods: Vec<Mod>) -> Self {
        Self { game, mods }
    }

    pub fn build(&self) {
        get_rust();

        self.create_cargo_project();

        let mut command = cargo_command();
        command.args(["build", "--release"]);
        command.current_dir("./server");
        if command.status().is_ok() {
            std::fs::copy("./server/target/release/server", ".").unwrap();
        }
    }

    fn create_cargo_project(&self) {
        let main_rs = format!(
            r#"
use game::bevy::prelude::*;

fn main() {{
    App::new()
        .add_plugins((
            game::DefaultPlugins,
            {}
        ))
        .run();
}}
"#,
            self.mods
                .iter()
                .map(|m| m.name())
                .collect::<Vec<&str>>()
                .join("::Mod,\n")
                + "::Mod\n"
        );

        let mut cargo_toml = format!(
            r#"
[package]
name = "server"
version = "0.1.0"
edition = "2024"

[dependencies]
{} = {{ version = "{}", package = "game" }}
"#,
            &self.game.name(),
            &self.game.version(),
        );

        for dependency in &self.mods {
            cargo_toml += &format!("{} = \"{}\"\n", dependency.name(), dependency.version());
        }

        if let Err(e) = std::fs::create_dir_all("./server/src") {
            println!("Could not create server compiltation folder, error: {}", e);
            return;
        }

        if let Err(e) = std::fs::write("./server/src/main.rs", main_rs) {
            println!("Could construct server directory, error: {}", e);
            return;
        }

        if let Err(e) = std::fs::write("./server/Cargo.toml", cargo_toml) {
            println!("Could construct server directory, error: {}", e);
            return;
        }
    }
}

fn cargo_command() -> std::process::Command {
    let data_dir = data_dir().unwrap();
    let mut command = std::process::Command::new(data_dir.join("fmc/rust/bin/cargo"));
    command.env("CARGO_HOME", ".");

    command
}

// TODO: Replace when ready for xdg
fn data_dir() -> Option<PathBuf> {
    //dirs::data_dir()
    Some(PathBuf::from("./server"))
}

// Returns true if a rust toolchain is available
fn has_rust() -> bool {
    let Some(data_dir) = data_dir() else {
        return false;
    };

    if data_dir.join("fmc/rust").exists() {
        return true;
    }

    return false;
}

fn get_rust() {
    if has_rust() {
        return;
    }

    let url = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => {
            "https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init"
        }
        ("windows", "x86_64") => {
            "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe"
        }
        ("macos", "x86_64") => {
            "https://static.rust-lang.org/rustup/dist/x86_64-apple-darwin/rustup-init"
        }
        ("macos", "aarch64") => {
            "https://static.rust-lang.org/rustup/dist/aarch64-apple-darwin/rustup-init"
        }
        _ => return,
    };

    let response = match reqwest::blocking::get(url) {
        Ok(r) => r,
        Err(_) => return,
    };

    let bytes = match response.bytes() {
        Ok(b) => b,
        Err(_) => return,
    };

    let mut path = std::env::temp_dir().join("fmc");
    std::fs::create_dir(&path).ok();
    path.push("rustup-init");
    path.set_extension(std::env::consts::EXE_EXTENSION);
    std::fs::write(&path, bytes).ok();

    if std::env::consts::FAMILY == "unix" {
        if !std::process::Command::new("chmod")
            .arg("+x")
            .arg(&path)
            .status()
            .is_ok()
        {
            println!("Could not add exec permission to rustup");
            return;
        }
    }

    let Some(data_dir) = data_dir() else {
        return;
    };

    // TODO: This can error if the user doesn't have permission, but that can't happen right?
    let rust_path = data_dir.join("fmc/rust");
    std::fs::create_dir_all(&rust_path).ok();

    let mut command = std::process::Command::new(&path);
    command.env("CARGO_HOME", &rust_path);
    command.env("RUSTUP_HOME", &rust_path);
    // Skip all confirmation prompts
    command.arg("-y");
    // Local installation, don't mess up the users path
    command.arg("--no-modify-path");
    // Only install rustc, cargo, and rust-std
    command.args(["--profile", "minimal"]);

    if let Err(e) = command.status() {
        error!("Could not execute rustup, error: {}", e);
    }
}

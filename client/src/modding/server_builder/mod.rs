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
        command.current_dir("./build");
        if command.status().is_ok() {
            let mut exe_path = PathBuf::from("build/target/release/server");
            exe_path.set_extension(std::env::consts::EXE_EXTENSION);
            std::fs::copy(&exe_path, exe_path.file_name().unwrap()).unwrap();
        }

        println!(
            "Delete the 'build' folder if you don't intend to change the server.\n\
            Keeping it will make the build go faster, but the folder is huge(~1-3Gb)."
        )
    }

    fn create_cargo_project(&self) {
        let main_rs = format!(
            r#"
use game::prelude::*;

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
                .map(|m| format!("{}::Mod,", m.name()))
                .collect::<Vec<String>>()
                .join("\n")
        );

        let mut cargo_toml = format!(
            r#"
[package]
name = "server"
version = "0.1.0"
edition = "2024"

[dependencies]
game = {{ version = "{}", package = "{}" }}
"#,
            &self.game.version(),
            &self.game.name(),
        );

        for dependency in &self.mods {
            cargo_toml += &format!("{} = {}\n", dependency.name(), dependency.version());
        }

        if let Err(e) = std::fs::create_dir_all("./build/src") {
            println!("Could not create build directory, error: {}", e);
            return;
        }

        if let Err(e) = std::fs::write("./build/src/main.rs", main_rs) {
            println!("Failed while writing to build directory, error: {}", e);
            return;
        }

        if let Err(e) = std::fs::write("./build/Cargo.toml", cargo_toml) {
            println!("Failed while writin to build directory, error: {}", e);
            return;
        }
    }
}

fn cargo_command() -> std::process::Command {
    let data_dir = data_dir().unwrap();
    let mut command =
        std::process::Command::new(data_dir.join("fmc/rust/bin/cargo").canonicalize().unwrap());
    command.env("CARGO_HOME", data_dir);

    command
}

// TODO: Replace when ready for xdg
#[track_caller]
fn data_dir() -> Option<PathBuf> {
    //dirs::data_dir()
    Some(PathBuf::from("./build"))
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

    let response = match ureq::get(url).call() {
        Ok(r) => r,
        Err(_) => return,
    };

    let mut buf = Vec::with_capacity(10 * 1024 * 1024);
    if response.into_reader().read_to_end(&mut buf).is_err() {
        return;
    };

    let Some(data_dir) = data_dir() else {
        return;
    };

    // TODO: This can error if the user doesn't have permission, but that can't happen right?
    let rust_path = data_dir.join("fmc/rust");
    std::fs::create_dir_all(&rust_path).ok();

    let mut rustup_path = rust_path.join("rustup-init");
    rustup_path.set_extension(std::env::consts::EXE_EXTENSION);
    std::fs::write(&rustup_path, buf).ok();

    if std::env::consts::FAMILY == "unix" {
        if !std::process::Command::new("chmod")
            .arg("+x")
            .arg(&rustup_path)
            .status()
            .is_ok()
        {
            println!("Could not add exec permission to rustup");
            return;
        }
    }

    let mut command = std::process::Command::new(&rustup_path);
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

    //std::fs::remove_file(rustup_path).ok();
}

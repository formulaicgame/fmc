use std::io::{Read, Write};

use bevy::{prelude::*, tasks::AsyncComputeTaskPool};
use crossbeam::{Receiver, Sender};

use super::{GuiState, Interface, Interfaces};
use crate::{
    game_state::GameState,
    networking::{Identity, NetworkClient},
    singleplayer::LaunchSinglePlayer,
    ui::{client::widgets::*, text_input::TextBox},
};

pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup, download_default_game))
            .add_systems(
                Update,
                (
                    press_singleplayer_button,
                    press_join_button,
                    goto_login,
                    download_progress_text,
                )
                    .run_if(in_state(GuiState::MainMenu)),
            );
    }
}

#[derive(Component)]
struct SinglePlayerButton;

#[derive(Component)]
struct ServerIp;

#[derive(Component)]
struct JoinButton;

fn setup(mut commands: Commands, mut interfaces: ResMut<Interfaces>) {
    let entity = commands
        .spawn((
            Interface,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                row_gap: Val::Px(4.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor::from(Color::srgb_u8(33, 33, 33)),
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    // XXX: Manually positioned since we want the interactive elements to
                    // remain centered
                    margin: UiRect::bottom(Val::Percent(20.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn_text("").insert(DownloadStatusText);
                });
            parent
                .spawn(Node {
                    width: Val::Px(200.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|parent| {
                    parent
                        .spawn_button("Singleplayer", Srgba::gray(0.7))
                        .insert(Node {
                            width: Val::Percent(100.0),
                            aspect_ratio: Some(200.0 / 20.0),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .insert(SinglePlayerButton);

                    parent
                        .spawn_textbox("hello")
                        .insert(Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(60.0),
                            ..default()
                        })
                        .insert(ServerIp);
                    parent
                        .spawn_button("Connect", Srgba::gray(0.7))
                        .insert(Node {
                            width: Val::Percent(100.0),
                            aspect_ratio: Some(200.0 / 20.0),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .insert(JoinButton);
                });
        })
        .id();
    interfaces.insert(GuiState::MainMenu, entity);
}

// TODO: The button should lead to its own screen where you select game and save file
fn press_singleplayer_button(
    button_query: Query<&Interaction, (Changed<Interaction>, With<SinglePlayerButton>)>,
    mut launch_single_player: EventWriter<LaunchSinglePlayer>,
) {
    if let Ok(interaction) = button_query.single() {
        if *interaction == Interaction::Pressed {
            launch_single_player.send(LaunchSinglePlayer {});
        }
    }
}

fn press_join_button(
    mut net: ResMut<NetworkClient>,
    keys: Res<ButtonInput<KeyCode>>,
    server_ip: Query<&TextBox, With<ServerIp>>,
    play_button: Query<&Interaction, (Changed<Interaction>, With<JoinButton>)>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    if play_button
        .single()
        .is_ok_and(|interaction| *interaction == Interaction::Pressed)
        || keys.just_pressed(KeyCode::Enter)
    {
        let mut ip = server_ip.single().unwrap().text.to_owned();

        if !ip.contains(":") {
            ip.push_str(":42069");
        }

        let addr = match ip.parse() {
            Ok(addr) => addr,
            Err(_) => return,
        };

        net.connect(addr);
        game_state.set(GameState::Connecting);
    }
}

fn goto_login(identity: Res<Identity>, mut gui_state: ResMut<NextState<GuiState>>) {
    if !identity.is_valid() {
        gui_state.set(GuiState::Login);
    }
}

enum DownloadStatus {
    Success { path: String },
    Progress { current: usize, total: usize },
    Failure(String),
}

#[derive(Component)]
struct DownloadStatusText;

#[derive(Component)]
struct DownloadReporter(Receiver<DownloadStatus>);

fn download_progress_text(
    time: Res<Time>,
    mut status_text: Query<(&mut Text, &mut Visibility), With<DownloadStatusText>>,
    downloads: Query<&DownloadReporter>,
    mut timer: Local<Timer>,
) {
    let (mut text, mut visibility) = status_text.single_mut().unwrap();

    for reporter in downloads.iter() {
        while let Ok(status) = reporter.0.try_recv() {
            *timer = Timer::from_seconds(2.0, TimerMode::Once);
            *visibility = Visibility::Inherited;

            match status {
                DownloadStatus::Success { path } => {
                    if std::env::consts::FAMILY == "unix" {
                        if std::process::Command::new("chmod")
                            .arg("+x")
                            .arg(&path)
                            .status()
                            .is_err()
                        {
                            error!("Couldn't set execution permissions for server");
                        }
                    }
                    *text = Text::new("Singleplayer server downloaded!");
                }
                DownloadStatus::Progress { current, total } => {
                    fn bytes_to_string(bytes: usize) -> String {
                        const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

                        let mut index = 0;
                        let mut value = bytes as f64;

                        while value >= 1024.0 && index < UNITS.len() - 1 {
                            value /= 1024.0;
                            index += 1;
                        }

                        // Round to one decimal place
                        let rounded_value = (value * 10.0).round() / 10.0;

                        // Format the result
                        format!("{:.1}{}", rounded_value, UNITS[index])
                    }

                    text.0 = format!(
                        "Downloading singleplayer: {}/{}",
                        bytes_to_string(current),
                        bytes_to_string(total)
                    );
                }
                DownloadStatus::Failure(_err) => {
                    text.0 = "Failed to download singleplayer".to_owned()
                }
            }
        }
    }

    timer.tick(time.delta());
    if timer.just_finished() {
        *visibility = Visibility::Hidden;
    }
}

fn download_default_game(mut commands: Commands) {
    let server_path = String::from("fmc_server/server") + std::env::consts::EXE_SUFFIX;
    if std::path::Path::new(&server_path).exists() {
        return;
    }

    let url = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "https://github.com/awowogei/fmc_173/releases/download/nightly/x86_64-unknown-linux-gnu",
        ("windows", "x86_64") => "https://github.com/awowogei/fmc_173/releases/download/nightly/x86_64-pc-windows-msvc.exe",
        ("macos", "x86_64") => "https://github.com/awowogei/fmc_173/releases/download/nightly/x86_64-apple-darwin",
        ("macos", "aarch64") => "https://github.com/awowogei/fmc_173/releases/download/nightly/aarch64-apple-darwin",
        _ => return
    }.to_owned();

    let (sender, receiver) = crossbeam::unbounded();
    commands.spawn(DownloadReporter(receiver));

    AsyncComputeTaskPool::get()
        .spawn(download_game(url, server_path, sender))
        .detach();
}

async fn download_game(url: String, result_path: String, reporter: Sender<DownloadStatus>) {
    let Ok(response) = ureq::get(&url).call() else {
        reporter
            .send(DownloadStatus::Failure(
                "Download url inaccessible".to_owned(),
            ))
            .unwrap();
        return;
    };

    if response.status() != 200 {
        reporter
            .send(DownloadStatus::Failure(
                "Download refused by server".to_owned(),
            ))
            .unwrap();
        return;
    }

    let path = std::path::Path::new(&result_path);
    if std::fs::create_dir_all(path.parent().unwrap()).is_err() {
        reporter
            .send(DownloadStatus::Failure(
                "Could not create download directory".to_owned(),
            ))
            .unwrap();
        return;
    };

    let Ok(file) = std::fs::File::create(&result_path) else {
        reporter
            .send(DownloadStatus::Failure(
                "Could not create download file".to_owned(),
            ))
            .unwrap();
        return;
    };
    // TODO: https://github.com/rust-lang/rust/issues/130804
    let mut file = std::io::BufWriter::new(file);

    let size = response.headers()["content-length"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let mut reader = response.into_body().into_reader();
    let mut downloaded = 0;
    loop {
        let mut buf = vec![0; 2048];
        match reader.read(&mut buf) {
            Ok(n) if n > 0 => {
                // clone data from buffer and clear it
                let Ok(written) = file.write(&buf[..n]) else {
                    reporter
                        .send(DownloadStatus::Failure(
                            "File unexpectedly unavailable".to_owned(),
                        ))
                        .unwrap();
                    return;
                };
                downloaded += written;
                reporter
                    .send(DownloadStatus::Progress {
                        current: downloaded,
                        total: size,
                    })
                    .unwrap();
            }
            Ok(_) => {
                if file.flush().is_err() {
                    panic!();
                };
                reporter
                    .send(DownloadStatus::Success { path: result_path })
                    .unwrap();
                return;
            }
            Err(err) => {
                reporter
                    .send(DownloadStatus::Failure(err.to_string()))
                    .unwrap();
                return;
            }
        };
    }
}

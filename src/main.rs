use clap::Parser;
use configuration::{AppConfiguration, ScreensProfile};
use futures::{future::BoxFuture, FutureExt};
use libmonitor::{ddc::DdcDevice, Monitor};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process,
    sync::{Arc, RwLock},
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use wayland_client::backend::ObjectId;
use wlr_output_state::MonitorInformation;

mod configuration;
mod ddc;
mod wlr_output_state;

static SOCKET_ADDR: Lazy<String> = Lazy::new(|| {
    env::var("XDG_RUNTIME_DIR")
        .and_then(|run_time_dir| {
            Ok(Path::new(&run_time_dir)
                .join("workspaces.socket")
                .to_str()
                .unwrap_or("/tmp/workspaces.socket")
                .to_string())
        })
        .unwrap_or("/tmp/workspaces.socket".to_string())
});

const TIMEOUT: u64 = 1000;

static DAEMON_STATE: Lazy<Arc<RwLock<DaemonState>>> =
    Lazy::new(|| Arc::new(RwLock::new(DaemonState::default())));

struct DaemonState {
    head_state: HashMap<ObjectId, MonitorInformation>,
    config: AppConfiguration,
    current_profile: Option<String>,
}

impl Default for DaemonState {
    fn default() -> Self {
        Self {
            head_state: HashMap::new(),
            config: AppConfiguration::default(),
            current_profile: None,
        }
    }
}

fn get_newest_message<'a>(
    wlr_rx: &'a mut UnboundedReceiver<HashMap<ObjectId, MonitorInformation>>,
) -> BoxFuture<'a, Result<HashMap<ObjectId, MonitorInformation>, mpsc::error::TryRecvError>> {
    async move {
        match wlr_rx.try_recv() {
            Ok(head_config) => {
                eprintln!("waiting for new state");
                tokio::time::sleep(tokio::time::Duration::from_millis(TIMEOUT)).await;
                match get_newest_message(wlr_rx).await {
                    Ok(newer_head_conifg) => Ok(newer_head_conifg),
                    Err(_) => Ok(head_config),
                }
            }
            Err(err) => Err(err),
        }
    }
    .boxed()
}

async fn connected_monitor_listen(
    mut wlr_rx: UnboundedReceiver<HashMap<ObjectId, MonitorInformation>>,
) {
    loop {
        if let Some(current_connected_monitors) = get_newest_message(&mut wlr_rx).await.ok() {
            println!(
                "{:#?}",
                current_connected_monitors.keys().collect::<Vec<_>>()
            );
            // run this in its own thread te make sure the runtime does not get blocked!
            let _ = tokio::task::spawn_blocking(|| {
                let _ = DAEMON_STATE.clone().write().and_then(|mut daemon_state| {
                    // add ddc connections to daemon state
                    if let Some((profile_name, profile)) = daemon_state
                        .config
                        .clone()
                        .profiles()
                        .iter()
                        .filter_map(|(name, profile)| {
                            eprintln!("Checking if profile {} is connected", name);
                            if profile.is_connected(&current_connected_monitors) {
                                Some((name, profile))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<(&String, &ScreensProfile)>>()
                        .first()
                    {
                        let hyprland_config_file =
                            daemon_state.config.hyprland_config_file().clone();
                        profile.apply(&current_connected_monitors, &hyprland_config_file);
                        daemon_state.current_profile = Some(profile_name.to_string());
                    }
                    eprintln!("apply configuration!");
                    daemon_state.head_state = current_connected_monitors;
                    Ok(())
                });
            })
            .await;
        } else {
            tokio::time::sleep(tokio::time::Duration::from_millis(TIMEOUT)).await;
        }
    }
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Options {
    #[arg(short)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Parser, Serialize, Deserialize, Clone)]
struct ProfileSelector {
    name: String,
}

#[derive(Debug, Parser, Clone, Serialize, Deserialize)]
enum Command {
    Attached,
    Profiles,
    CurrentProfile,
    MonitorInputs,
    Pid,
    Apply(ProfileSelector),
}

impl Command {
    pub fn run(&self, buffer: &mut BufWriter<UnixStream>) {
        match self {
            Command::Attached => {
                let _ = DAEMON_STATE.read().and_then(|daemon_state| {
                    let _ = writeln!(buffer, "Attached Monitors:");
                    for (_id, head) in daemon_state.head_state.iter() {
                        let _ = writeln!(
                            buffer,
                            "{}: {} {}\n",
                            head.name(),
                            head.make(),
                            head.serial().as_ref().unwrap_or(&"".to_string())
                        );
                        let _ = buffer.flush();
                    }
                    Ok(())
                });
            }
            Command::Profiles => {
                let _ = DAEMON_STATE.read().and_then(|daemon_state| {
                    let _ = writeln!(buffer, "Profiles:");
                    let _ = writeln!(
                        buffer,
                        "{}",
                        serde_yaml::to_string(&daemon_state.config.profiles()).unwrap()
                    );
                    Ok(())
                });
            }
            Command::CurrentProfile => {
                let _ = DAEMON_STATE.read().and_then(|daemon_state| {
                    let _ = writeln!(
                        buffer,
                        "Current Profile: {}",
                        daemon_state
                            .current_profile
                            .as_ref()
                            .unwrap_or(&"".to_string())
                    );
                    Ok(())
                });
            }
            Command::MonitorInputs => {
                let monitors = Monitor::enumerate();
                for mut display in monitors {
                    display.get_input_source().and_then(|input_source| {
                        let _ = writeln!(buffer, "{}: {:?}", display.handle.name(), input_source);
                        let _ = buffer.flush();
                        Ok(())
                    });
                }
            }
            Command::Pid => {
                let _ = writeln!(buffer, "{}", process::id());
            }
            Command::Apply(profile_selector) => {
                let _ = DAEMON_STATE.write().and_then(|mut daemon_state| {
                    match daemon_state
                        .config
                        .clone()
                        .profiles()
                        .get(&profile_selector.name)
                    {
                        Some(profile) => {
                            let head_config = daemon_state.head_state.clone();
                            let hyprland_config_file =
                                daemon_state.config.hyprland_config_file().clone();
                            profile.apply(&head_config, &hyprland_config_file);
                            daemon_state.current_profile = Some(profile_selector.name.clone());
                        }
                        None => {
                            let _ =
                                writeln!(buffer, "No profile with name {}!", profile_selector.name);
                        }
                    }
                    Ok(())
                });
            }
        }
    }
}

fn command_listener() {
    let _ = UnixListener::bind(SOCKET_ADDR.as_str()).and_then(|socket_server| {
        for connection in socket_server.incoming() {
            let _ = connection.and_then(|mut stream| {
                let reader = BufReader::new(&mut stream);
                let recv_command: Result<Command, Box<bincode::ErrorKind>> =
                    bincode::deserialize_from(reader);
                match recv_command {
                    Ok(command) => {
                        let mut buffer = BufWriter::new(stream);
                        command.run(&mut buffer);
                    }
                    Err(err) => {
                        let mut buffer = BufWriter::new(stream);
                        let _ = writeln!(buffer, "Error receiving command! {err:#?}");
                    }
                }
                Ok(())
            });
        }
        Ok(())
    });
}

fn check_socket_alive() -> bool {
    Path::new(SOCKET_ADDR.as_str()).exists()
        && UnixStream::connect(SOCKET_ADDR.as_str())
            .and_then(|mut con| {
                let _ = bincode::serialize(&Command::Pid).and_then(|command_bin| {
                    let _ = con.write(&command_bin);
                    let _ = con.flush();
                    Ok(())
                });
                let mut resp = String::new();
                let _ = con.read_to_string(&mut resp);
                Ok(resp
                    .trim()
                    .parse::<i32>()
                    .and_then(|_pid| Ok(true))
                    .unwrap_or(false))
            })
            .unwrap_or(false)
}

#[tokio::main]
async fn main() {
    let cmd_options = Options::parse();

    match cmd_options.command {
        // programm running as client
        Some(command) => {
            if !Path::new(SOCKET_ADDR.as_str()).exists() {
                println!(
                    "No daemon process is running at {}! Exiting",
                    SOCKET_ADDR.as_str()
                );
                return;
            }
            let _ = UnixStream::connect(SOCKET_ADDR.as_str()).and_then(|mut socket_stream| {
                let _ = bincode::serialize(&command).and_then(|command_bin| {
                    let _ = socket_stream.write(&command_bin);
                    let _ = socket_stream.flush();
                    Ok(())
                });
                let buffer = BufReader::new(socket_stream);
                for line in buffer.lines() {
                    match line {
                        Ok(l) => {
                            if l.len() != 0 {
                                println!("{l}");
                            }
                        }
                        Err(_) => {}
                    }
                }
                Ok(())
            });
        }

        // programm running as deamon
        None => {
            let config_path = cmd_options
                .config
                .unwrap_or(Path::new("workplaces.yml").into());
            let _ = DAEMON_STATE.write().and_then(|mut daemon_state| {
                let _ = File::open(config_path).and_then(|file_reader| {
                    daemon_state.config = serde_yaml::from_reader(file_reader)
                        .expect("Could not parse workspace profiles!");
                    Ok(())
                });
                Ok(())
            });

            let socket_path = Path::new(SOCKET_ADDR.as_str());
            if check_socket_alive() {
                println!(
                    "Daemon process is running at {}! Exiting",
                    SOCKET_ADDR.as_str()
                );
                return;
            } else if socket_path.exists() {
                let _ = std::fs::remove_file(&socket_path);
            }

            let (wlr_tx, wlr_rx) =
                mpsc::unbounded_channel::<HashMap<ObjectId, MonitorInformation>>();
            let wlr_output_updates_blocking = tokio::task::spawn_blocking(|| {
                wlr_output_state::wayland_event_loop(wlr_tx);
            });
            let commmand_listener_task = tokio::task::spawn_blocking(|| {
                command_listener();
            });
            let connected_monitors_handler = tokio::task::spawn(connected_monitor_listen(wlr_rx));
            let _ = tokio::join!(
                wlr_output_updates_blocking,
                connected_monitors_handler,
                commmand_listener_task
            );
        }
    }
}

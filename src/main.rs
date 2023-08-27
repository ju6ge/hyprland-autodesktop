use std::{path::{Path, PathBuf},
          collections::HashMap,
          env,
          os::unix::net::{UnixStream, UnixListener},
          io::{Write, BufReader, BufRead, BufWriter, Read},
          sync::{Arc, RwLock}, fs::File, process
};
use clap::Parser;
use configuration::{AppConfiguration, ScreensProfile};
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use wayland_client::backend::ObjectId;
use wlr_output_state::MonitorInformation;

mod wlr_output_state;
mod configuration;

static SOCKET_ADDR : Lazy<String> = Lazy::new(|| {
    env::var("XDG_RUNTIME_DIR")
    .and_then(|run_time_dir| {
        Ok(Path::new(&run_time_dir).join("workspaces.socket").to_str().unwrap_or("/tmp/workspaces.socket").to_string())
    }).unwrap_or("/tmp/workspaces.socket".to_string())
});

static DAEMON_STATE : Lazy<Arc<RwLock<DaemonState>>> = Lazy::new(|| {
    Arc::new(RwLock::new(DaemonState::default()))
});

struct DaemonState {
    head_state: HashMap<ObjectId, MonitorInformation>,
    config: AppConfiguration,
    current_profile: Option<String>
}

impl Default for DaemonState {
    fn default() -> Self {
        Self {
            head_state: HashMap::new(),
            config: AppConfiguration::default(),
            current_profile: None
        }
    }
}

async fn connected_monitor_listen(mut wlr_rx: UnboundedReceiver<HashMap<ObjectId, MonitorInformation>>) {
    while let Some(current_connected_monitors) = wlr_rx.recv().await {
        let _ = DAEMON_STATE.clone().write().and_then(|mut daemon_state| {
            if let Some((profile_name, profile)) = daemon_state.config.profiles().iter().filter_map(|(name, profile)| {
                if profile.is_connected(&current_connected_monitors) {
                    Some((name, profile))
                } else {
                    None
                }
            }).collect::<Vec<(&String, &ScreensProfile)>>().first() {
                profile.apply(&current_connected_monitors, &daemon_state.config.hyprland_config_file());
                daemon_state.current_profile = Some(profile_name.to_string());
            }
            daemon_state.head_state = current_connected_monitors;
            Ok(())
        });
    }
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Options {
    #[arg(short)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>
}

#[derive(Debug, Parser, Clone, Serialize, Deserialize)]
enum Command {
    Attached,
    Profiles,
    CurrentProfile,
    Pid,
    Apply
}

impl Command {
    pub fn run(&self, buffer: &mut BufWriter<UnixStream>) {
        match self {
            Command::Attached => {
                let _ = DAEMON_STATE.read().and_then(|daemon_state| {
                    let _ = writeln!(buffer, "Attached Monitors:");
                    for (_id, head) in  daemon_state.head_state.iter() {
                        let _ = writeln!(buffer, "{}: {} {}\n", head.name(), head.make(), head.serial().as_ref().unwrap_or(&"".to_string()));
                        let _ = buffer.flush();
                    }
                    Ok(())
                });
            },
            Command::Profiles => {
                let _ = DAEMON_STATE.read().and_then(|daemon_state| {
                    let _ = writeln!(buffer, "Profiles:");
                    let _ = writeln!(buffer, "{}", serde_yaml::to_string(&daemon_state.config.profiles()).unwrap());
                    Ok(())
                });
            }
            Command::Pid => {
                let _ = writeln!(buffer, "{}", process::id());
            }
            Command::Apply => {},
            Command::CurrentProfile => {
                let _ = DAEMON_STATE.read().and_then(|daemon_state| {
                    let _ = writeln!(buffer, "Current Profiles: {}", daemon_state.current_profile.as_ref().unwrap_or(&"".to_string()));
                    Ok(())
                });
            },
        }
    }
}

fn command_listener() {
    let _ = UnixListener::bind(SOCKET_ADDR.as_str()).and_then(|socket_server| {
      for connection in socket_server.incoming() {
          let _ = connection.and_then(|mut stream| {
              let reader = BufReader::new(&mut stream);
              let recv_command: Result<Command, Box<bincode::ErrorKind>> = bincode::deserialize_from(reader);
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
    Path::new(SOCKET_ADDR.as_str()).exists() && UnixStream::connect(SOCKET_ADDR.as_str()).and_then(|mut con| {
        let _ = bincode::serialize(&Command::Pid).and_then(|command_bin| {
            let _ = con.write(&command_bin);
            let _ = con.flush();
            Ok(())
        });
        let mut resp = String::new();
        let _ = con.read_to_string(&mut resp);
        Ok(resp.trim().parse::<i32>().and_then(|_pid| {
            Ok(true)
        }).unwrap_or(false))
    }).unwrap_or(false)
}

#[tokio::main]
async fn main() {
    let cmd_options = Options::parse();

    match cmd_options.command {
        // programm running as client
        Some(command) => {
            if !Path::new(SOCKET_ADDR.as_str()).exists() {
                println!("No daemon process is running at {}! Exiting", SOCKET_ADDR.as_str());
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
                        Ok(l) => if l.len() != 0 { println!("{l}"); }
                        Err(_) => {},
                    }
                }
                Ok(())
            });
        }

        // programm running as deamon
        None => {
            let config_path = cmd_options.config.unwrap_or(Path::new("workplaces.yml").into());
            let _ = DAEMON_STATE.write().and_then(|mut daemon_state| {
                let _ = File::open(config_path).and_then(|file_reader| {
                    daemon_state.config = serde_yaml::from_reader(file_reader).expect("Could not parse workspace profiles!");
                    Ok(())
                });
                Ok(())
            });

            let socket_path = Path::new(SOCKET_ADDR.as_str());
            if check_socket_alive() {
                println!("Daemon process is running at {}! Exiting", SOCKET_ADDR.as_str());
                return;
            } else if socket_path.exists() {
                let _ = std::fs::remove_file(&socket_path);
            }

            let (wlr_tx, wlr_rx) = mpsc::unbounded_channel::<HashMap<ObjectId, MonitorInformation>>();
            let wlr_output_updates_blocking = tokio::task::spawn_blocking(|| {
                wlr_output_state::wayland_event_loop(wlr_tx);
            });
            let commmand_listener_task = tokio::task::spawn_blocking(|| {
                command_listener();
            });
            let connected_monitors_handler = tokio::spawn(connected_monitor_listen(wlr_rx));
            let _ = tokio::join!(wlr_output_updates_blocking, connected_monitors_handler, commmand_listener_task);
        }
    }
}

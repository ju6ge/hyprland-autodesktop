use clap::Parser;
use configuration::{AppConfiguration, ScreensProfile, SwayMonitor};
use itertools::Itertools;
use libmonitor::{ddc::DdcDevice, Monitor};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, Receiver, Sender};
use std::{
    collections::{BTreeMap, HashMap},
    env,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process,
    sync::{Arc, RwLock},
    thread::sleep,
};
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
    wlr_rx: &'a mut Receiver<HashMap<ObjectId, MonitorInformation>>,
) -> Result<HashMap<ObjectId, MonitorInformation>, mpsc::TryRecvError> {
    match wlr_rx.try_recv() {
        Ok(head_config) => {
            eprintln!("waiting for new state");
            sleep(std::time::Duration::from_millis(TIMEOUT));
            match get_newest_message(wlr_rx) {
                Ok(newer_head_conifg) => Ok(newer_head_conifg),
                Err(_) => Ok(head_config),
            }
        }
        Err(err) => Err(err),
    }
}

fn connected_monitor_listen(
    mut wlr_rx: Receiver<HashMap<ObjectId, MonitorInformation>>,
    config_head_tx: Sender<Vec<(ObjectId, SwayMonitor)>>,
) {
    loop {
        if let Some(current_connected_monitors) = get_newest_message(&mut wlr_rx).ok() {
            println!(
                "{:#?}",
                current_connected_monitors.keys().collect::<Vec<_>>()
            );
            let mut config_update_tx = config_head_tx.clone();
            let mut current_monitor_inputs = BTreeMap::new();
            for mut monitor in Monitor::enumerate() {
                if let Ok(monitor_input) = monitor.get_input_source() {
                    current_monitor_inputs.insert(monitor.handle.name(), monitor_input);
                }
            }
            let _ = DAEMON_STATE.clone().write().and_then(|mut daemon_state| {
                if let Some((profile_name, profile)) = daemon_state
                    .config
                    .clone()
                    .profiles()
                    .iter()
                    .filter_map(|(name, profile)| {
                        eprintln!("Checking if profile {} is connected", name);
                        if profile
                            .is_connected(&current_connected_monitors, &current_monitor_inputs)
                        {
                            Some((name, profile))
                        } else {
                            None
                        }
                    })
                    .sorted_by_key(|profile| profile.1.weight()) // rate matching profiles
                    .rev() // profile with highest weight should be first
                    .collect::<Vec<(&String, &ScreensProfile)>>()
                    .first()
                {
                    profile.apply(&current_connected_monitors, &mut config_update_tx);
                    daemon_state.current_profile = Some(profile_name.to_string());
                }
                eprintln!("apply configuration!");
                daemon_state.head_state = current_connected_monitors;
                Ok(())
            });
        } else {
            // timeout here to avoid this read running with cpu at 100% when nothing is happening
            sleep(std::time::Duration::from_millis(TIMEOUT));
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
    /// List currently attached monitors and their names
    Attached,
    /// List all configured profiles with their settings
    Profiles,
    /// Print name of currently active profile
    CurrentProfile,
    /// Display current monitor and what input their are currently set to display
    MonitorInputs,
    /// Return the PID of the currently running daemon process
    Pid,
    /// Switch profile to the specified one
    Apply(ProfileSelector),
}

impl Command {
    pub fn run(
        &self,
        buffer: &mut BufWriter<UnixStream>,
        config_head_tx: &mut Sender<Vec<(ObjectId, SwayMonitor)>>,
    ) {
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
                    let _ = display.get_input_source().and_then(|input_source| {
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
                            profile.apply(&head_config, config_head_tx);
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

fn command_listener(mut head_config_tx: Sender<Vec<(ObjectId, SwayMonitor)>>) {
    let _ = UnixListener::bind(SOCKET_ADDR.as_str()).and_then(|socket_server| {
        for connection in socket_server.incoming() {
            let _ = connection.and_then(|mut stream| {
                let reader = BufReader::new(&mut stream);
                let recv_command: Result<Command, Box<bincode::ErrorKind>> =
                    bincode::deserialize_from(reader);
                match recv_command {
                    Ok(command) => {
                        let mut buffer = BufWriter::new(stream);
                        command.run(&mut buffer, &mut head_config_tx);
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

fn main() {
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

            let (wlr_tx, wlr_rx) = mpsc::channel::<HashMap<ObjectId, MonitorInformation>>();

            let (head_config_tx, head_config_rx) = mpsc::channel::<Vec<(ObjectId, SwayMonitor)>>();

            let head_config_command_tx = head_config_tx.clone();

            let wlr_output_updates_blocking = std::thread::spawn(|| {
                wlr_output_state::wayland_event_loop(wlr_tx, head_config_rx);
            });
            let commmand_listener_task = std::thread::spawn(|| {
                command_listener(head_config_command_tx);
            });
            let connected_monitors_handler =
                std::thread::spawn(|| connected_monitor_listen(wlr_rx, head_config_tx));

            let _ = wlr_output_updates_blocking.join();
            let _ = connected_monitors_handler.join();
            let _ = commmand_listener_task.join();
        }
    }
}

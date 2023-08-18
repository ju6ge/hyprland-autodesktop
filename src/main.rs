use std::{path::Path,
          collections::HashMap,
          env,
          os::unix::net::{UnixStream, UnixListener},
          io::{Write, Read, BufReader, BufRead, BufWriter},
          sync::{Arc, RwLock}
};
use clap::Parser;
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
    head_state: HashMap<ObjectId, MonitorInformation>
}

impl Default for DaemonState {
    fn default() -> Self {
        Self { head_state: HashMap::new() }
    }
}

async fn connected_monitor_listen(mut wlr_rx: UnboundedReceiver<HashMap<ObjectId, MonitorInformation>>) {
    while let Some(current_connected_monitors) = wlr_rx.recv().await {
        let _ = DAEMON_STATE.clone().write().and_then(|mut daemon_state| {
            daemon_state.head_state = current_connected_monitors;
            Ok(())
        });
    }
}

#[derive(Debug, Parser)]
struct Options {
    #[command(subcommand)]
    command: Option<Command>
}

#[derive(Debug, Parser, Clone, Serialize, Deserialize)]
enum Command {
    Attached,
    Apply,
    Force
}

impl Command {
    pub fn run(&self, buffer: &mut BufWriter<UnixStream>) {
        match self {
            Command::Attached => {
                DAEMON_STATE.read().and_then(|daemon_state| {
                    for (id, head) in  daemon_state.head_state.iter() {
                        println!("attached");
                        let _ = writeln!(buffer, "{}: {} {}\n", head.name(), head.make(), head.serial().as_ref().unwrap_or(&"".to_string()));
                        let _ = buffer.flush();
                    }
                    Ok(())
                });
            },
            Command::Apply => {},
            Command::Force => {},
        }
    }
}

fn command_listener() {
    UnixListener::bind(SOCKET_ADDR.as_str()).and_then(|socket_server| {
      for connection in socket_server.incoming() {
          connection.and_then(|mut stream| {
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
            UnixStream::connect(SOCKET_ADDR.as_str()).and_then(|mut socket_stream| {
                let _ = bincode::serialize(&command).and_then(|command_bin| {
                    let _ = socket_stream.write(&command_bin);
                    let _ = socket_stream.flush();
                    Ok(())
                });
                let buffer = BufReader::new(socket_stream);
                for line in buffer.lines() {
                    match line {
                        Ok(l) => println!("{l}"),
                        Err(_) => {},
                    }
                }
                Ok(())
            });
        }

        // programm running as deamon
        None => {
            if Path::new(SOCKET_ADDR.as_str()).exists() {
                println!("Deamon Process all ready running at {}! Exiting", SOCKET_ADDR.as_str());
                return;
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

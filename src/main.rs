use std::{path::{PathBuf, Path}, collections::{BTreeMap, HashMap}, fmt::format, env, net::SocketAddr, os::unix::net::{UnixStream, UnixListener}, io::{Write, Read}};

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

async fn connected_monitor_listen(mut wlr_rx: UnboundedReceiver<HashMap<ObjectId, MonitorInformation>>) {
    while let Some(current_connected_monitors) = wlr_rx.recv().await {
        println!("{current_connected_monitors:?}")
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

fn command_listener() {
    UnixListener::bind(SOCKET_ADDR.as_str()).and_then(|socket_server| {
      for connection in socket_server.incoming() {
          connection.and_then(|mut stream| {
              let mut message = Vec::new();
              let _ = stream.read_to_end(&mut message);
              bincode::deserialize::<Command>(&message).and_then(|command| {
                  println!("{command:#?}");
                  Ok(())
              });
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
                    let _ = socket_stream.write_all(&command_bin);
                    Ok(())
                });
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

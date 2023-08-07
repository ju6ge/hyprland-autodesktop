use std::path::PathBuf;

use clap::Parser;
use serde::{Serialize, Deserialize};
use wayland_client::{Connection, Dispatch, protocol::{wl_registry, wl_display::WlDisplay}, Proxy, event_created_child};
use wayland_protocols_wlr::output_management::v1::client::{*, zwlr_output_configuration_v1::ZwlrOutputConfigurationV1};


#[derive(Serialize,Deserialize)]
enum ScreenRotation {
    Landscape,
    Portrait,
    LandscapeReversed,
    PortraitReversed
}

#[derive(Serialize,Deserialize)]
struct ScreenConfiguration {
    position: (u64, u64),
    size: (u64, u64),
    scale: f32,
    rotation: ScreenRotation,
    display_output_code: u32,
    wallpaper: PathBuf
}

#[derive(Serialize,Deserialize)]
struct ScreenSetup {
    screens: Vec<ScreenConfiguration>
}

#[derive(Debug, Parser)]
enum ApplyCommands {
    Apply,
    Force
}

struct ScreenManagerState {
    running: bool,
    display: WlDisplay,
    output_manager: Option<zwlr_output_manager_v1::ZwlrOutputManagerV1>
}

impl ScreenManagerState {
    pub fn new(display: WlDisplay) -> Self {
        Self {
            running: true,
            display: display,
            output_manager: None
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for ScreenManagerState {
    fn event(
        state: &mut Self,
        proxy: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global { name, interface, version } => {
                if interface == zwlr_output_manager_v1::ZwlrOutputManagerV1::interface().name {
                    state.output_manager = Some(proxy.bind(name, version, qh, *_data));
                }
            },
            wl_registry::Event::GlobalRemove { name: _ } => { /* Nothing to do here */ },
            _ => { /* Nothing to do here */ },
        }
    }
}

impl Dispatch<zwlr_output_manager_v1::ZwlrOutputManagerV1, ()> for ScreenManagerState {
    fn event(
        state: &mut Self,
        proxy: &zwlr_output_manager_v1::ZwlrOutputManagerV1,
        event: <zwlr_output_manager_v1::ZwlrOutputManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_manager_v1::Event::Head { head } => todo!(),
            zwlr_output_manager_v1::Event::Done { serial } => todo!(),
            zwlr_output_manager_v1::Event::Finished => todo!(),
            _ => todo!(),
        }
    }

    event_created_child!(ScreenManagerState, zwlr_output_manager_v1::ZwlrOutputManagerV1, [
        0 => (ZwlrOutputConfigurationV1, ())
    ]);
}

impl Dispatch<zwlr_output_configuration_v1::ZwlrOutputConfigurationV1, ()> for ScreenManagerState {
    fn event(
        state: &mut Self,
        proxy: &zwlr_output_configuration_v1::ZwlrOutputConfigurationV1,
        event: <zwlr_output_configuration_v1::ZwlrOutputConfigurationV1 as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        println!("{event:#?}")
    }
}

impl Dispatch<zwlr_output_head_v1::ZwlrOutputHeadV1, ()> for ScreenManagerState {
    fn event(
        state: &mut Self,
        proxy: &zwlr_output_head_v1::ZwlrOutputHeadV1,
        event: <zwlr_output_head_v1::ZwlrOutputHeadV1 as Proxy>::Event,
        data: &(),
        conn: &Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        println!("{event:#?}")
    }
}


fn main() {
    let conn = Connection::connect_to_env().expect("Error connection to wayland session! Are you sure you are using a wayland based window manager?");

    let display = conn.display();

    let mut wl_events = conn.new_event_queue();
    let qh = wl_events.handle();

    let _registry = display.get_registry(&qh, ());

    let mut state = ScreenManagerState::new(display);

    while state.running {
        wl_events.blocking_dispatch(&mut state).unwrap();
    }
}

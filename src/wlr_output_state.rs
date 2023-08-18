use std::collections::HashMap;

use derive_builder::Builder;
use derive_getters::Getters;
use tokio::sync::mpsc::UnboundedSender;
use wayland_client::{Connection, Dispatch, protocol::{wl_registry, wl_display::WlDisplay, wl_output::Transform}, Proxy, event_created_child, backend::ObjectId};
use wayland_protocols_wlr::output_management::v1::client::{*, zwlr_output_head_v1::{ZwlrOutputHeadV1, AdaptiveSyncState}, zwlr_output_mode_v1::ZwlrOutputModeV1};

#[derive(Builder, Debug, Clone, Getters)]
pub struct MonitorMode {
    #[builder(setter(into))]
    id: ObjectId,
    #[builder(setter(into))]
    size: (i32, i32),
    #[builder(setter(into))]
    refresh: f64,
    #[builder(setter(into), default)]
    preferred: bool,
}

#[derive(Builder, Debug, Clone, Getters)]
pub struct MonitorInformation {
    #[builder(setter(into))]
    id: ObjectId,
    #[builder(setter(into))]
    name: String,
    #[builder(setter(into))]
    model: String,
    #[builder(setter(into))]
    make: String,
    #[builder(setter(into))]
    description: String,
    #[builder(setter(into))]
    size: (i32, i32),
    #[builder(setter(into))]
    position: (i32, i32),
    #[builder(setter(into))]
    enabled: i32,
    #[builder(setter(into))]
    transform: Transform,
    #[builder(setter(into))]
    scale: f64,
    #[builder(setter(into), default)]
    serial: Option<String>,
    #[builder(setter(into))]
    adaptive_sync: Option<AdaptiveSyncState>,
    #[builder(setter(into))]
    current_mode: ObjectId,
    #[builder(setter(into), default)]
    modes: Vec<MonitorMode>,
}

impl MonitorInformationBuilder {
    pub fn add_mode(&mut self, mode: MonitorMode) -> &mut Self {
        if let Some(ref mut modes) = self.modes.as_mut() {
            modes.push(mode)
        } else {
            let mut modes = Vec::new();
            modes.push(mode);
            self.modes = Some(modes);
        }
        self
    }

    pub fn from_value(monitor_information: &MonitorInformation) -> Self {
        Self {
            id: Some(monitor_information.id().clone()),
            name: Some(monitor_information.name.clone()),
            model: Some(monitor_information.model.clone()),
            make: Some(monitor_information.make.clone()),
            description: Some(monitor_information.description.clone()),
            size: Some(monitor_information.size),
            position: Some(monitor_information.position),
            enabled: Some(monitor_information.enabled),
            transform: Some(monitor_information.transform),
            scale: Some(monitor_information.scale),
            serial: Some(monitor_information.serial.clone()),
            adaptive_sync: Some(monitor_information.adaptive_sync),
            current_mode: Some(monitor_information.current_mode.clone()),
            modes: Some(monitor_information.modes.clone())
        }
    }
}

struct ScreenManagerState {
    running: bool,
    _display: WlDisplay,
    output_manager: Option<zwlr_output_manager_v1::ZwlrOutputManagerV1>,
    wlr_tx: UnboundedSender<HashMap<ObjectId, MonitorInformation>>,
    current_head: Option<MonitorInformationBuilder>,
    current_mode: Option<MonitorModeBuilder>,
    current_configuration: HashMap<ObjectId, MonitorInformation>
}

impl ScreenManagerState {
    pub fn new(display: WlDisplay, wlr_tx: UnboundedSender<HashMap<ObjectId, MonitorInformation>>) -> Self {
        Self {
            running: true,
            _display: display,
            output_manager: None,
            wlr_tx: wlr_tx,
            current_head: None,
            current_mode: None,
            current_configuration: HashMap::new()
        }
    }
}

impl ScreenManagerState {
    pub fn create_new_head(&mut self, id: ObjectId) {
        if self.current_head.is_some() {
            self.finish_head();
        }
        let mut builder = match self.current_configuration.get(&id) {
            Some(mi) => {
                MonitorInformationBuilder::from_value(mi)
            },
            None => {
                MonitorInformationBuilder::default()
            }
        };
        builder.id(id);
        self.current_head = Some(builder);
    }

    pub fn create_new_mode(&mut self, id: ObjectId) {
        if self.current_mode.is_some() {
            self.finish_mode();
        }
        let mut builder = MonitorModeBuilder::default();
        builder.id(id);
        self.current_mode = Some(builder);
    }
    pub fn finish_mode(&mut self) {
        self.current_mode.take().and_then(|mb|{
           mb.build().and_then(|m| {
               if let Some(ref mut current_head) = self.current_head.as_mut() {
                   current_head.add_mode(m);
               }
               Ok(())
           }).ok()
        });
    }

    pub fn finish_head(&mut self) {
        self.finish_mode();
        self.current_head.take().and_then(|hb|{
           hb.build().and_then(|h| {
               self.current_configuration.insert(h.id().clone(), h);
               Ok(())
           }).ok()
        });
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
        _proxy: &zwlr_output_manager_v1::ZwlrOutputManagerV1,
        event: <zwlr_output_manager_v1::ZwlrOutputManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_manager_v1::Event::Head { head } => {
                state.create_new_head(head.id());
            },
            zwlr_output_manager_v1::Event::Done { serial: _ } => {
                state.finish_head();

                state.wlr_tx.send(state.current_configuration.clone());
            },
            zwlr_output_manager_v1::Event::Finished => {
                println!("=========================================================\nFinished")
            },
            _ => { /* Nothing to do here */ },
        }
    }

    event_created_child!(ScreenManagerState, zwlr_output_head_v1::ZwlrOutputHeadV1, [
        0 => (ZwlrOutputHeadV1, ())
    ]);
}

impl Dispatch<zwlr_output_head_v1::ZwlrOutputHeadV1, ()> for ScreenManagerState {
    fn event(
        app_state: &mut Self,
        _proxy: &zwlr_output_head_v1::ZwlrOutputHeadV1,
        event: <zwlr_output_head_v1::ZwlrOutputHeadV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        //println!("{event:#?}");
        match event {
            zwlr_output_head_v1::Event::Name { name } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.name(name);
                }
            },
            zwlr_output_head_v1::Event::Description { description } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.description(description);
                }
            },
            zwlr_output_head_v1::Event::PhysicalSize { width, height } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.size((width, height));
                }
            },
            zwlr_output_head_v1::Event::Mode { mode } => {
                app_state.create_new_mode(mode.id());
            },
            zwlr_output_head_v1::Event::CurrentMode { mode } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.current_mode(mode.id());
                }
            },
            zwlr_output_head_v1::Event::Enabled { enabled } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.enabled(enabled);
                }
            },
            zwlr_output_head_v1::Event::Position { x, y } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.position((x,y));
                }
            },
            zwlr_output_head_v1::Event::Transform { transform } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    match transform {
                        wayland_client::WEnum::Value(transform) => {
                            builder.transform(transform);
                        },
                        wayland_client::WEnum::Unknown(_) => { /* unknown nothing to do here */ },
                    }
                }
            },
            zwlr_output_head_v1::Event::Scale { scale } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.scale(scale);
                }
            },
            zwlr_output_head_v1::Event::Finished => {
                println!("===================================\nFinished")
            },
            zwlr_output_head_v1::Event::Make { make } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.make(make);
                }
            },
            zwlr_output_head_v1::Event::Model { model } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.model(model);
                }
            },
            zwlr_output_head_v1::Event::SerialNumber { serial_number } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.serial(serial_number);
                }
            },
            zwlr_output_head_v1::Event::AdaptiveSync { state } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    match state {
                        wayland_client::WEnum::Value(state) => {
                            builder.adaptive_sync(state);
                        },
                        wayland_client::WEnum::Unknown(_) => { /* unknow nothing to do here */ },
                    }
                }
            },
            _ => {},
        }
    }

    event_created_child!(ScreenManagerState, zwlr_output_mode_v1::ZwlrOutputModeV1, [
        3 => (ZwlrOutputModeV1, ())
    ]);
}

impl Dispatch<zwlr_output_mode_v1::ZwlrOutputModeV1, ()>  for ScreenManagerState {
    fn event(
        app_state: &mut Self,
        _proxy: &zwlr_output_mode_v1::ZwlrOutputModeV1,
        event: <zwlr_output_mode_v1::ZwlrOutputModeV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_mode_v1::Event::Size { width, height } => {
                if let Some(ref mut builder) = app_state.current_mode.as_mut() {
                   builder.size((width, height));
                }
            },
            zwlr_output_mode_v1::Event::Refresh { refresh } => {
                if let Some(ref mut builder) = app_state.current_mode.as_mut() {
                   builder.refresh(refresh);
                }
            },
            zwlr_output_mode_v1::Event::Preferred => {
                if let Some(ref mut builder) = app_state.current_mode.as_mut() {
                   builder.preferred(true);
                }
            },
            zwlr_output_mode_v1::Event::Finished => {
                println!("============================================\nFinished");
            },
            _ => { /* Nothing to do here */ },
        }
    }
}

pub fn wayland_event_loop(wlr_tx: UnboundedSender<HashMap<ObjectId, MonitorInformation>>) {
    let conn = Connection::connect_to_env().expect("Error connection to wayland session! Are you sure you are using a wayland based window manager?");

    let display = conn.display();

    let mut wl_events = conn.new_event_queue();
    let qh = wl_events.handle();

    let _registry = display.get_registry(&qh, ());

    let mut state = ScreenManagerState::new(display, wlr_tx);

    while state.running {
        wl_events.blocking_dispatch(&mut state);
    }
}

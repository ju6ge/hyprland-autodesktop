use std::collections::HashMap;

use derive_builder::Builder;
use derive_getters::Getters;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use wayland_client::{
    backend::ObjectId,
    event_created_child,
    protocol::{wl_display::WlDisplay, wl_output::Transform, wl_registry},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols_wlr::output_management::{
    self,
    v1::client::{
        zwlr_output_head_v1::{AdaptiveSyncState, ZwlrOutputHeadV1},
        zwlr_output_mode_v1::ZwlrOutputModeV1,
        *,
    },
};

use crate::configuration::SwayMonitor;

#[derive(Builder, Debug, Clone, Getters)]
#[allow(dead_code)]
pub struct MonitorMode {
    #[builder(setter(into))]
    mode: ZwlrOutputModeV1,
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
    head: ZwlrOutputHeadV1,
    #[builder(setter(into), default)]
    name: String,
    #[builder(setter(into), default)]
    model: String,
    #[builder(setter(into), default)]
    make: String,
    #[builder(setter(into), default)]
    description: String,
    #[builder(setter(into), default)]
    size: (i32, i32),
    #[builder(setter(into), default)]
    position: (i32, i32),
    #[builder(setter(into), default)]
    enabled: i32,
    #[builder(setter(into))]
    transform: Transform,
    #[builder(setter(into), default)]
    scale: f64,
    #[builder(setter(into), default)]
    serial: Option<String>,
    #[builder(setter(into), default)]
    adaptive_sync: Option<AdaptiveSyncState>,
    #[builder(setter(into))]
    current_mode: ObjectId,
    #[builder(setter(into), default)]
    modes: Vec<MonitorMode>,
}

impl MonitorInformation {
    pub fn preffered_mode(&self) -> &MonitorMode {
        for mode in &self.modes {
            if *mode.preferred() {
                return mode;
            }
        }
        // if no mode is preffered return the first one
        &self.modes[0]
    }

    #[allow(dead_code)]
    pub fn biggest_mode(&self) -> &MonitorMode {
        let mut biggest_mode: &MonitorMode = &self.modes[0];
        for mode in &self.modes {
            if biggest_mode.size().0 < mode.size().0 {
                biggest_mode = mode;
            }
            if biggest_mode.size().0 == mode.size().0 && biggest_mode.size().1 > mode.size().1 {
                biggest_mode = mode;
            }
        }
        biggest_mode
    }
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
            head: Some(monitor_information.head().clone()),
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
            modes: Some(monitor_information.modes.clone()),
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
    current_configuration: HashMap<ObjectId, MonitorInformation>,
}

impl ScreenManagerState {
    pub fn new(
        display: WlDisplay,
        wlr_tx: UnboundedSender<HashMap<ObjectId, MonitorInformation>>,
    ) -> Self {
        Self {
            running: true,
            _display: display,
            output_manager: None,
            wlr_tx,
            current_head: None,
            current_mode: None,
            current_configuration: HashMap::new(),
        }
    }

    pub fn update_head_configuration(
        &mut self,
        monitors: Vec<(ObjectId, SwayMonitor)>,
        qh: &QueueHandle<Self>,
    ) -> () {
        println!("{monitors:#?}");
        println!("{:#?}", self.current_configuration);
        if let Some(ref mut output_management) = self.output_manager {
            for (id, desired_config) in monitors {
                if let Some(matching_head) = self.current_configuration.get(&id) {
                    if desired_config.enabled {
                        //TODO check the id here?
                        let config = output_management
                            .create_configuration(0, qh, ())
                            .enable_head(&matching_head.head, qh, ());
                        config.set_position(desired_config.pos_x, desired_config.pos_y);
                        config.set_scale(desired_config.scale);
                        config.set_transform(desired_config.rotation.into());
                    } else {
                        output_management
                            .create_configuration(0, qh, ())
                            .disable_head(&matching_head.head);
                    }
                }
            }
        }
    }
}

impl ScreenManagerState {
    pub fn create_new_head(&mut self, head: ZwlrOutputHeadV1) {
        if self.current_head.is_some() {
            self.finish_head();
        }
        let mut builder = match self.current_configuration.get(&head.id()) {
            Some(mi) => MonitorInformationBuilder::from_value(mi),
            None => MonitorInformationBuilder::default(),
        };
        builder.head(head);
        self.current_head = Some(builder);
    }

    pub fn create_new_mode(&mut self, mode: ZwlrOutputModeV1) {
        if self.current_mode.is_some() {
            self.finish_mode();
        }
        let mut builder = MonitorModeBuilder::default();
        builder.mode(mode);
        self.current_mode = Some(builder);
    }

    pub fn finish_mode(&mut self) {
        self.current_mode.take().and_then(|mb| {
            mb.build()
                .and_then(|m| {
                    if let Some(ref mut current_head) = self.current_head.as_mut() {
                        current_head.add_mode(m);
                    }
                    Ok(())
                })
                .ok()
        });
    }

    pub fn finish_head(&mut self) {
        self.finish_mode();
        self.current_head.take().and_then(|hb| {
            hb.build()
                .and_then(|h| {
                    self.current_configuration.insert(h.head().id(), h);
                    Ok(())
                })
                .map_err(|err| println!("{err:#?}"))
                .ok()
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
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                if interface == zwlr_output_manager_v1::ZwlrOutputManagerV1::interface().name {
                    state.output_manager = Some(proxy.bind(name, version, qh, *_data));
                }
            }
            wl_registry::Event::GlobalRemove { name: _ } => { /* Nothing to do here */ }
            _ => { /* Nothing to do here */ }
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
                state.create_new_head(head);
            }
            zwlr_output_manager_v1::Event::Done { serial: _ } => {
                state.finish_head();

                let _ = state.wlr_tx.send(state.current_configuration.clone());
            }
            zwlr_output_manager_v1::Event::Finished => {}
            _ => { /* Nothing to do here */ }
        }
    }

    event_created_child!(ScreenManagerState, zwlr_output_head_v1::ZwlrOutputHeadV1, [
        0 => (ZwlrOutputHeadV1, ())
    ]);
}

impl Dispatch<zwlr_output_head_v1::ZwlrOutputHeadV1, ()> for ScreenManagerState {
    fn event(
        app_state: &mut Self,
        head: &zwlr_output_head_v1::ZwlrOutputHeadV1,
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
            }
            zwlr_output_head_v1::Event::Description { description } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.description(description);
                }
            }
            zwlr_output_head_v1::Event::PhysicalSize { width, height } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.size((width, height));
                }
            }
            zwlr_output_head_v1::Event::Mode { mode } => {
                app_state.create_new_mode(mode);
            }
            zwlr_output_head_v1::Event::CurrentMode { mode } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.current_mode(mode.id());
                }
            }
            zwlr_output_head_v1::Event::Enabled { enabled } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.enabled(enabled);
                }
            }
            zwlr_output_head_v1::Event::Position { x, y } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.position((x, y));
                }
            }
            zwlr_output_head_v1::Event::Transform { transform } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    match transform {
                        wayland_client::WEnum::Value(transform) => {
                            builder.transform(transform);
                        }
                        wayland_client::WEnum::Unknown(_) => { /* unknown nothing to do here */ }
                    }
                }
            }
            zwlr_output_head_v1::Event::Scale { scale } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.scale(scale);
                }
            }
            zwlr_output_head_v1::Event::Make { make } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.make(make);
                }
            }
            zwlr_output_head_v1::Event::Model { model } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.model(model);
                }
            }
            zwlr_output_head_v1::Event::SerialNumber { serial_number } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    builder.serial(serial_number);
                }
            }
            zwlr_output_head_v1::Event::AdaptiveSync { state } => {
                if let Some(ref mut builder) = app_state.current_head.as_mut() {
                    match state {
                        wayland_client::WEnum::Value(state) => {
                            builder.adaptive_sync(state);
                        }
                        wayland_client::WEnum::Unknown(_) => { /* unknow nothing to do here */ }
                    }
                }
            }
            zwlr_output_head_v1::Event::Finished => {
                app_state.current_configuration.remove(&head.id());
                let _ = app_state
                    .wlr_tx
                    .send(app_state.current_configuration.clone());
            }
            _ => {}
        }
    }

    event_created_child!(ScreenManagerState, zwlr_output_mode_v1::ZwlrOutputModeV1, [
        3 => (ZwlrOutputModeV1, ())
    ]);
}

impl Dispatch<zwlr_output_configuration_head_v1::ZwlrOutputConfigurationHeadV1, ()>
    for ScreenManagerState
{
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_output_configuration_head_v1::ZwlrOutputConfigurationHeadV1,
        event: <zwlr_output_configuration_head_v1::ZwlrOutputConfigurationHeadV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            //TODO figure out if this is useful for anything?
            _ => { /* nothing to see here */ }
        }
    }
}

impl Dispatch<zwlr_output_configuration_v1::ZwlrOutputConfigurationV1, ()> for ScreenManagerState {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_output_configuration_v1::ZwlrOutputConfigurationV1,
        event: <zwlr_output_configuration_v1::ZwlrOutputConfigurationV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        // TODO should i do something with these events?
        match event {
            zwlr_output_configuration_v1::Event::Succeeded => { /*nothing done here yet*/ }
            zwlr_output_configuration_v1::Event::Failed => { /*nothing done here yet*/ }
            zwlr_output_configuration_v1::Event::Cancelled => { /*nothing done here yet*/ }
            _ => {
                unimplemented!("propbaly an unknown future event has occured and needs a handler!")
            }
        }
    }
}

impl Dispatch<zwlr_output_mode_v1::ZwlrOutputModeV1, ()> for ScreenManagerState {
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
            }
            zwlr_output_mode_v1::Event::Refresh { refresh } => {
                if let Some(ref mut builder) = app_state.current_mode.as_mut() {
                    builder.refresh(refresh);
                }
            }
            zwlr_output_mode_v1::Event::Preferred => {
                if let Some(ref mut builder) = app_state.current_mode.as_mut() {
                    builder.preferred(true);
                }
            }
            zwlr_output_mode_v1::Event::Finished => {
                //println!("============================================\nFinished");
            }
            _ => { /* Nothing to do here */ }
        }
    }
}

pub fn wayland_event_loop(
    wlr_tx: UnboundedSender<HashMap<ObjectId, MonitorInformation>>,
    mut config_head_rx: UnboundedReceiver<Vec<(ObjectId, SwayMonitor)>>,
) {
    let conn = Connection::connect_to_env().expect("Error connection to wayland session! Are you sure you are using a wayland based window manager?");

    let display = conn.display();

    let mut wl_events = conn.new_event_queue();
    let qh = wl_events.handle();

    let _registry = display.get_registry(&qh, ());

    let mut state = ScreenManagerState::new(display, wlr_tx);

    while state.running {
        let _ = wl_events.blocking_dispatch(&mut state);
        if let Ok(update_head_event) = config_head_rx.try_recv() {
            state.update_head_configuration(update_head_event, &qh);
        }
    }
}

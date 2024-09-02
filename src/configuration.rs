use derive_getters::Getters;
use hyprland::dispatch::{Dispatch, DispatchType};
use id_tree::{Node, NodeId, Tree, TreeBuilder};
use libmonitor::mccs::features::InputSource;
use libmonitor::{ddc::DdcDevice, Monitor};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    path::{Path, PathBuf},
    process::Command,
};
use wayland_client::backend::ObjectId;

use crate::{ddc::MonitorInputSourceMatcher, wlr_output_state::MonitorInformation};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ScreenRotation {
    Landscape,
    LandscapeReversed,
    Portrait,
    PortraitReversed,
}

impl ScreenRotation {
    pub fn transform_size(&self, size: (i32, i32)) -> (i32, i32) {
        match self {
            ScreenRotation::Landscape | ScreenRotation::LandscapeReversed => size,
            ScreenRotation::Portrait | ScreenRotation::PortraitReversed => (size.1, size.0),
        }
    }

    pub fn transform_id(&self) -> u8 {
        match self {
            ScreenRotation::Landscape => 0,
            ScreenRotation::LandscapeReversed => 2,
            ScreenRotation::Portrait => 1,
            ScreenRotation::PortraitReversed => 3,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ScreenPositionRelative {
    Root,
    Over(String),
    Under(String),
    Left(String),
    Right(String),
    LeftOver(String),
    LeftUnder(String),
    RightOver(String),
    RightUnder(String),
    Mirror(String),
}

impl ScreenPositionRelative {
    pub fn parent(&self) -> Option<&str> {
        match self {
            ScreenPositionRelative::Root => None,
            ScreenPositionRelative::Over(identifer)
            | ScreenPositionRelative::Under(identifer)
            | ScreenPositionRelative::Left(identifer)
            | ScreenPositionRelative::Right(identifer)
            | ScreenPositionRelative::LeftOver(identifer)
            | ScreenPositionRelative::LeftUnder(identifer)
            | ScreenPositionRelative::RightOver(identifer)
            | ScreenPositionRelative::RightUnder(identifer)
            | ScreenPositionRelative::Mirror(identifer) => Some(&identifer),
        }
    }

    pub fn offset(&self, parent_size: (i32, i32), own_size: (i32, i32)) -> (i32, i32) {
        match self {
            ScreenPositionRelative::Root => (0, 0),
            ScreenPositionRelative::Over(_) => (0, -1 * own_size.1),
            ScreenPositionRelative::Under(_) => (0, parent_size.1),
            ScreenPositionRelative::Left(_) => (-1 * own_size.0, 0),
            ScreenPositionRelative::Right(_) => (parent_size.0, 0),
            ScreenPositionRelative::LeftOver(_) => (-1 * own_size.0, -1 * own_size.1),
            ScreenPositionRelative::LeftUnder(_) => (-1 * own_size.0, parent_size.1),
            ScreenPositionRelative::RightOver(_) => (parent_size.0, -1 * own_size.1),
            ScreenPositionRelative::RightUnder(_) => (parent_size.0, parent_size.1),
            ScreenPositionRelative::Mirror(_) => (0, 0),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct ScreenConfiguration {
    identifier: String,
    scale: f32,
    rotation: ScreenRotation,
    #[serde(default)]
    display_output_code: MonitorInputSourceMatcher,
    wallpaper: PathBuf,
    position: ScreenPositionRelative,
    #[serde(default)]
    workspaces: Vec<u8>,
    enabled: bool,
}

// collect settings required to configure hyprland
pub struct SwayMonitor {
    pub mirror: Option<String>,
    pub enabled: bool,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub fps: f64,
    pub pos_x: i32,
    pub pos_y: i32,
    pub scale: f32,
    pub rotation: u8,
    pub workspaces: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct ScreensProfile {
    screens: Vec<ScreenConfiguration>,
    #[serde(default)]
    skripts: Vec<String>,
}

impl ScreensProfile {
    /// check if a profile matches the current screens connected to the device
    pub fn is_connected(
        &self,
        head_config: &HashMap<ObjectId, MonitorInformation>,
        current_monitor_inputs: &BTreeMap<String, InputSource>,
    ) -> bool {
        let mut connected = true;
        for screen in &self.screens {
            let mut screen_found = false;
            for (_id, monitor_info) in head_config.iter() {
                if screen.identifier() == monitor_info.name()
                    || screen.identifier()
                        == &format!(
                            "{} {}",
                            monitor_info.make(),
                            monitor_info.serial().as_ref().unwrap_or(&"".to_string())
                        )
                {
                    if let Some(source) = current_monitor_inputs.get(monitor_info.name()) {
                        // if we have information about the current monitor selected input
                        // then only consider it connected if the profiles input matches
                        // the currently active one
                        if screen.display_output_code().matches(*source) {
                            screen_found = true;
                            break;
                        }
                    } else {
                        // if we do not have information about the current monitor input assume monitor is configured
                        // to display the device
                        screen_found = true;
                        break;
                    }
                }
            }
            if !screen_found {
                connected = false;
                break;
            }
        }
        connected
    }

    /// calculate profile weight, this is a value that describes how good the match of a profile is if it is found to be connected
    /// the higher the value, the higher the requirements for the profile to match, hense if it matches it should be selected of other
    /// profiles that also match but do not have as much weight.
    pub fn weight(&self) -> usize {
        let mut weight = self.screens().len(); // start with the amount of screens that need to match as a baseline value
        for screen in self.screens() {
            // if the screen has a requirement to match agains a specific monitor input the weight needs to be increased
            if *screen.display_output_code() != MonitorInputSourceMatcher::Any {
                weight += 1;
            }
        }
        weight
    }

    pub fn apply(
        &self,
        head_config: &HashMap<ObjectId, MonitorInformation>,
    ) {
        // match connected monitor information with profile monitor configuration
        let mut monitor_map: BTreeMap<&str, (&ScreenConfiguration, &MonitorInformation)> =
            BTreeMap::new();
        for screen in &self.screens {
            for (_id, monitor_info) in head_config.iter() {
                if screen.identifier() == monitor_info.name()
                    || screen.identifier()
                        == &format!(
                            "{} {}",
                            monitor_info.make(),
                            monitor_info.serial().as_ref().unwrap_or(&"".to_string())
                        )
                {
                    monitor_map.insert(screen.identifier(), (screen, monitor_info));
                    if let Some(mut monitor_device) =
                        Monitor::enumerate().find(|mon| *monitor_info.name() == mon.handle.name())
                    {
                        match screen.display_output_code() {
                            MonitorInputSourceMatcher::Any => { /* nothing to do here */ }
                            MonitorInputSourceMatcher::Input(sould_be_input) => {
                                // if applied profile monitor config specifies a monitor input
                                // make sure it is configured correctly!
                                let _ =
                                    monitor_device.get_input_source().and_then(|current_input| {
                                        if current_input != *sould_be_input {
                                            let _ =
                                                monitor_device.set_input_source(*sould_be_input);
                                        }
                                        Ok(())
                                    });
                            }
                        }
                    }
                }
            }
        }

        // build tree of attached displays
        let mut position_tree = TreeBuilder::new().with_root(Node::new("Root")).build();
        let mut already_added: Vec<&str> = Vec::new();
        for (ident, (_conf, _info)) in monitor_map.iter() {
            add_node_to_tree(ident, &mut position_tree, &monitor_map, &mut already_added);
        }


        let mut sway_monitors = Vec::new();
        for (ident, (conf, info)) in monitor_map.iter() {
            let position = calc_screen_pixel_positon(ident, &position_tree, &monitor_map);
            sway_monitors.push(SwayMonitor {
                mirror: match conf.position() {
                    ScreenPositionRelative::Mirror(parent) => Some(parent.to_string()),
                    _ => None,
                },
                enabled: *conf.enabled(),
                name: info.name().to_string(),
                width: info.preffered_mode().size().0,
                height: info.preffered_mode().size().1,
                fps: info.preffered_mode().refresh() / 1000.,
                pos_x: position.0,
                pos_y: position.1,
                scale: *conf.scale(),
                rotation: conf.rotation().transform_id(),
                workspaces: conf.workspaces().clone(),
            });
        }

        // repostion montiors so that all coordinates are postive (why hyprland?)
        let min_pos_x = sway_monitors.iter().map(|hm| hm.pos_x).min().unwrap();
        let min_pos_y = sway_monitors.iter().map(|hm| hm.pos_y).min().unwrap();
        sway_monitors = sway_monitors
            .into_iter()
            .map(|mut hm| {
                hm.pos_x -= min_pos_x;
                hm.pos_y -= min_pos_y;
                hm
            })
            .collect();

        // write hyprland configuration file
        let mut moved_workspaces = Vec::new();
        for hm in sway_monitors {
            if hm.enabled {
                if let Some(parent) = hm.mirror {
                    //let _ = writeln!(
                        //&mut sway_monitor_config,
                        //"monitor={name},preferred,auto,1,mirror,{parent}",
                        //name = hm.name
                    //);
                } else {
                    //let _ = writeln!(&mut sway_monitor_config,
                            //"monitor={name},{width}x{height}@{fps},{pos_x}x{pos_y},{scale},transform,{rotation}",
                            //name = hm.name,
                            //width = hm.width,
                            //height = hm.height,
                            //fps = hm.fps,
                            //pos_x = hm.pos_x,
                            //pos_y = hm.pos_y,
                            //scale = hm.scale,
                            //rotation = hm.rotation
                    //);

                    for ws in &hm.workspaces {
                        if moved_workspaces.contains(ws) {
                            println!(
                                "Workspace {ws} already bound to different monitor! Ignoring …"
                            );
                        } else {
                            //TODO sway dispatch to move workspace
                            //let move_workspace = DispatchType::MoveWorkspaceToMonitor(
                                //hyprland::dispatch::WorkspaceIdentifier::Id((*ws).into()),
                                //hyprland::dispatch::MonitorIdentifier::Name(&hm.name),
                            //);
                            //let _ = Dispatch::call(move_workspace);
                            moved_workspaces.push(*ws);
                        }
                    }
                }
            } else {
                //writeln!(
                    //&mut sway_monitor_config,
                    //"monitor={name},disabled",
                    //name = hm.name
                //)
                //.unwrap();
            }
        }

        // run commands that where defined
        for cmd in &self.skripts {
            let args = cmd.split(' ').collect::<Vec<&str>>();
            let _out = Command::new(args[0]).args(&args[1..]).output().unwrap();
        }
    }
}

fn calc_screen_pixel_positon(
    ident: &str,
    position_tree: &Tree<&str>,
    monitor_map: &BTreeMap<&str, (&ScreenConfiguration, &MonitorInformation)>,
) -> (i32, i32) {
    let root_node_id = position_tree.root_node_id().unwrap();
    let current_node_id = find_nodeid_from_ident(ident, position_tree).unwrap();
    position_tree
        .get(&current_node_id)
        .and_then(|current_node| {
            if current_node.parent().unwrap() == root_node_id {
                // if multiple screens are attached to root then the profile is broken and the resulting configuration may look broken!
                Ok((0, 0))
            } else {
                let parent_ident = position_tree
                    .get(current_node.parent().unwrap())
                    .unwrap()
                    .data();
                let parent_position =
                    calc_screen_pixel_positon(&parent_ident, position_tree, monitor_map);
                let (conf, info) = monitor_map.get(ident).unwrap();
                let (parent_conf, parent_info) = monitor_map.get(parent_ident).unwrap();
                let parent_size = if parent_conf.enabled {
                    parent_conf
                        .rotation()
                        .transform_size(*parent_info.preffered_mode().size())
                } else {
                    (0, 0)
                };
                let own_size = if conf.enabled {
                    conf.rotation()
                        .transform_size(*info.preffered_mode().size())
                } else {
                    (0, 0)
                };
                let offset = conf.position().offset(parent_size, own_size);
                Ok((parent_position.0 + offset.0, parent_position.1 + offset.1))
            }
        })
        .unwrap()
}

fn find_nodeid_from_ident(ident: &str, position_tree: &Tree<&str>) -> Option<NodeId> {
    for node_id in position_tree
        .traverse_level_order_ids(position_tree.root_node_id().unwrap())
        .unwrap()
    {
        if position_tree.get(&node_id).unwrap().data() == &ident {
            return Some(node_id.clone());
        }
    }
    None
}

fn add_node_to_tree<'a>(
    ident: &'a str,
    position_tree: &mut Tree<&'a str>,
    monitor_map: &BTreeMap<&'a str, (&'a ScreenConfiguration, &'a MonitorInformation)>,
    already_added: &mut Vec<&'a str>,
) -> Option<NodeId> {
    // if monitor was already added do not add it again!
    if !already_added.contains(&ident) {
        monitor_map.get(&ident).and_then(|(conf, _info)| {
            let parent_ident = conf.position().parent();
            match parent_ident {
                Some(parent) => {
                    match monitor_map.get(parent) {
                        Some(_) => {
                            let parent_node_id =
                                add_node_to_tree(parent, position_tree, monitor_map, already_added)
                                    .unwrap();
                            let node = position_tree
                                .insert(
                                    Node::new(ident),
                                    id_tree::InsertBehavior::UnderNode(&parent_node_id),
                                )
                                .unwrap();
                            already_added.push(ident);
                            Some(node)
                        }
                        None => {
                            // if the parent indentifier is not found in the configuration then attach it to root
                            let node = position_tree
                                .insert(
                                    Node::new(ident),
                                    id_tree::InsertBehavior::UnderNode(
                                        &position_tree.root_node_id().unwrap().clone(),
                                    ),
                                )
                                .unwrap();
                            already_added.push(ident);
                            Some(node)
                        }
                    }
                }
                None => {
                    // No parent means this monitor is the Root display
                    let node = position_tree
                        .insert(
                            Node::new(ident),
                            id_tree::InsertBehavior::UnderNode(
                                &position_tree.root_node_id().unwrap().clone(),
                            ),
                        )
                        .unwrap();
                    already_added.push(ident);
                    Some(node)
                }
            }
        })
    } else {
        find_nodeid_from_ident(ident, position_tree)
    }
}

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct AppConfiguration {
    hyprland_config_file: PathBuf,
    profiles: BTreeMap<String, ScreensProfile>,
}

impl Default for AppConfiguration {
    fn default() -> Self {
        Self {
            hyprland_config_file: Path::new("~/.config/hypr/display.conf").into(),
            profiles: BTreeMap::new(),
        }
    }
}

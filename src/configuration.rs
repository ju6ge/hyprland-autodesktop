use std::{path::{PathBuf, Path}, collections::{BTreeMap, HashMap}, fs::File, process::Command};
use derive_getters::Getters;
use serde::{Serialize, Deserialize};
use id_tree::{TreeBuilder, Tree, Node, NodeId};
use std::io::Write;
use wayland_client::backend::ObjectId;

use crate::{wlr_output_state::MonitorInformation, ddc::{MonitorInputSource, DdcMonitor}};

#[derive(Serialize,Deserialize,Debug, Clone)]
pub enum ScreenRotation {
    Landscape,
    LandscapeReversed,
    Portrait,
    PortraitReversed
}

impl ScreenRotation {
    pub fn transform_size(&self, size: (i32, i32)) -> (i32, i32) {
        match self {
            ScreenRotation::Landscape |
            ScreenRotation::LandscapeReversed => {
               size
            },
            ScreenRotation::Portrait |
            ScreenRotation::PortraitReversed => {
                (size.1, size.0)
            },
        }
    }

    pub fn transform_id(&self) -> u8 {
        match self {
            ScreenRotation::Landscape  => {
                0
            },
            ScreenRotation::LandscapeReversed => {
                2
            },
            ScreenRotation::Portrait => {
                1
            },
            ScreenRotation::PortraitReversed => {
                3
            },
        }
    }
}

#[derive(Serialize,Deserialize,Debug, Clone)]
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
}

impl ScreenPositionRelative {
    pub fn parent(&self) -> Option<&str> {
       match self {
        ScreenPositionRelative::Root => None,
        ScreenPositionRelative::Over(identifer) |
        ScreenPositionRelative::Under(identifer) |
        ScreenPositionRelative::Left(identifer) |
        ScreenPositionRelative::Right(identifer) |
        ScreenPositionRelative::LeftOver(identifer) |
        ScreenPositionRelative::LeftUnder(identifer) |
        ScreenPositionRelative::RightOver(identifer) |
        ScreenPositionRelative::RightUnder(identifer) => {
            Some(&identifer)
        }
       }
    }

    pub fn offset(&self, parent_size: (i32, i32), own_size: (i32, i32)) -> (i32, i32) {
        match self {
            ScreenPositionRelative::Root => (0, 0),
            ScreenPositionRelative::Over(_) => (0, -1*own_size.1),
            ScreenPositionRelative::Under(_) => (0, parent_size.1),
            ScreenPositionRelative::Left(_) => (-1*own_size.0, 0),
            ScreenPositionRelative::Right(_) => (parent_size.0, 0),
            ScreenPositionRelative::LeftOver(_) => (-1*own_size.0, -1 * own_size.1),
            ScreenPositionRelative::LeftUnder(_) => (-1*own_size.0, parent_size.1),
            ScreenPositionRelative::RightOver(_) => (parent_size.0, -1 * own_size.1),
            ScreenPositionRelative::RightUnder(_) => (parent_size.0, parent_size.1),
        }
    }
}

#[derive(Serialize,Deserialize,Debug,Getters, Clone)]
pub struct ScreenConfiguration {
    identifier: String,
    scale: f32,
    rotation: ScreenRotation,
    display_output_code: Option<MonitorInputSource>,
    wallpaper: PathBuf,
    position: ScreenPositionRelative,
    enabled: bool
}

#[derive(Serialize,Deserialize,Debug,Getters, Clone)]
pub struct ScreensProfile {
    screens: Vec<ScreenConfiguration>,
    #[serde(default)]
    skripts: Vec<String>
}

impl ScreensProfile {
    pub fn is_connected(&self, head_config: &HashMap<ObjectId, MonitorInformation>, ddc_connections: &mut HashMap<ObjectId, DdcMonitor>) -> bool {
        let mut connected = true;
        for screen in &self.screens {
            let mut screen_found = false;
            for (id, monitor_info) in head_config.iter() {
                if screen.identifier() == monitor_info.name() || screen.identifier() ==  &format!("{} {}", monitor_info.make(), monitor_info.serial().as_ref().unwrap_or(&"".to_string())){
                    if let Some(ref mut display) = ddc_connections.get_mut(id) {
                        let current_source = display.get_input_source();
                        if current_source.is_some() && screen.display_output_code.is_some() {
                            let configured_source = screen.display_output_code.as_ref().unwrap();
                            let current_source = current_source.unwrap();
                            if current_source == *configured_source {
                                screen_found = true;
                                break;
                            }
                        } else {
                            // if no input source can be read assume monitor is set to correct input or if no input is configured for the screen
                            screen_found = true;
                            break;
                        }
                    } else {
                        // if no ddc connection exist to the monitor assume if is set to the correct input (this should only happen for laptop displays)
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

    pub fn apply(&self, head_config: &HashMap<ObjectId, MonitorInformation>, ddc_connections: &mut HashMap<ObjectId, DdcMonitor>, hyprland_config_file: &Path) {
        // match connected monitor information with profile monitor configuration
        let mut monitor_map: BTreeMap<&str, (&ScreenConfiguration, &MonitorInformation)> = BTreeMap::new();
        for screen in &self.screens {
            for (_id, monitor_info) in head_config.iter() {
                if screen.identifier() == monitor_info.name() || screen.identifier() ==  &format!("{} {}", monitor_info.make(), monitor_info.serial().as_ref().unwrap_or(&"".to_string())){
                    monitor_map.insert(screen.identifier(), (screen, monitor_info));
                }
            }
        }

        // build tree of attached displays
        let mut position_tree = TreeBuilder::new().with_root(Node::new("Root")).build();
        let mut already_added: Vec<&str> = Vec::new();
        for (ident, (_conf, _info)) in monitor_map.iter() {
            add_node_to_tree(ident, &mut position_tree, &monitor_map, &mut already_added);
        }
        
        // collect settings required to configure hyprland
        struct HyprlandMonitor {
            enabled: bool,
            name: String,
            width: i32,
            height: i32,
            fps: f64,
            pos_x: i32,
            pos_y: i32,
            scale: f32,
            rotation: u8
        }

        let mut hyprland_monitors = Vec::new();
        for (ident, (conf, info)) in monitor_map.iter() {
            let position = calc_screen_pixel_positon(ident, &position_tree, &monitor_map);
            hyprland_monitors.push(HyprlandMonitor{
                enabled: *conf.enabled(),
                name: info.name().to_string(),
                width: info.preffered_mode().size().0,
                height: info.preffered_mode().size().1,
                fps: info.preffered_mode().refresh()/1000.,
                pos_x: position.0,
                pos_y: position.1,
                scale: *conf.scale(),
                rotation: conf.rotation().transform_id(),
            });
        }
        
        // repostion montiors so that all coordinates are postive (why hyprland?)
        let min_pos_x = hyprland_monitors.iter().map(|hm| { hm.pos_x }).min().unwrap();
        let min_pos_y = hyprland_monitors.iter().map(|hm| { hm.pos_y }).min().unwrap();
        hyprland_monitors = hyprland_monitors.into_iter().map(|mut hm| {
            hm.pos_x -= min_pos_x;
            hm.pos_y -= min_pos_y;
            hm
        }).collect();

        // write hyprland configuration file
        let mut hyprland_monitor_config = File::create(hyprland_config_file).unwrap();
        for hm in hyprland_monitors {
            if hm.enabled {
                writeln!(&mut hyprland_monitor_config,
                        "monitor={name},{width}x{height}@{fps},{pos_x}x{pos_y},{scale},transform,{rotation}",
                        name = hm.name,
                        width = hm.width,
                        height = hm.height,
                        fps = hm.fps,
                        pos_x = hm.pos_x,
                        pos_y = hm.pos_y,
                        scale = hm.scale,
                        rotation = hm.rotation
                ).unwrap();
            } else {
                writeln!(&mut hyprland_monitor_config,
                            "monitor={name},disabled",
                            name = hm.name
                         ).unwrap();
            }
        }
        
        // run commands that where defined
        for cmd in &self.skripts {
            let args = cmd.split(' ').collect::<Vec<&str>>();
            let _out = Command::new(args[0]).args(&args[1..]).output().unwrap();
        }
    }
}


fn calc_screen_pixel_positon(ident: &str, position_tree: &Tree<&str>, monitor_map: &BTreeMap<&str, (&ScreenConfiguration, &MonitorInformation)>) -> (i32, i32){
    let root_node_id = position_tree.root_node_id().unwrap();
    let current_node_id = find_nodeid_from_ident(ident, position_tree).unwrap();
    position_tree.get(&current_node_id).and_then(|current_node| {
        if current_node.parent().unwrap() == root_node_id {
            // if multiple screens are attached to root then the profile is broken and the resulting configuration may look broken!
            Ok((0, 0))
        } else {
            let parent_ident = position_tree.get(current_node.parent().unwrap()).unwrap().data();
            let parent_position = calc_screen_pixel_positon(&parent_ident, position_tree, monitor_map);
            let (conf, info) = monitor_map.get(ident).unwrap();
            let (parent_conf, parent_info) = monitor_map.get(parent_ident).unwrap();
            let parent_size = if parent_conf.enabled { parent_conf.rotation().transform_size(*parent_info.preffered_mode().size()) } else { (0, 0) };
            let own_size = if conf.enabled { conf.rotation().transform_size(*info.preffered_mode().size()) } else { (0,0) };
            let offset = conf.position().offset(parent_size, own_size);
            Ok((parent_position.0 + offset.0, parent_position.1 + offset.1))
        }
    }).unwrap()
}

fn find_nodeid_from_ident(ident: &str, position_tree: &Tree<&str>) -> Option<NodeId> {
    for node_id in position_tree.traverse_level_order_ids(position_tree.root_node_id().unwrap()).unwrap() {
        if position_tree.get(&node_id).unwrap().data() == &ident {
            return Some(node_id.clone());
        }
    }
    None
}

fn add_node_to_tree<'a>(ident: &'a str, position_tree: &mut Tree<&'a str>, monitor_map: &BTreeMap<&'a str, (&'a ScreenConfiguration, &'a MonitorInformation)>, already_added: &mut Vec<&'a str>) -> Option<NodeId> {
    // if monitor was already added do not add it again!
    if !already_added.contains(&ident) {
        monitor_map.get(&ident).and_then(|(conf, _info)| {
            let parent_ident = conf.position().parent();
            match parent_ident {
                Some(parent) => {
                    match monitor_map.get(parent) {
                        Some(_) => {
                            let parent_node_id = add_node_to_tree(parent, position_tree, monitor_map, already_added).unwrap();
                            let node = position_tree.insert(Node::new(ident), id_tree::InsertBehavior::UnderNode(&parent_node_id)).unwrap();
                            already_added.push(ident);
                            Some(node)
                        },
                        None => {
                            // if the parent indentifier is not found in the configuration then attach it to root
                            let node = position_tree.insert(Node::new(ident), id_tree::InsertBehavior::UnderNode(&position_tree.root_node_id().unwrap().clone())).unwrap();
                            already_added.push(ident);
                            Some(node)
                        },
                    }
                },
                None => {
                    // No parent means this monitor is the Root display
                    let node = position_tree.insert(Node::new(ident), id_tree::InsertBehavior::UnderNode(&position_tree.root_node_id().unwrap().clone())).unwrap();
                    already_added.push(ident);
                    Some(node)
                },
            }
        })
    } else {
        find_nodeid_from_ident(ident, position_tree)
    }
}

#[derive(Serialize,Deserialize,Debug,Getters,Clone)]
pub struct AppConfiguration {
    hyprland_config_file: PathBuf,
    profiles: BTreeMap<String, ScreensProfile>
}

impl Default for AppConfiguration {
    fn default() -> Self {
        Self {
            hyprland_config_file: Path::new("~/.config/hypr/display.conf").into(),
            profiles: BTreeMap::new()
        }
    }
}


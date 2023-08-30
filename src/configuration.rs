use std::{path::{PathBuf, Path}, collections::{BTreeMap, HashMap}};
use derive_getters::Getters;
use serde::{Serialize, Deserialize};
use id_tree::{TreeBuilder, Tree, Node, NodeId};
use wayland_client::backend::ObjectId;

use crate::wlr_output_state::MonitorInformation;

#[derive(Serialize,Deserialize,Debug)]
pub enum ScreenRotation {
    Landscape,
    Portrait,
    LandscapeReversed,
    PortraitReversed
}

#[derive(Serialize,Deserialize,Debug)]
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
}

#[derive(Serialize,Deserialize,Debug, Getters)]
pub struct ScreenConfiguration {
    identifier: String,
    scale: f32,
    rotation: ScreenRotation,
    display_output_code: Option<u32>,
    wallpaper: PathBuf,
    position: ScreenPositionRelative,
    enabled: bool
}

#[derive(Serialize,Deserialize,Debug, Getters)]
pub struct ScreensProfile {
    screens: Vec<ScreenConfiguration>
}

impl ScreensProfile {
    pub fn is_connected(&self, head_config: &HashMap<ObjectId, MonitorInformation>) -> bool {
        let mut connected = true;
        for screen in &self.screens {
            let mut screen_found = false;
            for (_id, monitor_info) in head_config.iter() {
                if screen.identifier() == monitor_info.name() || screen.identifier() ==  &format!("{} {}", monitor_info.make(), monitor_info.serial().as_ref().unwrap_or(&"".to_string())){
                    screen_found = true;
                }
            }
            if !screen_found {
                connected = false;
                break;
            }
        }
        connected
    }

    pub fn apply(&self, head_config: &HashMap<ObjectId, MonitorInformation>, _hyprland_confi_file: &Path) {
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
        let mut display = String::new();
        position_tree.write_formatted(&mut display);
        println!("{display}");
    }
}

fn add_node_to_tree<'a>(ident: &'a str, position_tree: &mut Tree<&'a str>, monitor_map: &BTreeMap<&'a str, (&'a ScreenConfiguration, &'a MonitorInformation)>, already_added: &mut Vec<&'a str>) -> Option<NodeId> {
    // if monitor was already added do not add it again!
    if !already_added.contains(&ident) {
        monitor_map.get(&ident).and_then(|(conf, info)| {
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
        for node_id in position_tree.traverse_level_order_ids(position_tree.root_node_id().unwrap()).unwrap() {
            if position_tree.get(&node_id).unwrap().data() == &ident {
                return Some(node_id.clone());
            }
        }
        None
    }
}

#[derive(Serialize,Deserialize,Debug, Getters)]
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


use std::{path::{PathBuf, Path}, collections::{BTreeMap, HashMap}};
use derive_getters::Getters;
use serde::{Serialize, Deserialize};
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

    pub fn apply(&self, _head_config: &HashMap<ObjectId, MonitorInformation>, _hyprland_confi_file: &Path) {

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


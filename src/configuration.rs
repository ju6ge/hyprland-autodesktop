use std::{path::PathBuf, collections::BTreeMap};
use derive_getters::Getters;
use serde::{Serialize, Deserialize};

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
    position: ScreenPositionRelative
}

#[derive(Serialize,Deserialize,Debug, Getters)]
pub struct ScreensProfile {
    screens: Vec<ScreenConfiguration>
}

#[derive(Serialize,Deserialize,Debug, Getters)]
pub struct AppConfiguration {
    profiles: BTreeMap<String, ScreensProfile>
}

impl Default for AppConfiguration {
    fn default() -> Self {
        Self { profiles: BTreeMap::new() }
    }
}

#[cfg(test)]
mod test {

    use crate::configuration::ScreensProfile;

    use super::{ScreenConfiguration};

    #[test]
    fn serialize() {
        let x = ScreenConfiguration {
            identifier: "e-DP1".to_string(),
            scale: 1.0,
            rotation: super::ScreenRotation::Landscape,
            display_output_code: None,
            wallpaper: "/tmp/test.png".into(),
            position: super::ScreenPositionRelative::Left("e-DP1".to_string()),
        };
        let y = ScreensProfile {
            screens: vec![x]
        };
        println!("{}", serde_yaml::to_string(&y).unwrap());
    }
}

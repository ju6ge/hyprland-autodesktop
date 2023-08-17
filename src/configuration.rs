use std::{path::PathBuf, collections::BTreeMap};
use serde::{Serialize, Deserialize};

#[derive(Serialize,Deserialize,Debug)]
enum ScreenRotation {
    Landscape,
    Portrait,
    LandscapeReversed,
    PortraitReversed
}

#[derive(Serialize,Deserialize,Debug)]
enum ScreenPositionRelative {
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

#[derive(Serialize,Deserialize,Debug)]
struct ScreenConfiguration {
    identifier: String,
    scale: f32,
    rotation: ScreenRotation,
    display_output_code: Option<u32>,
    wallpaper: PathBuf,
    position: ScreenPositionRelative
}

#[derive(Serialize,Deserialize,Debug)]
struct ScreensProfile {
    screens: Vec<ScreenConfiguration>
}

#[derive(Serialize,Deserialize,Debug)]
struct AppConfiguration {
    profiles: BTreeMap<String, ScreensProfile>
}

#[cfg(test)]
mod test {
    use std::{fs::File, io::Read, collections::BTreeMap};

    use crate::configuration::ScreensProfile;

    use super::{AppConfiguration, ScreenConfiguration};

    #[test]
    fn parse_configuration() {
    }

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

use std::path::PathBuf;

use clap::Parser;
use serde::{Serialize, Deserialize};

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


fn main() {
    println!("WIP");
}

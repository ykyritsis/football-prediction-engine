//! Unattended capture mode: `SANDBOX_CAPTURE=<dir> football_sandbox` loads
//! every preset, advances it a little, saves a screenshot per preset into
//! `<dir>` and exits. Useful for docs, bug reports and CI artefacts.

use crate::Sandbox;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use football_ai::scenario::Preset;
use std::path::PathBuf;

#[derive(Resource)]
pub struct Capture {
    dir: PathBuf,
    frame: u32,
}

/// Frames spent on each shot: the scene is set up at 0, captured at `SHOOT`.
const PERIOD: u32 = 30;
const SHOOT: u32 = 15;

pub fn from_env() -> Option<Capture> {
    let dir = std::env::var_os("SANDBOX_CAPTURE")?;
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).ok()?;
    Some(Capture { dir, frame: 0 })
}

pub fn run(
    mut commands: Commands,
    mut cap: ResMut<Capture>,
    mut sb: ResMut<Sandbox>,
    mut exit: MessageWriter<AppExit>,
) {
    let shot = (cap.frame / PERIOD) as usize;
    let phase = cap.frame % PERIOD;
    // Shots 0..6 are the presets; shot 6 is a live match two minutes in.
    if shot > Preset::ALL.len() {
        exit.write(AppExit::Success);
        return;
    }
    if phase == 0 {
        if shot < Preset::ALL.len() {
            sb.preset = Preset::ALL[shot];
            sb.rebuild();
            sb.sim.run(20);
        } else {
            sb.preset = Preset::KickOff;
            sb.rebuild();
            sb.sim.run(30 * 120);
        }
        sb.paused = true;
        sb.selected = sb.sim.state.ball.owner;
    }
    if phase == SHOOT {
        let name = if shot < Preset::ALL.len() {
            format!("preset_{}.png", shot + 1)
        } else {
            "match_2min.png".into()
        };
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(cap.dir.join(name)));
    }
    cap.frame += 1;
}

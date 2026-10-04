//! Visual sandbox for inspecting `football_ai` behaviours.
//!
//! The deterministic simulation runs headless inside a Bevy resource; this
//! app only *reads* it to draw players, the ball and tactical overlays
//! (shape targets, marking links, pressing, cover shadows, support runs,
//! pass candidates, offside lines) and a HUD with phases, pressing triggers,
//! the carrier's scored options and the selected player's identity.
//!
//! Controls are listed in the on-screen help (and in the README).

mod capture;
mod draw;
mod hud;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use football_ai::identity::presets::{self, example_messi, example_ronaldo, Squad};
use football_ai::role::Role;
use football_ai::scenario::Preset;
use football_ai::{PlayerId, Simulation, TeamStyle};

/// Pixels per metre.
pub const SCALE: f32 = 11.0;
/// Pitch centre in screen space (leaves room for the HUD on the right).
pub const PITCH_ORIGIN: Vec2 = Vec2::new(-200.0, 30.0);

/// Convert a simulation position (metres) to screen space.
pub fn to_screen(p: football_ai::math::Vec2) -> Vec2 {
    PITCH_ORIGIN + Vec2::new(p.x, p.y) * SCALE
}

/// Which overlays are drawn.
#[derive(Debug, Clone, Copy)]
pub struct Overlays {
    pub targets: bool,
    pub shape: bool,
    pub marking: bool,
    pub pressing: bool,
    pub support: bool,
    pub passes: bool,
    pub offside: bool,
}

impl Default for Overlays {
    fn default() -> Self {
        Self {
            targets: false,
            shape: false,
            marking: true,
            pressing: true,
            support: true,
            passes: true,
            offside: true,
        }
    }
}

/// Sandbox state.
#[derive(Resource)]
pub struct Sandbox {
    pub sim: Simulation,
    pub paused: bool,
    /// Simulation speed multiplier.
    pub speed: f32,
    pub accumulator: f32,
    pub selected: Option<PlayerId>,
    pub overlays: Overlays,
    pub preset: Preset,
    pub home_style: usize,
    pub away_style: usize,
    pub stars: bool,
    pub seed: u64,
}

impl Sandbox {
    fn styles() -> Vec<TeamStyle> {
        presets::team_archetypes()
    }

    fn squad(&self, style: usize, stars: bool) -> Squad {
        let styles = Self::styles();
        let mut squad = Squad::generated(styles[style % styles.len()].clone(), 80);
        if stars {
            // Put the example stars into the first winger and striker slots.
            let f = football_ai::formation::Formation::new(squad.style.formation);
            if let Some(w) =
                (1..11).find(|&i| matches!(f.slots[i].role, Role::Winger | Role::AttackingMid))
            {
                squad = squad.with_player(w, example_messi());
            }
            if let Some(s) = (1..11).find(|&i| f.slots[i].role == Role::Striker) {
                squad = squad.with_player(s, example_ronaldo());
            }
        }
        squad
    }

    /// Rebuild the simulation from the current preset and team choices.
    pub fn rebuild(&mut self) {
        let home = self.squad(self.home_style, self.stars);
        let away = self.squad(self.away_style, false);
        self.sim = self.preset.build(self.seed, Some(home), Some(away));
        self.accumulator = 0.0;
    }

    fn new() -> Self {
        let mut s = Sandbox {
            sim: Preset::KickOff.build(1, None, None),
            paused: false,
            speed: 1.0,
            accumulator: 0.0,
            selected: None,
            overlays: Overlays::default(),
            preset: Preset::KickOff,
            home_style: 1, // gegenpress
            away_style: 2, // route one
            stars: true,
            seed: 1,
        };
        s.rebuild();
        s
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "football_ai sandbox".into(),
            resolution: WindowResolution::from((1700u32, 960u32)),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::srgb(0.10, 0.11, 0.13)))
    .insert_resource(Sandbox::new())
    .add_systems(Startup, (draw::setup, hud::setup))
    .add_systems(
        Update,
        (
            input,
            step_simulation,
            draw::sync_entities,
            draw::overlays,
            hud::update,
        )
            .chain(),
    );
    if let Some(capture) = capture::from_env() {
        app.insert_resource(capture)
            .add_systems(Update, capture::run.before(input));
    }
    app.run();
}

/// Advance the simulation by real time × speed, in fixed steps.
fn step_simulation(time: Res<Time>, mut sb: ResMut<Sandbox>) {
    if sb.paused {
        return;
    }
    let dt = sb.sim.state.dt;
    sb.accumulator += time.delta_secs() * sb.speed;
    let mut steps = 0;
    while sb.accumulator >= dt && steps < 40 {
        sb.sim.step();
        sb.accumulator -= dt;
        steps += 1;
    }
    if steps == 40 {
        sb.accumulator = 0.0;
    }
}

fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut sb: ResMut<Sandbox>,
) {
    if keys.just_pressed(KeyCode::Space) {
        sb.paused = !sb.paused;
    }
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::Period) {
        sb.paused = true;
        sb.sim.step();
    }
    if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
        sb.speed = (sb.speed * 2.0).min(16.0);
    }
    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        sb.speed = (sb.speed / 2.0).max(0.125);
    }
    let digits = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ];
    for (i, k) in digits.iter().enumerate() {
        if keys.just_pressed(*k) {
            sb.preset = Preset::ALL[i];
            sb.rebuild();
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        sb.rebuild();
    }
    if keys.just_pressed(KeyCode::KeyN) {
        sb.seed = sb.seed.wrapping_add(1);
        sb.rebuild();
    }
    if keys.just_pressed(KeyCode::KeyH) {
        sb.home_style += 1;
        sb.rebuild();
    }
    if keys.just_pressed(KeyCode::KeyA) {
        sb.away_style += 1;
        sb.rebuild();
    }
    if keys.just_pressed(KeyCode::KeyF) {
        // Whistle a foul on the carrier by the nearest opponent (to inspect set pieces).
        if let Some(c) = sb.sim.state.carrier().cloned() {
            if let Some(f) = sb
                .sim
                .state
                .nearest_player(c.team.opponent(), c.pos, false)
                .map(|p| p.id)
            {
                sb.sim.whistle_foul(f, c.id, false);
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyX) {
        sb.stars = !sb.stars;
        sb.rebuild();
    }
    if keys.just_pressed(KeyCode::Tab) {
        let next = sb.selected.map_or(0, |p| (p.0 + 1) % 22);
        sb.selected = Some(PlayerId(next));
    }
    if keys.just_pressed(KeyCode::Escape) {
        sb.selected = None;
    }
    let o = &mut sb.overlays;
    if keys.just_pressed(KeyCode::KeyT) {
        o.targets = !o.targets;
    }
    if keys.just_pressed(KeyCode::KeyS) {
        o.shape = !o.shape;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        o.marking = !o.marking;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        o.pressing = !o.pressing;
    }
    if keys.just_pressed(KeyCode::KeyU) {
        o.support = !o.support;
    }
    if keys.just_pressed(KeyCode::KeyC) {
        o.passes = !o.passes;
    }
    if keys.just_pressed(KeyCode::KeyO) {
        o.offside = !o.offside;
    }

    // Click to select the nearest player.
    if mouse.just_pressed(MouseButton::Left) {
        let (Ok(window), Ok((camera, cam_tf))) = (windows.single(), cameras.single()) else {
            return;
        };
        let Some(cursor) = window.cursor_position() else {
            return;
        };
        let Ok(world) = camera.viewport_to_world_2d(cam_tf, cursor) else {
            return;
        };
        let best = sb
            .sim
            .state
            .players
            .iter()
            .map(|p| (to_screen(p.pos).distance(world), p.id))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, id)) = best {
            sb.selected = (d < 2.5 * SCALE).then_some(id);
        }
    }
}

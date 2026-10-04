//! Run a headless match and print a summary.
//!
//! ```text
//! cargo run --release -p football_ai --example headless_match -- [seed] [minutes] [home] [away]
//! ```
//! `home`/`away` are team archetype names: tiki-taka, gegenpress, long-ball,
//! counter, wing-play, bus.

use football_ai::identity::presets::{self, Squad};
use football_ai::passing::PassKind;
use football_ai::pitch::TeamSide;
use football_ai::{MatchEvent, SimConfig, Simulation};

fn style(name: &str) -> football_ai::TeamStyle {
    match name {
        "tiki-taka" => presets::tiki_taka(),
        "gegenpress" => presets::gegenpress(),
        "long-ball" => presets::long_ball(),
        "counter" => presets::counter_attack(),
        "wing-play" => presets::wing_play(),
        "bus" => presets::park_the_bus(),
        other => panic!("unknown style {other}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(7);
    let minutes: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(90.0);
    let home = style(args.get(3).map(String::as_str).unwrap_or("tiki-taka"));
    let away = style(args.get(4).map(String::as_str).unwrap_or("long-ball"));

    let config = SimConfig { seed, dt: 1.0 / 30.0, home: Squad::generated(home, 80), away: Squad::generated(away, 80) };
    let mut sim = Simulation::new(config);
    let ticks = (minutes * 60.0 / sim.state.dt) as u32;
    let mut possession_ticks = [0u32; 2];
    let start = std::time::Instant::now();
    for _ in 0..ticks {
        sim.step();
        possession_ticks[sim.state.possession.index()] += 1;
    }
    let elapsed = start.elapsed();

    let mut stats = [[0u32; 9]; 2];
    // passes, completed, long, through, crosses, shots, tackles won, interceptions, xg*100
    let mut xg = [0f32; 2];
    for (_, e) in &sim.events {
        match *e {
            MatchEvent::Pass { from, kind, .. } => {
                let t = from.team().index();
                stats[t][0] += 1;
                match kind {
                    PassKind::Long => stats[t][2] += 1,
                    PassKind::Through => stats[t][3] += 1,
                    PassKind::Cross => stats[t][4] += 1,
                    PassKind::Short => {}
                }
            }
            MatchEvent::PassCompleted { from, .. } => stats[from.team().index()][1] += 1,
            MatchEvent::Shot { by, xg: x } => {
                stats[by.team().index()][5] += 1;
                xg[by.team().index()] += x;
            }
            MatchEvent::Tackle { by, won: true, .. } => stats[by.team().index()][6] += 1,
            MatchEvent::Interception { by, .. } => stats[by.team().index()][7] += 1,
            _ => {}
        }
    }
    println!("{}", sim.squads[0].style.describe());
    println!();
    println!("{}", sim.squads[1].style.describe());
    println!();
    println!(
        "{} {} - {} {}   ({minutes} min, seed {seed}, simulated in {:.2?})",
        sim.squads[0].style.name, sim.state.score[0], sim.state.score[1], sim.squads[1].style.name, elapsed
    );
    let total = (possession_ticks[0] + possession_ticks[1]).max(1) as f32;
    for side in TeamSide::BOTH {
        let i = side.index();
        let s = stats[i];
        println!(
            "{:>14}: poss {:>3.0}%  passes {:>4} ({:>3.0}% compl)  long {:>3}  through {:>3}  crosses {:>3}  shots {:>3} (xG {:.2})  tackles won {:>3}  interceptions {:>3}",
            sim.squads[i].style.name,
            100.0 * possession_ticks[i] as f32 / total,
            s[0],
            100.0 * s[1] as f32 / s[0].max(1) as f32,
            s[2],
            s[3],
            s[4],
            s[5],
            xg[i],
            s[6],
            s[7],
        );
    }
}

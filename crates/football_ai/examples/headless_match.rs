//! Run a headless match and print a summary.
//!
//! ```text
//! cargo run --release -p football_ai --example headless_match -- [seed] [minutes] [home] [away]
//! ```
//! `home`/`away` are team archetype names: tiki-taka, gegenpress, long-ball,
//! counter, wing-play, bus.

use football_ai::identity::presets::{self, Squad};
use football_ai::{SimConfig, Simulation};

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

    let config = SimConfig::new(seed, Squad::generated(home, 80), Squad::generated(away, 80));
    let mut sim = Simulation::new(config);
    // Play the requested minutes, or a full match with stoppage time for 90.
    let ticks = if minutes >= 90.0 {
        u32::MAX
    } else {
        (minutes * 60.0 / sim.state.dt) as u32
    };
    let start = std::time::Instant::now();
    for _ in 0..ticks {
        if sim.finished() {
            break;
        }
        sim.step();
    }
    let elapsed = start.elapsed();

    println!("{}", sim.squads[0].style.describe());
    println!();
    println!("{}", sim.squads[1].style.describe());
    println!();
    println!(
        "{:.1} min played, seed {seed}, simulated in {:.2?}\n",
        sim.match_seconds() / 60.0,
        elapsed
    );
    println!(
        "{}",
        sim.stats
            .report([&sim.squads[0].style.name, &sim.squads[1].style.name])
    );
    if let Some(m) = sim.stats.man_of_the_match() {
        println!("Man of the match: {} ({:.1})", m.name, m.rating());
    }
}

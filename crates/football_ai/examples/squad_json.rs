//! Export a squad to JSON, or load one and play a match with it.
//!
//! ```text
//! cargo run -p football_ai --features serde --example squad_json -- export > squad.json
//! cargo run -p football_ai --features serde --example squad_json -- play squad.json
//! ```
//!
//! The JSON schema mirrors the PES 2018 database fields (ratings, playing
//! style, COM styles, skills, game plan), so an extractor can write squads
//! straight into it.

use football_ai::identity::presets::{self, example_messi, example_ronaldo, Squad};
use football_ai::{MatchEvent, SimConfig, Simulation};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("play") => {
            let path = args.get(2).expect("path to a squad JSON file");
            let text = std::fs::read_to_string(path).expect("readable file");
            let home: Squad = serde_json::from_str(&text).expect("valid squad JSON");
            assert_eq!(home.players.len(), 11, "a squad needs eleven starters");
            println!("{}\n", home.describe());
            let away = Squad::generated(presets::counter_attack(), 80);
            let mut sim = Simulation::new(SimConfig::new(1, home, away));
            sim.run_to_full_time(u32::MAX);
            let goals = sim
                .events
                .iter()
                .filter(|(_, e)| matches!(e, MatchEvent::Goal { .. }))
                .count();
            println!(
                "Full time: {} {} - {} {} ({goals} goals)",
                sim.squads[0].style.name,
                sim.state.score[0],
                sim.state.score[1],
                sim.squads[1].style.name
            );
        }
        _ => {
            // Default: export an example squad with the two star profiles.
            let mut squad = Squad::generated(presets::gegenpress(), 82);
            squad.style.name = "Example XI".into();
            let squad = squad
                .with_player(7, example_messi())
                .with_player(10, example_ronaldo());
            println!("{}", serde_json::to_string_pretty(&squad).unwrap());
        }
    }
}

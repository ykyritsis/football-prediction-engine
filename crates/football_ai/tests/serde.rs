//! Identities round-trip through JSON (feature `serde`).

use football_ai::identity::presets::{self, example_messi, Squad};
use football_ai::identity::Skill;
use football_ai::{SimConfig, Simulation};

#[test]
fn squads_round_trip_through_json() {
    for style in presets::team_archetypes() {
        let squad = Squad::generated(style, 79);
        let json = serde_json::to_string(&squad).unwrap();
        let back: Squad = serde_json::from_str(&json).unwrap();
        assert_eq!(squad, back);
    }
}

#[test]
fn skills_are_written_as_readable_names() {
    let json = serde_json::to_string(&example_messi()).unwrap();
    assert!(json.contains("\"DoubleTouch\""), "{json}");
    assert!(json.contains("\"CreativePlaymaker\""));
    let back: football_ai::PlayerProfile = serde_json::from_str(&json).unwrap();
    assert!(back.skills.contains(Skill::DoubleTouch));
}

#[test]
fn the_bundled_example_file_loads_and_plays() {
    let text = include_str!("../../../data/example_squad.json");
    let squad: Squad = serde_json::from_str(text).unwrap();
    assert_eq!(squad.players.len(), 11);
    let mut sim = Simulation::new(SimConfig::new(
        2,
        squad,
        Squad::generated(presets::wing_play(), 80),
    ));
    sim.run(30 * 60);
    assert_eq!(sim.squads[0].players[7].name, "L. Messi");
}

//! Match statistics: consistent with the event log and in a football range.

use football_ai::identity::presets::{self, Squad};
use football_ai::pitch::TeamSide;
use football_ai::stats::TeamStats;
use football_ai::{MatchEvent, SimConfig, Simulation};

fn full_match(seed: u64) -> Simulation {
    let styles = presets::team_archetypes();
    let mut sim = Simulation::new(SimConfig::new(
        seed,
        Squad::generated(styles[seed as usize % 6].clone(), 80),
        Squad::generated(styles[(seed as usize + 2) % 6].clone(), 80),
    ));
    sim.run_to_full_time(u32::MAX);
    sim
}

fn count(sim: &Simulation, side: TeamSide, f: impl Fn(&MatchEvent) -> Option<TeamSide>) -> u32 {
    sim.events
        .iter()
        .filter(|(_, e)| f(e) == Some(side))
        .count() as u32
}

#[test]
fn totals_match_the_event_log() {
    let sim = full_match(3);
    for side in TeamSide::BOTH {
        let t = sim.stats.team(side);
        assert_eq!(t.goals, sim.state.score[side.index()] as u32);
        let passes = count(&sim, side, |e| match e {
            MatchEvent::Pass { from, .. } => Some(from.team()),
            _ => None,
        });
        assert_eq!(t.passes, passes);
        let shots = count(&sim, side, |e| match e {
            MatchEvent::Shot { by, .. } => Some(by.team()),
            _ => None,
        });
        assert_eq!(t.shots, shots);
        assert!(t.shots_on_target <= t.shots);
        assert!(t.shots_on_target >= t.goals);
        assert!(t.passes_completed <= t.passes);
        let fouls = count(&sim, side, |e| match e {
            MatchEvent::Foul { by, .. } => Some(by.team()),
            _ => None,
        });
        assert_eq!(t.fouls, fouls);
        // Player lines add up to the team.
        let lines = sim.stats.players.iter().filter(|p| p.team == side);
        assert_eq!(lines.clone().map(|p| p.passes).sum::<u32>(), t.passes);
        assert_eq!(lines.map(|p| p.shots).sum::<u32>(), t.shots);
    }
    let p = sim.stats.possession(TeamSide::Home) + sim.stats.possession(TeamSide::Away);
    assert!((p - 1.0).abs() < 1e-4);
}

#[test]
fn scorers_assists_and_substitutes_get_credit() {
    let sims: Vec<Simulation> = (0..4).map(full_match).collect();
    let mut scorers = 0;
    for sim in &sims {
        let goals: u32 = sim
            .stats
            .players
            .iter()
            .map(|p| p.goals + p.own_goals)
            .sum();
        let total = sim.state.score[0] as u32 + sim.state.score[1] as u32;
        assert!(goals <= total);
        for p in sim.stats.players.iter().filter(|p| p.goals > 0) {
            scorers += 1;
            assert!(
                p.rating() > 6.5,
                "{} scored but rated {}",
                p.name,
                p.rating()
            );
        }
        let conceded: u32 = sim.stats.players.iter().map(|p| p.goals_conceded).sum();
        assert_eq!(conceded, total);
        // One line per appearance: starters plus substitutes.
        let subs = (sim.state.subs_used[0] + sim.state.subs_used[1]) as usize;
        assert_eq!(sim.stats.players.len(), 22 + subs);
        assert!(sim.stats.players.iter().filter(|p| p.on > 0.0).count() == subs);
        assert!(sim.stats.man_of_the_match().is_some());
    }
    assert!(scorers > 0);
    let assists: u32 = sims
        .iter()
        .flat_map(|s| s.stats.players.iter())
        .map(|p| p.assists)
        .sum();
    assert!(assists > 0, "no assists in four matches");
}

#[test]
fn match_statistics_are_in_a_football_range() {
    // Twelve matches on four threads: goals are too rare to judge on fewer.
    let handles: Vec<_> = (0..4u64)
        .map(|t| std::thread::spawn(move || (t..12).step_by(4).map(full_match).collect::<Vec<_>>()))
        .collect();
    let sims: Vec<Simulation> = handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();
    let teams: Vec<&TeamStats> = sims.iter().flat_map(|s| s.stats.teams.iter()).collect();
    let avg = |f: &dyn Fn(&TeamStats) -> f32| {
        teams.iter().map(|t| f(t)).sum::<f32>() / teams.len() as f32
    };
    let shots = avg(&|t| t.shots as f32);
    let on_target = avg(&|t| t.shots_on_target as f32) / shots;
    let accuracy = avg(&|t| t.pass_accuracy());
    let fouls = avg(&|t| t.fouls as f32);
    let corners = avg(&|t| t.corners as f32);
    let goals = avg(&|t| t.goals as f32);
    let xg = avg(&|t| t.xg);
    assert!((6.0..20.0).contains(&shots), "{shots} shots");
    assert!((0.25..0.7).contains(&on_target), "{on_target} on target");
    assert!((0.65..0.9).contains(&accuracy), "{accuracy} pass accuracy");
    assert!((5.0..20.0).contains(&fouls), "{fouls} fouls");
    assert!(corners >= 1.0, "{corners} corners");
    assert!((0.6..3.0).contains(&goals), "{goals} goals");
    // The decision layer's xG predicts what goes in.
    assert!((goals / xg - 1.0).abs() < 0.5, "{goals} goals from {xg} xG");
}

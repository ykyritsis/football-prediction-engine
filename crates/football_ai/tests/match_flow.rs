//! Whole-match flow: the game keeps moving and produces football-like events.

use football_ai::identity::presets::{self, Squad};
use football_ai::pitch::TeamSide;
use football_ai::state::RestartKind;
use football_ai::{MatchEvent, SimConfig, Simulation};

fn full_match(seed: u64, home: football_ai::TeamStyle, away: football_ai::TeamStyle) -> Simulation {
    let cfg = SimConfig::new(seed, Squad::generated(home, 80), Squad::generated(away, 80));
    let mut sim = Simulation::new(cfg);
    let mut idle = 0u32;
    let mut restart_ticks = 0u32;
    for t in 0..(30 * 60 * 90) {
        sim.step();
        let s = &sim.state;
        // A loose, stationary ball must be picked up quickly.
        if s.ball.owner.is_none() && s.restart.is_none() && s.ball.vel.length() < 0.5 {
            idle += 1;
        } else {
            idle = 0;
        }
        assert!(idle < 30 * 8, "ball abandoned at tick {t}");
        // Restarts must be taken.
        if s.restart.is_some() {
            restart_ticks += 1;
        } else {
            restart_ticks = 0;
        }
        assert!(
            restart_ticks < 30 * 20,
            "restart never taken at tick {t}: {:?}",
            s.restart
        );
        // Nobody keeps the ball for ever.
        if s.ball.owner.is_some() {
            assert!(
                s.seconds_since(s.ball.owner_since) < 25.0,
                "carrier stalled at tick {t}"
            );
        }
    }
    sim
}

fn count(sim: &Simulation, f: impl Fn(&MatchEvent) -> bool) -> usize {
    sim.events.iter().filter(|(_, e)| f(e)).count()
}

#[test]
fn a_full_match_flows() {
    let sim = full_match(77, presets::gegenpress(), presets::counter_attack());
    for side in TeamSide::BOTH {
        let passes = count(
            &sim,
            |e| matches!(e, MatchEvent::Pass { from, .. } if from.team() == side),
        );
        let completed = count(
            &sim,
            |e| matches!(e, MatchEvent::PassCompleted { from, .. } if from.team() == side),
        );
        let shots = count(
            &sim,
            |e| matches!(e, MatchEvent::Shot { by, .. } if by.team() == side),
        );
        assert!(passes > 250, "{side:?} only {passes} passes");
        let rate = completed as f32 / passes as f32;
        assert!((0.5..0.95).contains(&rate), "{side:?} completion {rate}");
        assert!(shots >= 1, "{side:?} never shot");
    }
    let changes = count(&sim, |e| matches!(e, MatchEvent::PossessionChange { .. }));
    assert!(changes > 50, "only {changes} possession changes");
    let goals = sim.state.score[0] + sim.state.score[1];
    assert!(goals <= 20, "{goals} goals is not football");
    assert_eq!(
        goals as usize,
        count(&sim, |e| matches!(e, MatchEvent::Goal { .. }))
    );
}

#[test]
fn all_restart_types_occur_and_are_taken() {
    // Over a few matches (a single match can go without a corner).
    let sims: Vec<Simulation> = (5..8)
        .map(|seed| full_match(seed, presets::wing_play(), presets::long_ball()))
        .collect();
    for kind in [
        RestartKind::ThrowIn,
        RestartKind::GoalKick,
        RestartKind::Corner,
    ] {
        let total: usize = sims
            .iter()
            .map(|sim| {
                count(
                    sim,
                    |e| matches!(e, MatchEvent::OutOfPlay { restart, .. } if *restart == kind),
                )
            })
            .sum();
        assert!(total > 0, "no {kind:?} in three matches");
    }
}

#[test]
fn goal_resets_to_kick_off_for_the_conceding_team() {
    let cfg = SimConfig {
        seed: 3,
        ..SimConfig::default()
    };
    let mut sim = Simulation::new(cfg);
    for _ in 0..(30 * 60 * 90) {
        let before = sim.state.score;
        sim.step();
        if sim.state.score != before {
            let scorer = if sim.state.score[0] != before[0] {
                TeamSide::Home
            } else {
                TeamSide::Away
            };
            let r = sim.state.restart.expect("kick-off pending");
            assert_eq!(r.kind, RestartKind::KickOff);
            assert_eq!(r.team, scorer.opponent());
            assert!(sim.state.ball.pos.length() < 1.0);
            // Everyone is in his own half.
            for p in &sim.state.players {
                assert!(
                    p.team.to_local(p.pos).x <= 0.0,
                    "{:?} not in own half",
                    p.id
                );
            }
            return;
        }
    }
    panic!("no goal in 90 minutes with seed 3");
}

#[test]
fn keepers_make_saves_and_offsides_are_called() {
    let sim = full_match(12, presets::tiki_taka(), presets::gegenpress());
    assert!(count(&sim, |e| matches!(e, MatchEvent::Save { .. })) > 0);
    assert!(
        count(&sim, |e| matches!(e, MatchEvent::Offside { .. })) > 0
            || count(&sim, |e| matches!(e, MatchEvent::Shot { .. })) > 0
    );
}

#[test]
fn scoring_is_football_like() {
    let styles = presets::team_archetypes();
    let n = 6;
    let (mut goals, mut shots, mut on_target) = (0usize, 0usize, 0usize);
    for seed in 0..n {
        let mut sim = Simulation::new(SimConfig::new(
            seed,
            Squad::generated(styles[seed as usize % 6].clone(), 80),
            Squad::generated(styles[(seed as usize + 3) % 6].clone(), 80),
        ));
        sim.run_to_full_time(u32::MAX);
        goals += count(&sim, |e| matches!(e, MatchEvent::Goal { .. }));
        shots += count(&sim, |e| matches!(e, MatchEvent::Shot { .. }));
        on_target += count(&sim, |e| matches!(e, MatchEvent::Goal { .. } | MatchEvent::Save { .. }));
    }
    let per_match = goals as f32 / n as f32;
    assert!((1.0..7.0).contains(&per_match), "{per_match} goals per match");
    let accuracy = on_target as f32 / shots as f32;
    assert!(accuracy < 0.85, "{:.0}% of shots on target", accuracy * 100.0);
    assert!((10..60).contains(&(shots / n as usize)), "{} shots per match", shots / n as usize);
}

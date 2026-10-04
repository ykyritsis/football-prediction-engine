//! Determinism and invariants of the simulation.

use football_ai::decision::decide;
use football_ai::identity::presets::{self, Squad};
use football_ai::pitch::{TeamSide, HALF_LENGTH, HALF_WIDTH};
use football_ai::plan::plan_team;
use football_ai::{SimConfig, Simulation};

fn config(seed: u64) -> SimConfig {
    SimConfig {
        seed,
        dt: 1.0 / 30.0,
        home: Squad::generated(presets::tiki_taka(), 80),
        away: Squad::generated(presets::counter_attack(), 78),
    }
}

#[test]
fn same_seed_gives_identical_state_every_tick() {
    let mut a = Simulation::new(config(42));
    let mut b = Simulation::new(config(42));
    for t in 0..6000 {
        a.step();
        b.step();
        assert_eq!(
            a.state.state_hash(),
            b.state.state_hash(),
            "diverged at tick {t}"
        );
    }
    assert_eq!(a.state, b.state);
    assert_eq!(a.events, b.events);
}

#[test]
fn different_seeds_diverge() {
    let mut a = Simulation::new(config(1));
    let mut b = Simulation::new(config(2));
    a.run(3000);
    b.run(3000);
    assert_ne!(a.state.state_hash(), b.state.state_hash());
}

#[test]
fn cloned_simulation_continues_identically() {
    let mut a = Simulation::new(config(9));
    a.run(1500);
    let mut b = a.clone();
    a.run(1500);
    b.run(1500);
    assert_eq!(a.state.state_hash(), b.state.state_hash());
}

#[test]
fn planning_and_decisions_are_pure_and_order_independent() {
    let mut sim = Simulation::new(config(5));
    for _ in 0..20 {
        sim.run(97);
        let state = sim.state.clone();
        let plans = [
            plan_team(&state, TeamSide::Home),
            plan_team(&state, TeamSide::Away),
        ];
        // Planning twice gives the same plan.
        assert_eq!(plans[0], plan_team(&state, TeamSide::Home));
        // Decisions in reverse order equal decisions in forward order.
        let forward: Vec<_> = state
            .players
            .iter()
            .map(|p| decide(&state, &plans[p.team.index()], p.id))
            .collect();
        let mut reverse: Vec<_> = state
            .players
            .iter()
            .rev()
            .map(|p| decide(&state, &plans[p.team.index()], p.id))
            .collect();
        reverse.reverse();
        assert_eq!(forward, reverse);
        // Nothing was mutated.
        assert_eq!(state, sim.state);
    }
}

#[test]
fn state_stays_finite_and_on_the_pitch() {
    let mut sim = Simulation::new(config(11));
    for t in 0..(30 * 60 * 15) {
        sim.step();
        for p in &sim.state.players {
            assert!(
                p.pos.is_finite() && p.vel.is_finite(),
                "non-finite player {} at tick {t}",
                p.id.0
            );
            assert!(
                p.pos.x.abs() <= HALF_LENGTH + 2.0 && p.pos.y.abs() <= HALF_WIDTH + 2.0,
                "player off pitch at tick {t}"
            );
            assert!(
                p.vel.length() <= p.attrs.top_speed + 1e-3,
                "player over top speed"
            );
        }
        let b = &sim.state.ball;
        assert!(b.pos.is_finite() && b.vel.is_finite());
        // The ball is either in play or has just been placed for a restart.
        assert!(
            b.pos.x.abs() <= HALF_LENGTH + 1.0 && b.pos.y.abs() <= HALF_WIDTH + 1.0,
            "ball escaped at tick {t}"
        );
        if let Some(o) = b.owner {
            assert_eq!(
                o.team(),
                sim.state.possession,
                "owner/possession mismatch at tick {t}"
            );
        }
    }
}

#[test]
fn every_player_gets_a_decision_each_tick() {
    let mut sim = Simulation::new(config(3));
    sim.run(10);
    assert_eq!(sim.decisions.len(), 22);
    for (i, d) in sim.decisions.iter().enumerate() {
        assert_eq!(d.player.index(), i);
        assert!(d.target.is_finite());
    }
}

#[test]
fn hash_detects_any_change() {
    let mut sim = Simulation::new(config(3));
    sim.run(100);
    let h = sim.state.state_hash();
    let mut s2 = sim.state.clone();
    s2.players[7].pos.x += 1e-4;
    assert_ne!(h, s2.state_hash());
}

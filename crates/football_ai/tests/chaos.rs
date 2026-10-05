//! Football happens: lucky goals, howlers, set-piece goals, confidence
//! swings, and stars who change matches.

use football_ai::identity::presets::{self, Squad};
use football_ai::pitch::TeamSide;
use football_ai::role::Role;
use football_ai::state::RestartKind;
use football_ai::{MatchEvent, SimConfig, Simulation};

fn mixed(seed: u64) -> (Squad, Squad) {
    let styles = presets::team_archetypes();
    (
        Squad::generated(styles[seed as usize % 6].clone(), 80),
        Squad::generated(styles[(seed as usize + 2) % 6].clone(), 80),
    )
}

/// Run matches on four threads, in seed order.
fn matches(seeds: std::ops::Range<u64>, make: fn(u64) -> (Squad, Squad)) -> Vec<Simulation> {
    let seeds: Vec<u64> = seeds.collect();
    let chunks: Vec<Vec<u64>> = seeds
        .chunks(seeds.len().div_ceil(4))
        .map(|c| c.to_vec())
        .collect();
    let handles: Vec<_> = chunks
        .into_iter()
        .map(|chunk| {
            std::thread::spawn(move || {
                chunk
                    .into_iter()
                    .map(|s| {
                        let (h, a) = make(s);
                        let mut sim = Simulation::new(SimConfig::new(s, h, a));
                        sim.run_to_full_time(u32::MAX);
                        sim
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect()
}

#[test]
fn scoring_lifts_a_players_confidence() {
    for seed in 0..6 {
        let (h, a) = mixed(seed);
        let mut sim = Simulation::new(SimConfig::new(seed, h, a));
        while !sim.finished() {
            let n = sim.events.len();
            sim.step();
            for (_, e) in &sim.events[n..] {
                if let MatchEvent::Goal {
                    team,
                    scorer: Some(s),
                } = e
                {
                    if s.team() == *team {
                        let c = sim.state.player(*s).confidence;
                        assert!(c > 0.15, "scorer confidence {c}");
                        return;
                    }
                }
            }
        }
    }
    panic!("no goal scored in six matches");
}

#[test]
fn luck_errors_and_set_pieces_decide_some_goals() {
    let sims = matches(0..24, mixed);
    let (mut lucky, mut set_piece, mut centre_back, mut mishaps) = (0, 0, 0, 0);
    for sim in &sims {
        let mut last_xg = [1.0f32; 2];
        let mut last_restart: Option<(u32, RestartKind)> = None;
        let mut pending: Option<RestartKind> = None;
        for (tick, e) in &sim.events {
            match e {
                MatchEvent::OutOfPlay { restart, .. } => pending = Some(*restart),
                MatchEvent::Pass { .. } | MatchEvent::Shot { .. } if pending.is_some() => {
                    last_restart = pending.take().map(|k| (*tick, k));
                }
                _ => {}
            }
            match e {
                MatchEvent::Shot { by, xg, .. } => last_xg[by.team().index()] = *xg,
                MatchEvent::KeeperError { .. } => mishaps += 1,
                MatchEvent::Goal { team, scorer } => {
                    if scorer.is_some_and(|s| s.team() != *team) {
                        mishaps += 1;
                    } else if last_xg[team.index()] < 0.05 {
                        lucky += 1;
                    }
                    if let Some(s) = scorer.filter(|s| s.team() == *team) {
                        if sim.state.player(s).role == Role::CentreBack {
                            centre_back += 1;
                        }
                    }
                    if matches!(last_restart, Some((t, RestartKind::Corner | RestartKind::FreeKick)) if tick - t < 300)
                    {
                        set_piece += 1;
                    }
                }
                _ => {}
            }
        }
    }
    assert!(lucky > 0, "no goal from a long-odds shot in 24 matches");
    assert!(set_piece > 0, "no set-piece goal in 24 matches");
    assert!(centre_back > 0, "no centre-back goal in 24 matches");
    assert!(mishaps > 0, "no howler or own goal in 24 matches");
}

#[test]
fn a_star_changes_matches() {
    fn with_mbappe(seed: u64) -> (Squad, Squad) {
        let mut home = Squad::generated(presets::wing_play(), 79);
        let slot = home
            .players
            .iter()
            .position(|p| p.position == Role::Striker)
            .unwrap();
        home.players[slot] = presets::example_mbappe();
        (
            home,
            Squad::generated(presets::team_archetypes()[seed as usize % 6].clone(), 79),
        )
    }
    fn without(seed: u64) -> (Squad, Squad) {
        (
            Squad::generated(presets::wing_play(), 79),
            Squad::generated(presets::team_archetypes()[seed as usize % 6].clone(), 79),
        )
    }
    let points = |sims: &[Simulation]| {
        sims.iter()
            .map(|s| match s.state.score[0].cmp(&s.state.score[1]) {
                std::cmp::Ordering::Greater => 3,
                std::cmp::Ordering::Equal => 1,
                std::cmp::Ordering::Less => 0,
            })
            .sum::<u32>()
    };
    let (star, plain) = (matches(0..24, with_mbappe), matches(0..24, without));
    let (ps, pp) = (points(&star), points(&plain));
    assert!(ps > pp + 6, "with the star {ps} points, without {pp}");
    // He is decisive but not a cheat code: he does not win every match.
    let lost_or_drawn = star
        .iter()
        .filter(|s| s.state.score[0] <= s.state.score[1])
        .count();
    assert!(
        lost_or_drawn >= 4,
        "the star's team dropped points only {lost_or_drawn} times"
    );
    let _ = TeamSide::Home;
}

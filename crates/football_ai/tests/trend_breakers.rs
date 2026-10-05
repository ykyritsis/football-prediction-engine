//! Trend breakers: ratings set expectations, they do not decide matches.
//! Stars beat their numbers, teams have good and bad days, a low block
//! protects an underdog and the weaker side sometimes wins.

use football_ai::identity::presets::{self, Squad};
use football_ai::identity::TeamStyle;
use football_ai::pitch::TeamSide;
use football_ai::role::Role;
use football_ai::{MatchEvent, PlayerId, SimConfig, Simulation};

fn full_match(seed: u64, home: Squad, away: Squad) -> Simulation {
    let mut sim = Simulation::new(SimConfig::new(seed, home, away));
    sim.run_to_full_time(u32::MAX);
    sim
}

/// Run matches on all cores; results in seed order.
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
                        full_match(s, h, a)
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

fn goals(sim: &Simulation, side: TeamSide) -> u32 {
    sim.state.score[side.index()] as u32
}

#[test]
fn match_day_form_is_seeded_and_varies() {
    let cfg = |seed| {
        SimConfig::new(
            seed,
            Squad::generated(presets::wing_play(), 80),
            Squad::generated(presets::counter_attack(), 80),
        )
    };
    let form = |sim: &Simulation| sim.state.players.iter().map(|p| p.form).collect::<Vec<_>>();
    let (a, b, c) = (
        Simulation::new(cfg(1)),
        Simulation::new(cfg(1)),
        Simulation::new(cfg(2)),
    );
    assert_eq!(form(&a), form(&b), "form must come from the seed");
    assert_ne!(form(&a), form(&c));
    // Somebody is always noticeably on or off his game.
    assert!(form(&a).iter().any(|f| f.abs() > 0.2));
    // Team days differ: average form is not the same for both sides.
    let avg = |sim: &Simulation, side: TeamSide| {
        let v: Vec<f32> = sim
            .state
            .team_players(side)
            .iter()
            .map(|p| p.form)
            .collect();
        v.iter().sum::<f32>() / v.len() as f32
    };
    let spread: f32 = (0..8)
        .map(|s| {
            let sim = Simulation::new(cfg(s));
            (avg(&sim, TeamSide::Home) - avg(&sim, TeamSide::Away)).abs()
        })
        .fold(0.0, f32::max);
    assert!(spread > 0.2, "team days never differ: {spread}");
}

fn with_star(style: TeamStyle, star: bool) -> Squad {
    let mut squad = Squad::generated(style, 79);
    if star {
        let slot = squad
            .players
            .iter()
            .position(|p| p.position == Role::Striker)
            .expect("striker");
        let mut cr = presets::example_ronaldo();
        cr.position = Role::Striker;
        squad.players[slot] = cr;
    }
    squad
}

fn striker_goals(sim: &Simulation) -> u32 {
    let striker: Vec<PlayerId> = sim
        .state
        .team_players(TeamSide::Home)
        .iter()
        .filter(|p| p.role == Role::Striker)
        .map(|p| p.id)
        .collect();
    sim.events
        .iter()
        .filter(
            |(_, e)| matches!(e, MatchEvent::Goal { scorer: Some(s), .. } if striker.contains(s)),
        )
        .count() as u32
}

#[test]
fn a_star_finisher_outperforms_an_ordinary_striker() {
    let star = matches(0..8, |s| {
        (
            with_star(presets::wing_play(), true),
            Squad::generated(presets::team_archetypes()[s as usize % 6].clone(), 79),
        )
    });
    let plain = matches(0..8, |s| {
        (
            with_star(presets::wing_play(), false),
            Squad::generated(presets::team_archetypes()[s as usize % 6].clone(), 79),
        )
    });
    let (a, b): (u32, u32) = (
        star.iter().map(striker_goals).sum(),
        plain.iter().map(striker_goals).sum(),
    );
    assert!(a > b, "star striker {a} goals, ordinary striker {b}");
}

#[test]
fn the_weaker_team_sometimes_wins() {
    // Three overall points apart: a clear favourite, not a foregone conclusion.
    let sims = matches(0..36, |s| {
        let styles = presets::team_archetypes();
        (
            Squad::generated(styles[s as usize % 6].clone(), 80),
            Squad::generated(styles[(s as usize + 1) % 6].clone(), 77),
        )
    });
    let (mut fav, mut dog, mut dog_wins) = (0, 0, 0);
    for sim in &sims {
        let (h, a) = (goals(sim, TeamSide::Home), goals(sim, TeamSide::Away));
        fav += if h > a {
            3
        } else if h == a {
            1
        } else {
            0
        };
        dog += if a > h {
            3
        } else if h == a {
            1
        } else {
            0
        };
        dog_wins += (a > h) as u32;
    }
    assert!(fav > dog, "favourite {fav} points, underdog {dog}");
    // Real football: across a gap like this (a top-four side against a
    // mid-table one) the underdog takes about a fifth to a quarter of the
    // points and wins one match in six or seven.
    assert!(dog >= 18, "underdog took only {dog} points from 36 matches");
    assert!(dog_wins >= 4, "the underdog won only {dog_wins} of 36");
}

#[test]
fn a_low_block_protects_an_underdog() {
    // Goals the underdog concedes while still playing its own system: once
    // it falls behind and chases the game, it gives up the block (and gets
    // caught on the break, as real teams do).
    let in_system = |sims: &[Simulation]| {
        sims.iter()
            .map(|s| {
                let mut chasing = false;
                let mut conceded = 0;
                for (_, e) in &s.events {
                    match e {
                        MatchEvent::MentalityChange { team, mentality }
                            if *team == TeamSide::Away =>
                        {
                            chasing = *mentality != football_ai::tactics::Mentality::Normal;
                        }
                        MatchEvent::Goal { team, .. } if *team == TeamSide::Home && !chasing => {
                            conceded += 1
                        }
                        _ => {}
                    }
                }
                conceded
            })
            .sum::<u32>()
    };
    let bus = matches(0..12, |_| {
        (
            Squad::generated(presets::tiki_taka(), 83),
            Squad::generated(presets::park_the_bus(), 77),
        )
    });
    let open = matches(0..12, |_| {
        (
            Squad::generated(presets::tiki_taka(), 83),
            Squad::generated(presets::gegenpress(), 77),
        )
    });
    let (b, o) = (in_system(&bus), in_system(&open));
    assert!(b < o, "park the bus conceded {b}, gegenpress {o}");
}

#[test]
fn carriers_are_dispossessed_in_traffic() {
    let sims = matches(0..4, |_| {
        (
            Squad::generated(presets::park_the_bus(), 80),
            Squad::generated(presets::tiki_taka(), 80),
        )
    });
    let n: usize = sims
        .iter()
        .map(|s| {
            s.events
                .iter()
                .filter(|(_, e)| matches!(e, MatchEvent::Dispossessed { .. }))
                .count()
        })
        .sum();
    assert!(n > 0, "nobody was ever crowded off the ball");
}

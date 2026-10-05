//! Trend breakers in numbers: upsets across a rating gap, a star finisher
//! against an ordinary one, and match-day form.
//!
//! ```sh
//! cargo run --release -p football_ai --example trend_report -- 24
//! ```

use football_ai::execution::condition_label;
use football_ai::identity::presets::{self, Squad};
use football_ai::pitch::TeamSide;
use football_ai::role::Role;
use football_ai::{MatchEvent, SimConfig, Simulation};

fn play(seed: u64, home: Squad, away: Squad) -> Simulation {
    let mut sim = Simulation::new(SimConfig::new(seed, home, away));
    sim.run_to_full_time(u32::MAX);
    sim
}

/// Run `n` matches on four threads.
fn batch(n: u64, make: fn(u64) -> (Squad, Squad)) -> Vec<Simulation> {
    let handles: Vec<_> = (0..4u64)
        .map(|t| {
            std::thread::spawn(move || {
                (t..n)
                    .step_by(4)
                    .map(|s| {
                        let (h, a) = make(s);
                        play(s, h, a)
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

fn opp(s: u64) -> Squad {
    Squad::generated(presets::team_archetypes()[s as usize % 6].clone(), 79)
}

/// Wing Play 79, with one of the example stars in his position (0: none,
/// 1: C. Ronaldo up front, 2: K. Mbappe up front, 3: L. Messi on the wing).
fn with_star(which: u8) -> Squad {
    let mut squad = Squad::generated(presets::wing_play(), 79);
    let (profile, role) = match which {
        1 => (presets::example_ronaldo(), Role::Striker),
        2 => (presets::example_mbappe(), Role::Striker),
        3 => (presets::example_messi(), Role::Winger),
        _ => return squad,
    };
    let slot = squad
        .players
        .iter()
        .position(|p| p.position == role)
        .unwrap();
    let mut star = profile;
    star.position = role;
    squad.players[slot] = star;
    squad
}

/// Goals and summed shot xG of the home player in `role`'s first slot.
fn slot_line(sims: &[Simulation], role: Role) -> (u32, f32, u32) {
    let (mut goals, mut xg, mut shots) = (0, 0.0, 0);
    for sim in sims {
        let Some(id) = sim
            .state
            .team_players(TeamSide::Home)
            .iter()
            .find(|p| p.role == role)
            .map(|p| p.id)
        else {
            continue;
        };
        for (_, e) in &sim.events {
            match e {
                MatchEvent::Shot { by, xg: x, .. } if *by == id => {
                    shots += 1;
                    xg += x;
                }
                MatchEvent::Goal {
                    scorer: Some(s), ..
                } if *s == id => goals += 1,
                _ => {}
            }
        }
    }
    (goals, xg, shots)
}

fn points(sims: &[Simulation]) -> f32 {
    let p: u32 = sims
        .iter()
        .map(|s| match s.state.score[0].cmp(&s.state.score[1]) {
            std::cmp::Ordering::Greater => 3,
            std::cmp::Ordering::Equal => 1,
            std::cmp::Ordering::Less => 0,
        })
        .sum();
    p as f32 / sims.len() as f32
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);

    println!(
        "== Upsets: 80-rated favourite at home to a 77-rated side ({n} matches, mixed styles)"
    );
    let sims = batch(n, |s| {
        let styles = presets::team_archetypes();
        (
            Squad::generated(styles[s as usize % 6].clone(), 80),
            Squad::generated(styles[(s as usize + 1) % 6].clone(), 77),
        )
    });
    let (mut w, mut d, mut l) = (0, 0, 0);
    for sim in &sims {
        let [h, a] = sim.state.score;
        if h > a {
            w += 1
        } else if h == a {
            d += 1
        } else {
            l += 1
        }
    }
    println!(
        "favourite W{w} D{d} L{l}  (underdog wins {:.0}%)",
        100.0 * l as f32 / n as f32
    );

    println!("\n== Stars: Wing Play 79 with and without a star, same opponents ({n} matches each)");
    let base = batch(n, |s| (with_star(0), opp(s)));
    let rows: [(&str, Role, Vec<Simulation>); 3] = [
        (
            "C. Ronaldo",
            Role::Striker,
            batch(n, |s| (with_star(1), opp(s))),
        ),
        (
            "K. Mbappe",
            Role::Striker,
            batch(n, |s| (with_star(2), opp(s))),
        ),
        (
            "L. Messi",
            Role::Winger,
            batch(n, |s| (with_star(3), opp(s))),
        ),
    ];
    for (name, role, sims) in &rows {
        let (g, xg, shots) = slot_line(sims, *role);
        let (g0, xg0, shots0) = slot_line(&base, *role);
        println!(
            "{name:<11} {:.2} pts/match (without: {:.2})  his {shots} shots xG {xg:.1} goals {g}  (ordinary {:?}: {shots0} shots xG {xg0:.1} goals {g0})",
            points(sims),
            points(&base),
            role
        );
    }

    println!("\n== Football happens ({n} matches above, without stars)");
    let per = |f: &dyn Fn(&MatchEvent) -> bool| {
        base.iter()
            .map(|s| s.events.iter().filter(|(_, e)| f(e)).count())
            .sum::<usize>() as f32
            / n as f32
    };
    println!(
        "per match: keeper howlers {:.2}  own goals {:.2}  injuries {:.2}  red cards {:.2}",
        per(&|e| matches!(e, MatchEvent::KeeperError { .. })),
        base.iter()
            .flat_map(|s| s.stats.players.iter())
            .map(|p| p.own_goals)
            .sum::<u32>() as f32
            / n as f32,
        per(&|e| matches!(e, MatchEvent::Injury { .. })),
        per(&|e| matches!(e, MatchEvent::RedCard { .. })),
    );

    println!("\n== Match-day form, seed 1");
    let sim = Simulation::new(SimConfig::new(
        1,
        Squad::generated(presets::tiki_taka(), 80),
        Squad::generated(presets::gegenpress(), 80),
    ));
    for side in TeamSide::BOTH {
        let players = sim.state.team_players(side);
        let avg = players.iter().map(|p| p.form).sum::<f32>() / players.len() as f32;
        let labels: Vec<_> = players.iter().map(|p| condition_label(p.form)).collect();
        println!("{side:?}: team day {avg:+.2}  {}", labels.join(", "));
    }
}

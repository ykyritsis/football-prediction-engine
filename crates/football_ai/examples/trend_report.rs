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

fn with_star(star: bool) -> Squad {
    let mut squad = Squad::generated(presets::wing_play(), 79);
    if star {
        let slot = squad
            .players
            .iter()
            .position(|p| p.position == Role::Striker)
            .unwrap();
        let mut cr = presets::example_ronaldo();
        cr.position = Role::Striker;
        squad.players[slot] = cr;
    }
    squad
}

/// Goals and summed shot xG of the home striker(s).
fn striker_line(sims: &[Simulation]) -> (u32, f32, u32) {
    let (mut goals, mut xg, mut shots) = (0, 0.0, 0);
    for sim in sims {
        let strikers: Vec<_> = sim
            .state
            .team_players(TeamSide::Home)
            .iter()
            .filter(|p| p.role == Role::Striker)
            .map(|p| p.id)
            .collect();
        for (_, e) in &sim.events {
            match e {
                MatchEvent::Shot { by, xg: x, .. } if strikers.contains(by) => {
                    shots += 1;
                    xg += x;
                }
                MatchEvent::Goal {
                    scorer: Some(s), ..
                } if strikers.contains(s) => goals += 1,
                _ => {}
            }
        }
    }
    (goals, xg, shots)
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

    println!("\n== Star finisher: Wing Play 79 with C. Ronaldo vs an ordinary striker ({n} matches each)");
    let star = batch(n, |s| (with_star(true), opp(s)));
    let plain = batch(n, |s| (with_star(false), opp(s)));
    for (label, sims) in [("star", &star), ("ordinary", &plain)] {
        let (g, xg, shots) = striker_line(sims);
        println!(
            "{label:<9} {shots:>3} shots  xG {xg:>5.1}  goals {g:>3}  ({:+.1} vs xG)",
            g as f32 - xg
        );
    }

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

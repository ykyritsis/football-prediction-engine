//! Round-robin between the team archetypes, reporting per-style averages.
//!
//! Shows that identity changes how teams play: possession, passing length,
//! pressing height (PPDA, high regains), crossing, tempo.
//!
//! ```text
//! cargo run --release -p football_ai --example style_report -- [minutes] [seeds]
//! ```

use football_ai::identity::presets::{self, Squad};
use football_ai::passing::PassKind;
use football_ai::pitch::TeamSide;
use football_ai::{MatchEvent, SimConfig, Simulation};

#[derive(Default, Clone)]
struct Agg {
    matches: f32,
    goals_for: f32,
    goals_against: f32,
    possession: f32,
    passes: f32,
    completed: f32,
    long: f32,
    through: f32,
    crosses: f32,
    shots: f32,
    xg: f32,
    regains_high: f32,
    def_actions_opp_half: f32,
    opp_passes_own_half: f32,
    avg_pass_len: f32,
    pass_len_n: f32,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let minutes: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(90.0);
    let seeds: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let styles = presets::team_archetypes();
    let mut agg = vec![Agg::default(); styles.len()];

    for (i, a) in styles.iter().enumerate() {
        for (j, b) in styles.iter().enumerate() {
            if i == j {
                continue;
            }
            for seed in 0..seeds {
                let cfg = SimConfig {
                    seed: 1000 + seed * 31 + (i * 7 + j) as u64,
                    dt: 1.0 / 30.0,
                    home: Squad::generated(a.clone(), 80),
                    away: Squad::generated(b.clone(), 80),
                };
                let mut sim = Simulation::new(cfg);
                let ticks = (minutes * 60.0 / sim.state.dt) as u32;
                let mut poss = [0f32; 2];
                for _ in 0..ticks {
                    let before = sim.events.len();
                    let pre_ball = sim.state.ball.pos;
                    sim.step();
                    poss[sim.state.possession.index()] += 1.0;
                    for (_, e) in &sim.events[before..] {
                        match *e {
                            MatchEvent::Pass { from, kind, .. } => {
                                let t = from.team();
                                let g = &mut agg[if t == TeamSide::Home { i } else { j }];
                                g.passes += 1.0;
                                match kind {
                                    PassKind::Long => g.long += 1.0,
                                    PassKind::Through => g.through += 1.0,
                                    PassKind::Cross => g.crosses += 1.0,
                                    PassKind::Short => {}
                                }
                                if let Some(p) = sim.state.ball.pass {
                                    g.avg_pass_len += p.origin.distance(p.target);
                                    g.pass_len_n += 1.0;
                                }
                                // Opponent passes in their own 60% (for PPDA of the other side).
                                if t.to_local(pre_ball).x < 10.0 {
                                    agg[if t == TeamSide::Home { j } else { i }].opp_passes_own_half += 1.0;
                                }
                            }
                            MatchEvent::PassCompleted { from, .. } => {
                                agg[if from.team() == TeamSide::Home { i } else { j }].completed += 1.0
                            }
                            MatchEvent::Shot { by, xg } => {
                                let g = &mut agg[if by.team() == TeamSide::Home { i } else { j }];
                                g.shots += 1.0;
                                g.xg += xg;
                            }
                            MatchEvent::Tackle { by, .. } | MatchEvent::Interception { by, .. } => {
                                let t = by.team();
                                let g = &mut agg[if t == TeamSide::Home { i } else { j }];
                                if t.to_local(pre_ball).x > -10.0 {
                                    g.def_actions_opp_half += 1.0;
                                }
                                let won = matches!(*e, MatchEvent::Interception { .. } | MatchEvent::Tackle { won: true, .. });
                                if won && t.to_local(pre_ball).x > 17.5 {
                                    g.regains_high += 1.0;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                let total = poss[0] + poss[1];
                for (k, side) in [(i, 0usize), (j, 1usize)] {
                    let g = &mut agg[k];
                    g.matches += 1.0;
                    g.possession += poss[side] / total;
                    g.goals_for += sim.state.score[side] as f32;
                    g.goals_against += sim.state.score[1 - side] as f32;
                }
            }
        }
    }

    println!(
        "{:<15} {:>5} {:>5} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "style", "GF", "GA", "poss", "passes", "compl", "len m", "long%", "thru", "cross", "shots", "xG", "PPDA"
    );
    for (s, g) in styles.iter().zip(&agg) {
        let m = g.matches.max(1.0);
        let ppda = g.opp_passes_own_half / g.def_actions_opp_half.max(1.0);
        println!(
            "{:<15} {:>5.2} {:>5.2} {:>4.0}% {:>6.0} {:>5.0}% {:>6.1} {:>5.0}% {:>6.1} {:>6.1} {:>6.1} {:>6.2} {:>6.2}   high regains {:>5.1}",
            s.name,
            g.goals_for / m,
            g.goals_against / m,
            100.0 * g.possession / m,
            g.passes / m,
            100.0 * g.completed / g.passes.max(1.0),
            g.avg_pass_len / g.pass_len_n.max(1.0),
            100.0 * g.long / g.passes.max(1.0),
            g.through / m,
            g.crosses / m,
            g.shots / m,
            g.xg / m,
            ppda,
            g.regains_high / m,
        );
    }
}

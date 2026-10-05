//! Calibration scorecard: average per-team match statistics over a
//! round of matches between every pair of team archetypes, next to typical
//! top-flight values, plus the upset rate across a rating gap.
//!
//! ```text
//! cargo run --release -p football_ai --example calibration -- [matches]
//! ```

use football_ai::identity::presets::{self, Squad};
use football_ai::stats::TeamStats;
use football_ai::{MatchEvent, SimConfig, Simulation};

/// Run `n` matches on four threads; `make(i)` gives seed, home and away.
fn batch(n: usize, make: fn(usize) -> (u64, Squad, Squad)) -> Vec<Simulation> {
    let handles: Vec<_> = (0..4)
        .map(|t| {
            std::thread::spawn(move || {
                (t..n)
                    .step_by(4)
                    .map(|i| {
                        let (seed, h, a) = make(i);
                        let mut sim = Simulation::new(SimConfig::new(seed, h, a));
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

fn pairing(i: usize) -> (usize, usize) {
    // Every ordered pair of different archetypes: 30 pairings.
    let a = i % 6;
    let b = (a + 1 + (i / 6) % 5) % 6;
    (a, b)
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    let sims = batch(n, |i| {
        let styles = presets::team_archetypes();
        let (a, b) = pairing(i);
        (
            i as u64,
            Squad::generated(styles[a].clone(), 80),
            Squad::generated(styles[b].clone(), 80),
        )
    });
    let teams: Vec<&TeamStats> = sims.iter().flat_map(|s| s.stats.teams.iter()).collect();
    let k = teams.len() as f32;
    let avg = |f: &dyn Fn(&TeamStats) -> f32| teams.iter().map(|t| f(t)).sum::<f32>() / k;

    let in_play: f32 = sims
        .iter()
        .map(|s| {
            (s.stats.teams[0].possession_ticks + s.stats.teams[1].possession_ticks) as f32
                * s.state.dt
                / 60.0
        })
        .sum::<f32>()
        / sims.len() as f32;
    println!("Per team per match, {n} matches between archetypes rated 80");
    println!("ball in play {in_play:.0} min per match (real: 55-60)\n");
    println!("{:<18} {:>8}   {:>12}", "statistic", "sim", "real (approx)");
    let rows: Vec<(&str, f32, &str)> = vec![
        ("goals", avg(&|t| t.goals as f32), "1.4"),
        ("shots", avg(&|t| t.shots as f32), "12-13"),
        ("on target", avg(&|t| t.shots_on_target as f32), "4-5"),
        ("xG", avg(&|t| t.xg), "1.3-1.5"),
        ("passes", avg(&|t| t.passes as f32), "400-550"),
        (
            "pass accuracy %",
            100.0 * avg(&|t| t.pass_accuracy()),
            "78-84",
        ),
        ("crosses", avg(&|t| t.crosses as f32), "15-20"),
        ("through balls", avg(&|t| t.through_balls as f32), "2-6"),
        ("long balls", avg(&|t| t.long_balls as f32), "50-70"),
        ("tackles", avg(&|t| t.tackles as f32), "16-20"),
        ("tackles won", avg(&|t| t.tackles_won as f32), "10-13"),
        ("interceptions", avg(&|t| t.interceptions as f32), "9-12"),
        (
            "possessions lost",
            avg(&|t| t.possessions_lost as f32),
            "100-130",
        ),
        (
            "regains att. 3rd",
            avg(&|t| t.regains_final_third as f32),
            "5-10",
        ),
        ("saves", avg(&|t| t.saves as f32), "3"),
        ("fouls", avg(&|t| t.fouls as f32), "10-12"),
        ("yellow cards", avg(&|t| t.yellow_cards as f32), "1.7"),
        ("corners", avg(&|t| t.corners as f32), "5"),
        ("offsides", avg(&|t| t.offsides as f32), "2"),
    ];
    for (label, sim, real) in rows {
        println!("{label:<18} {sim:>8.1}   {real:>12}");
    }

    // Unpredictability: football is a low-scoring game where the better
    // side often does not win.
    let m = sims.len() as f32;
    let (mut xg_fail, mut first_fail, mut with_goals, mut nil_nil, mut four_plus) = (0, 0, 0, 0, 0);
    let (mut goals, mut lucky, mut own_goals) = (0, 0, 0);
    for sim in &sims {
        let [h, a] = sim.state.score;
        let t = &sim.stats.teams;
        if (t[0].xg - t[1].xg).abs() > 0.3 {
            let better = if t[0].xg > t[1].xg { 0 } else { 1 };
            if sim.state.score[better] <= sim.state.score[1 - better] {
                xg_fail += 1;
            }
        }
        if h + a == 0 {
            nil_nil += 1;
        }
        if h + a >= 4 {
            four_plus += 1;
        }
        // Who scored first, and did they win?
        let mut last_shot_xg = [0.0f32; 2];
        let mut first = None;
        for (_, e) in &sim.events {
            match e {
                MatchEvent::Shot { by, xg, .. } => last_shot_xg[by.team().index()] = *xg,
                MatchEvent::Goal { team, scorer } => {
                    goals += 1;
                    first.get_or_insert(*team);
                    if scorer.is_some_and(|s| s.team() != *team) {
                        own_goals += 1;
                    } else if last_shot_xg[team.index()] < 0.05 {
                        lucky += 1;
                    }
                    last_shot_xg[team.index()] = 1.0;
                }
                _ => {}
            }
        }
        if let Some(f) = first {
            with_goals += 1;
            if sim.state.score[f.index()] <= sim.state.score[f.opponent().index()] {
                first_fail += 1;
            }
        }
    }
    println!(
        "\nUnpredictability ({} matches)            sim   real (approx)",
        sims.len()
    );
    let pct = |x: i32, of: f32| 100.0 * x as f32 / of.max(1.0);
    println!(
        "xG winner (by 0.3+) fails to win    {:>5.0}%   35-40%",
        pct(xg_fail, m)
    );
    println!(
        "team scoring first fails to win     {:>5.0}%   30-35%",
        pct(first_fail, with_goals as f32)
    );
    println!(
        "0-0                                 {:>5.0}%   7-8%",
        pct(nil_nil, m)
    );
    println!(
        "4+ goals                            {:>5.0}%   20-25%",
        pct(four_plus, m)
    );
    println!(
        "goals from shots under 0.05 xG      {:>5.0}%   8-10%",
        pct(lucky, goals as f32)
    );
    println!(
        "own goals per match                 {:>6.2}   0.1",
        own_goals as f32 / m
    );
    let count = |f: &dyn Fn(&MatchEvent) -> bool| {
        sims.iter()
            .map(|s| s.events.iter().filter(|(_, e)| f(e)).count())
            .sum::<usize>() as f32
            / m
    };
    println!(
        "red cards per match                 {:>6.2}   0.1-0.2",
        count(&|e| matches!(e, MatchEvent::RedCard { .. }))
    );
    println!(
        "injuries per match                  {:>6.2}   0.3-0.5",
        count(&|e| matches!(e, MatchEvent::Injury { .. }))
    );
    println!(
        "keeper howlers per match            {:>6.2}   0.05-0.1",
        count(&|e| matches!(e, MatchEvent::KeeperError { .. }))
    );

    let upsets = batch(n.min(24), |i| {
        let styles = presets::team_archetypes();
        let (a, b) = pairing(i);
        (
            1000 + i as u64,
            Squad::generated(styles[a].clone(), 80),
            Squad::generated(styles[b].clone(), 77),
        )
    });
    let (mut w, mut d, mut l) = (0, 0, 0);
    for sim in &upsets {
        let [h, a] = sim.state.score;
        match h.cmp(&a) {
            std::cmp::Ordering::Greater => w += 1,
            std::cmp::Ordering::Equal => d += 1,
            std::cmp::Ordering::Less => l += 1,
        }
    }
    println!(
        "\n80 vs 77 ({} matches): favourite W{w} D{d} L{l}   (real: roughly W50% D25% L25%)",
        upsets.len()
    );
}

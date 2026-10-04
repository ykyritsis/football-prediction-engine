//! Calibration scorecard: average per-team match statistics over a
//! round of matches between every pair of team archetypes, next to typical
//! top-flight values, plus the upset rate across a rating gap.
//!
//! ```text
//! cargo run --release -p football_ai --example calibration -- [matches]
//! ```

use football_ai::identity::presets::{self, Squad};
use football_ai::stats::TeamStats;
use football_ai::{SimConfig, Simulation};

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

    println!("Per team per match, {n} matches between archetypes rated 80\n");
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
        ("saves", avg(&|t| t.saves as f32), "3"),
        ("fouls", avg(&|t| t.fouls as f32), "10-12"),
        ("yellow cards", avg(&|t| t.yellow_cards as f32), "1.7"),
        ("corners", avg(&|t| t.corners as f32), "5"),
        ("offsides", avg(&|t| t.offsides as f32), "2"),
    ];
    for (label, sim, real) in rows {
        println!("{label:<18} {sim:>8.1}   {real:>12}");
    }

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

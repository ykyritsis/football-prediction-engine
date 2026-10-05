//! Season projection: Monte Carlo over the remaining fixtures.

use crate::data::Match;
use crate::model::Model;
use std::collections::BTreeMap;

/// Projected outcome for one club.
#[derive(Debug, Clone, Default)]
pub struct Projection {
    pub team: String,
    pub points_now: u32,
    pub played: u32,
    pub mean_points: f64,
    pub title: f64,
    pub top4: f64,
    /// Finishing in the bottom three (in 18-team leagues this includes the
    /// relegation play-off place).
    pub bottom3: f64,
    pub mean_position: f64,
}

/// Small deterministic RNG (xorshift64*).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Project the season of `fixtures` (one season, played and unplayed) with
/// `model`, over `runs` simulations.
pub fn project(fixtures: &[Match], model: &Model, runs: usize, seed: u64) -> Vec<Projection> {
    let mut teams: BTreeMap<String, usize> = BTreeMap::new();
    for m in fixtures {
        let n = teams.len();
        teams.entry(m.home.clone()).or_insert(n);
        let n = teams.len();
        teams.entry(m.away.clone()).or_insert(n);
    }
    let n = teams.len();
    let mut base_pts = vec![0u32; n];
    let mut base_gd = vec![0i32; n];
    let mut played = vec![0u32; n];
    let mut remaining = Vec::new();
    for m in fixtures {
        let (h, a) = (teams[&m.home], teams[&m.away]);
        match m.score {
            Some((gh, ga)) => {
                played[h] += 1;
                played[a] += 1;
                base_gd[h] += gh as i32 - ga as i32;
                base_gd[a] += ga as i32 - gh as i32;
                match gh.cmp(&ga) {
                    std::cmp::Ordering::Greater => base_pts[h] += 3,
                    std::cmp::Ordering::Equal => {
                        base_pts[h] += 1;
                        base_pts[a] += 1
                    }
                    std::cmp::Ordering::Less => base_pts[a] += 3,
                }
            }
            None => {
                // Cumulative scoreline distribution for sampling.
                let p = model.predict(&m.home, &m.away);
                let mut cdf = Vec::with_capacity(121);
                let mut acc = 0.0;
                for (gh, row) in p.scores.iter().enumerate() {
                    for (ga, &q) in row.iter().enumerate() {
                        acc += q;
                        cdf.push((acc, gh as i32, ga as i32));
                    }
                }
                remaining.push((h, a, cdf));
            }
        }
    }
    let mut rng = Rng(seed | 1);
    let mut sum_pts = vec![0.0; n];
    let mut sum_pos = vec![0.0; n];
    let (mut title, mut top4, mut bottom3) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    for _ in 0..runs {
        let mut pts = base_pts.clone();
        let mut gd = base_gd.clone();
        for (h, a, cdf) in &remaining {
            let u = rng.next();
            let &(_, gh, ga) = cdf.iter().find(|c| c.0 >= u).unwrap_or(cdf.last().unwrap());
            gd[*h] += gh - ga;
            gd[*a] += ga - gh;
            match gh.cmp(&ga) {
                std::cmp::Ordering::Greater => pts[*h] += 3,
                std::cmp::Ordering::Equal => {
                    pts[*h] += 1;
                    pts[*a] += 1
                }
                std::cmp::Ordering::Less => pts[*a] += 3,
            }
        }
        let mut order: Vec<usize> = (0..n).collect();
        // Points, then goal difference, then a random tiebreak.
        let tiebreak: Vec<f64> = (0..n).map(|_| rng.next()).collect();
        order.sort_by(|&x, &y| {
            pts[y]
                .cmp(&pts[x])
                .then(gd[y].cmp(&gd[x]))
                .then(tiebreak[x].partial_cmp(&tiebreak[y]).unwrap())
        });
        for (pos, &t) in order.iter().enumerate() {
            sum_pts[t] += pts[t] as f64;
            sum_pos[t] += (pos + 1) as f64;
            if pos == 0 {
                title[t] += 1.0;
            }
            if pos < 4 {
                top4[t] += 1.0;
            }
            if pos + 3 >= n {
                bottom3[t] += 1.0;
            }
        }
    }
    let r = runs as f64;
    let mut out: Vec<Projection> = teams
        .iter()
        .map(|(name, &i)| Projection {
            team: name.clone(),
            points_now: base_pts[i],
            played: played[i],
            mean_points: sum_pts[i] / r,
            title: title[i] / r,
            top4: top4[i] / r,
            bottom3: bottom3[i] / r,
            mean_position: sum_pos[i] / r,
        })
        .collect();
    out.sort_by(|a, b| a.mean_position.partial_cmp(&b.mean_position).unwrap());
    out
}

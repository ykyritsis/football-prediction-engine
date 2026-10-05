//! Dixon–Coles goals model with time decay.

use crate::data::{day_number, Match};
use std::collections::BTreeMap;

/// Fitting choices.
#[derive(Debug, Clone, Copy)]
pub struct ModelConfig {
    /// A match this many days old counts half as much as one played today.
    pub half_life_days: f64,
    /// Prior standard deviation of attack/defence strengths (log scale).
    pub prior_sd: f64,
    /// How much weaker than average a club with no data is assumed to be
    /// (log scale, applied to attack and defence alike).
    pub newcomer_penalty: f64,
}

impl Default for ModelConfig {
    fn default() -> Self {
        ModelConfig {
            half_life_days: 240.0,
            prior_sd: 0.35,
            newcomer_penalty: 0.25,
        }
    }
}

/// Fitted strengths for one league.
#[derive(Debug, Clone)]
pub struct Model {
    pub teams: BTreeMap<String, Strength>,
    /// Log of the average away side's goal rate.
    pub mu: f64,
    /// Home advantage (log scale).
    pub home: f64,
    /// Dixon–Coles low-score dependence.
    pub rho: f64,
    newcomer: Strength,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Strength {
    pub attack: f64,
    pub defence: f64,
    /// Total decayed weight of the club's matches (how much data it has).
    pub weight: f64,
}

/// A predicted match.
#[derive(Debug, Clone)]
pub struct Prediction {
    pub home_win: f64,
    pub draw: f64,
    pub away_win: f64,
    pub home_xg: f64,
    pub away_xg: f64,
    pub most_likely_score: (u8, u8),
    pub over_2_5: f64,
    pub both_score: f64,
    /// `scores[h][a]` = P(home h, away a), 0..=10 goals each.
    pub scores: Vec<Vec<f64>>,
}

const MAX_GOALS: usize = 10;

fn tau(h: u8, a: u8, lh: f64, la: f64, rho: f64) -> f64 {
    match (h, a) {
        (0, 0) => 1.0 - lh * la * rho,
        (0, 1) => 1.0 + lh * rho,
        (1, 0) => 1.0 + la * rho,
        (1, 1) => 1.0 - rho,
        _ => 1.0,
    }
}

fn poisson(k: usize, lambda: f64) -> f64 {
    let mut p = (-lambda).exp();
    for i in 1..=k {
        p *= lambda / i as f64;
    }
    p
}

impl Model {
    /// Fit to the played matches in `history`, weighting by age relative to
    /// `as_of` (ISO date).
    pub fn fit(history: &[Match], as_of: &str, cfg: &ModelConfig) -> Model {
        let today = day_number(as_of);
        let decay = std::f64::consts::LN_2 / cfg.half_life_days;
        struct Row {
            h: usize,
            a: usize,
            gh: f64,
            ga: f64,
            w: f64,
        }
        let mut index: BTreeMap<String, usize> = BTreeMap::new();
        let mut rows = Vec::new();
        for m in history {
            let Some((gh, ga)) = m.score else { continue };
            let age = (today - day_number(&m.date)).max(0) as f64;
            let w = (-decay * age).exp();
            if w < 1e-3 {
                continue;
            }
            let n = index.len();
            let h = *index.entry(m.home.clone()).or_insert(n);
            let n = index.len();
            let a = *index.entry(m.away.clone()).or_insert(n);
            rows.push(Row {
                h,
                a,
                gh: gh as f64,
                ga: ga as f64,
                w,
            });
        }
        let n = index.len();
        let mut att = vec![0.0; n];
        let mut def = vec![0.0; n];
        let mut weight = vec![0.0; n];
        for r in &rows {
            weight[r.h] += r.w;
            weight[r.a] += r.w;
        }
        // Clubs with little data lean towards "a bit below average".
        let prior_mean: Vec<f64> = weight
            .iter()
            .map(|w| -cfg.newcomer_penalty * (-w / 8.0).exp())
            .collect();
        let inv_var = 1.0 / (cfg.prior_sd * cfg.prior_sd);
        let (mut mu, mut home) = (0.1f64, 0.25f64);
        for _sweep in 0..200 {
            let mut change = 0.0f64;
            // Attack.
            let mut g = vec![0.0; n];
            let mut hss = vec![0.0; n];
            for r in &rows {
                let lh = (mu + home + att[r.h] - def[r.a]).exp();
                let la = (mu + att[r.a] - def[r.h]).exp();
                g[r.h] += r.w * (r.gh - lh);
                hss[r.h] += r.w * lh;
                g[r.a] += r.w * (r.ga - la);
                hss[r.a] += r.w * la;
            }
            for i in 0..n {
                let step = (g[i] - (att[i] - prior_mean[i]) * inv_var) / (hss[i] + inv_var);
                att[i] += step;
                change = change.max(step.abs());
            }
            // Defence.
            let mut g = vec![0.0; n];
            let mut hss = vec![0.0; n];
            for r in &rows {
                let lh = (mu + home + att[r.h] - def[r.a]).exp();
                let la = (mu + att[r.a] - def[r.h]).exp();
                g[r.a] -= r.w * (r.gh - lh);
                hss[r.a] += r.w * lh;
                g[r.h] -= r.w * (r.ga - la);
                hss[r.h] += r.w * la;
            }
            for i in 0..n {
                let step = (g[i] - (def[i] - prior_mean[i]) * inv_var) / (hss[i] + inv_var);
                def[i] += step;
                change = change.max(step.abs());
            }
            // Global level and home advantage.
            let (mut goals, mut expect, mut hg, mut he) = (0.0, 0.0, 0.0, 0.0);
            for r in &rows {
                let lh = (mu + home + att[r.h] - def[r.a]).exp();
                let la = (mu + att[r.a] - def[r.h]).exp();
                goals += r.w * (r.gh + r.ga);
                expect += r.w * (lh + la);
                hg += r.w * r.gh;
                he += r.w * lh;
            }
            if expect > 0.0 {
                let s = (goals / expect).ln();
                mu += s;
                change = change.max(s.abs());
            }
            if he > 0.0 {
                let s = (hg / he).ln() * 0.5;
                home += s;
                change = change.max(s.abs());
            }
            if change < 1e-7 {
                break;
            }
        }
        // Low-score correction by grid search.
        let mut best = (f64::NEG_INFINITY, 0.0);
        for k in -25..=15 {
            let rho = k as f64 / 100.0;
            let ll: f64 = rows
                .iter()
                .map(|r| {
                    let lh = (mu + home + att[r.h] - def[r.a]).exp();
                    let la = (mu + att[r.a] - def[r.h]).exp();
                    r.w * tau(r.gh as u8, r.ga as u8, lh, la, rho).max(1e-9).ln()
                })
                .sum();
            if ll > best.0 {
                best = (ll, rho);
            }
        }
        let mut teams = BTreeMap::new();
        for (name, &i) in &index {
            teams.insert(
                name.clone(),
                Strength {
                    attack: att[i],
                    defence: def[i],
                    weight: weight[i],
                },
            );
        }
        Model {
            teams,
            mu,
            home,
            rho: best.1,
            newcomer: Strength {
                attack: -cfg.newcomer_penalty,
                defence: -cfg.newcomer_penalty,
                weight: 0.0,
            },
        }
    }

    /// Strength of `team`, or the newcomer prior for a club never seen.
    pub fn strength(&self, team: &str) -> Strength {
        self.teams.get(team).copied().unwrap_or(self.newcomer)
    }

    /// Expected goals for each side.
    pub fn rates(&self, home: &str, away: &str) -> (f64, f64) {
        let (h, a) = (self.strength(home), self.strength(away));
        (
            (self.mu + self.home + h.attack - a.defence).exp(),
            (self.mu + a.attack - h.defence).exp(),
        )
    }

    /// Full prediction for `home` against `away`.
    pub fn predict(&self, home: &str, away: &str) -> Prediction {
        let (lh, la) = self.rates(home, away);
        let mut scores = vec![vec![0.0; MAX_GOALS + 1]; MAX_GOALS + 1];
        let mut total = 0.0;
        for (h, row) in scores.iter_mut().enumerate() {
            for (a, cell) in row.iter_mut().enumerate() {
                let p = poisson(h, lh) * poisson(a, la) * tau(h as u8, a as u8, lh, la, self.rho);
                *cell = p.max(0.0);
                total += *cell;
            }
        }
        let (mut hw, mut dr, mut aw, mut over, mut btts) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let mut best = (0.0, (0u8, 0u8));
        for (h, row) in scores.iter_mut().enumerate() {
            for (a, cell) in row.iter_mut().enumerate() {
                *cell /= total;
                let p = *cell;
                match h.cmp(&a) {
                    std::cmp::Ordering::Greater => hw += p,
                    std::cmp::Ordering::Equal => dr += p,
                    std::cmp::Ordering::Less => aw += p,
                }
                if h + a > 2 {
                    over += p;
                }
                if h > 0 && a > 0 {
                    btts += p;
                }
                if p > best.0 {
                    best = (p, (h as u8, a as u8));
                }
            }
        }
        Prediction {
            home_win: hw,
            draw: dr,
            away_win: aw,
            home_xg: lh,
            away_xg: la,
            most_likely_score: best.1,
            over_2_5: over,
            both_score: btts,
            scores,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(date: &str, h: &str, a: &str, gh: u8, ga: u8) -> Match {
        Match {
            date: date.into(),
            home: h.into(),
            away: a.into(),
            score: Some((gh, ga)),
        }
    }

    #[test]
    fn stronger_team_is_favoured_and_probabilities_sum_to_one() {
        let mut hist = Vec::new();
        for k in 0..20 {
            let d = crate::data::add_days("2025-08-01", k * 7);
            hist.push(m(&d, "Strong", "Weak", 3, 0));
            hist.push(m(&d, "Weak", "Strong", 0, 2));
            hist.push(m(&d, "Mid", "Weak", 1, 0));
            hist.push(m(&d, "Strong", "Mid", 2, 1));
        }
        let model = Model::fit(&hist, "2026-01-01", &ModelConfig::default());
        let p = model.predict("Strong", "Weak");
        assert!((p.home_win + p.draw + p.away_win - 1.0).abs() < 1e-9);
        assert!(p.home_win > 0.7, "{p:?}");
        assert!(p.home_xg > p.away_xg);
        let q = model.predict("Weak", "Strong");
        assert!(q.away_win > q.home_win);
    }
}

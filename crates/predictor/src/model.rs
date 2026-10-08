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
    /// Share of each side's goal tally replaced by its shots on target
    /// scaled to goals (0 = goals only). Shots are less noisy than goals,
    /// so they reveal strength sooner. Needs [`Match::stats`].
    pub shot_weight: f64,
    /// Prior standard deviation of each club's own home advantage around the
    /// league's (0 = one shared value). Some grounds are harder to visit.
    pub home_sd: f64,
    /// Multiplies strength differences at prediction time. Exponential
    /// decay averages a club over months, which blurs how far apart clubs
    /// are now; the backtest shows favourites win more often than an
    /// unscaled fit says.
    pub spread: f64,
}

impl Default for ModelConfig {
    fn default() -> Self {
        ModelConfig {
            half_life_days: 365.0,
            prior_sd: 0.5,
            newcomer_penalty: 0.25,
            shot_weight: 0.3,
            home_sd: 0.1,
            spread: 1.15,
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
    spread: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Strength {
    pub attack: f64,
    pub defence: f64,
    /// Total decayed weight of the club's matches (how much data it has).
    pub weight: f64,
    /// This club's home advantage relative to the league's (log scale).
    pub home: f64,
}

/// A predicted match.
#[derive(Debug, Clone, serde::Serialize)]
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
    /// Fit to played matches strictly before `as_of` (ISO date), weighting
    /// by age. Same-day and future scores and statistics are excluded.
    pub fn fit(history: &[Match], as_of: &str, cfg: &ModelConfig) -> Model {
        let today = day_number(as_of);
        let decay = std::f64::consts::LN_2 / cfg.half_life_days;
        struct Row {
            h: usize,
            a: usize,
            gh: f64,
            ga: f64,
            /// Actual goals (for the low-score correction).
            goals: (u8, u8),
            w: f64,
            home_edge: f64,
        }
        let mut index: BTreeMap<String, usize> = BTreeMap::new();
        let mut rows = Vec::new();
        // Goals per shot on target, for the shot blend.
        let (mut g_sum, mut sot_sum) = (0.0, 0.0);
        for m in history.iter().filter(|m| m.date.as_str() < as_of) {
            if let (Some((gh, ga)), Some(s)) = (m.score, &m.stats) {
                g_sum += (gh + ga) as f64;
                sot_sum += (s.shots_on_target.0 + s.shots_on_target.1) as f64;
            }
        }
        let per_sot = if sot_sum > 0.0 { g_sum / sot_sum } else { 0.0 };
        for m in history.iter().filter(|m| m.date.as_str() < as_of) {
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
            let (mut th, mut ta) = (gh as f64, ga as f64);
            if let (Some(s), true) = (&m.stats, cfg.shot_weight > 0.0 && per_sot > 0.0) {
                let k = cfg.shot_weight;
                th = (1.0 - k) * th + k * per_sot * s.shots_on_target.0 as f64;
                ta = (1.0 - k) * ta + k * per_sot * s.shots_on_target.1 as f64;
            }
            rows.push(Row {
                h,
                a,
                gh: th,
                ga: ta,
                goals: (gh, ga),
                w,
                home_edge: if m.neutral { 0.0 } else { 1.0 },
            });
        }
        let n = index.len();
        let mut att = vec![0.0; n];
        let mut def = vec![0.0; n];
        let mut weight = vec![0.0; n];
        let mut hadv = vec![0.0; n];
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
                let lh = (mu + r.home_edge * (home + hadv[r.h]) + att[r.h] - def[r.a]).exp();
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
                let lh = (mu + r.home_edge * (home + hadv[r.h]) + att[r.h] - def[r.a]).exp();
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
            // Each club's own home advantage.
            if cfg.home_sd > 0.0 {
                let prec = 1.0 / (cfg.home_sd * cfg.home_sd);
                let mut g = vec![0.0; n];
                let mut hss = vec![0.0; n];
                for r in &rows {
                    let lh = (mu + r.home_edge * (home + hadv[r.h]) + att[r.h] - def[r.a]).exp();
                    g[r.h] += r.w * r.home_edge * (r.gh - lh);
                    hss[r.h] += r.w * r.home_edge * lh;
                }
                for i in 0..n {
                    let step = (g[i] - hadv[i] * prec) / (hss[i] + prec);
                    hadv[i] += step;
                    change = change.max(step.abs());
                }
            }
            // Global level and home advantage.
            let (mut goals, mut expect, mut hg, mut he) = (0.0, 0.0, 0.0, 0.0);
            for r in &rows {
                let lh = (mu + r.home_edge * (home + hadv[r.h]) + att[r.h] - def[r.a]).exp();
                let la = (mu + att[r.a] - def[r.h]).exp();
                goals += r.w * (r.gh + r.ga);
                expect += r.w * (lh + la);
                hg += r.w * r.home_edge * r.gh;
                he += r.w * r.home_edge * lh;
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
                    let lh = (mu + r.home_edge * (home + hadv[r.h]) + att[r.h] - def[r.a]).exp();
                    let la = (mu + att[r.a] - def[r.h]).exp();
                    r.w * tau(r.goals.0, r.goals.1, lh, la, rho).max(1e-9).ln()
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
                    home: hadv[i],
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
                home: 0.0,
            },
            spread: cfg.spread,
        }
    }

    /// Shift a club's strengths (team news: injuries, suspensions).
    pub fn adjust(&mut self, team: &str, attack: f64, defence: f64) {
        let s = self.teams.entry(team.to_string()).or_insert(self.newcomer);
        s.attack += attack;
        s.defence += defence;
    }

    /// Strength of `team`, or the newcomer prior for a club never seen.
    pub fn strength(&self, team: &str) -> Strength {
        self.teams.get(team).copied().unwrap_or(self.newcomer)
    }

    /// Expected goals for each side.
    pub fn rates(&self, home: &str, away: &str) -> (f64, f64) {
        let (h, a) = (self.strength(home), self.strength(away));
        (
            (self.mu + self.home + h.home + self.spread * (h.attack - a.defence)).exp(),
            (self.mu + self.spread * (a.attack - h.defence)).exp(),
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

    /// Ninety-minute outcome at a neutral ground, with both home effects removed.
    pub fn predict_neutral(&self, home: &str, away: &str) -> Prediction {
        let mut neutral = self.clone();
        neutral.home = 0.0;
        for strength in neutral.teams.values_mut() {
            strength.home = 0.0;
        }
        neutral.predict(home, away)
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
            neutral: false,
            stats: None,
        }
    }

    #[test]
    fn neutral_venue_removes_both_home_effects() {
        let hist = vec![
            m("2025-08-01", "A", "B", 3, 1),
            m("2025-08-02", "B", "A", 2, 1),
        ];
        let model = Model::fit(&hist, "2025-08-03", &ModelConfig::default());
        let p = model.predict_neutral("A", "B");
        let q = model.predict_neutral("B", "A");
        assert!((p.home_win - q.away_win).abs() < 1e-10);
        assert!((p.home_xg - q.away_xg).abs() < 1e-10);
        assert!((p.home_win + p.draw + p.away_win - 1.0).abs() < 1e-10);
        assert_ne!(model.predict("A", "B").scores, p.scores);
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

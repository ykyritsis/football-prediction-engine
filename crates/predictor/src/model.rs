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
    /// Prior standard deviation of a group of clubs' strength offset (log
    /// scale) when several competitions are fitted together (see [`Pooling`]).
    pub league_sd: f64,
    /// Prior standard deviation of each competition's goal level and home
    /// advantage around the average across pooled competitions (log scale).
    pub level_sd: f64,
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
            league_sd: 0.4,
            level_sd: 0.15,
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
    /// Goal level and home advantage of every fitted competition (index =
    /// `Match::comp`). `mu` and `home` above are those of `target`.
    levels: Vec<(f64, f64)>,
    target: usize,
}

/// Fit several competitions at once, so that a club's league games inform its
/// European odds. Clubs share one attack and defence strength across all
/// competitions, each competition has its own goal level and home advantage,
/// and each group of clubs (normally a domestic league) has a strength offset
/// that lines the leagues up wherever their clubs meet.
#[derive(Debug, Clone)]
pub struct Pooling {
    /// Group of each club. Clubs not listed fall in `default_group`.
    pub groups: BTreeMap<String, usize>,
    pub default_group: usize,
    pub n_groups: usize,
    /// Competition (`Match::comp`) whose goal level the fitted model predicts.
    pub target: usize,
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
    /// The single most probable scoreline. Because probability is spread over
    /// many scores this is nearly always 1-1, 1-0 or 2-1, so it often
    /// contradicts the most likely result. See `predicted_score`.
    pub most_likely_score: (u8, u8),
    /// The most probable scoreline among those that produce the most likely
    /// result (home win, draw or away win), so the score agrees with the odds.
    pub predicted_score: (u8, u8),
    pub over_2_5: f64,
    pub both_score: f64,
    /// Share of the bookmaker market in the result probabilities (0 = model only).
    pub market_weight: f64,
    /// `scores[h][a]` = P(home h, away a), 0..=10 goals each.
    pub scores: Vec<Vec<f64>>,
}

impl Prediction {
    /// Normalise a scoreline distribution and derive every summary from it.
    fn from_scores(mut scores: Vec<Vec<f64>>, home_xg: f64, away_xg: f64) -> Prediction {
        let total: f64 = scores.iter().flatten().sum();
        let (mut hw, mut dr, mut aw, mut over, mut btts) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let mut best = (0.0, (0u8, 0u8));
        let mut cells = Vec::new();
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
                cells.push((p, h, a));
            }
        }
        // Most probable scoreline among those that give the most likely result.
        let top = if hw >= dr && hw >= aw {
            std::cmp::Ordering::Greater
        } else if dr >= aw {
            std::cmp::Ordering::Equal
        } else {
            std::cmp::Ordering::Less
        };
        let mut predicted = (0.0, (0u8, 0u8));
        for &(p, h, a) in &cells {
            if h.cmp(&a) == top && p > predicted.0 {
                predicted = (p, (h as u8, a as u8));
            }
        }
        Prediction {
            home_win: hw,
            draw: dr,
            away_win: aw,
            home_xg,
            away_xg,
            most_likely_score: best.1,
            predicted_score: predicted.1,
            over_2_5: over,
            both_score: btts,
            market_weight: 0.0,
            scores,
        }
    }

    /// Blend the home, draw and away probabilities with a bookmaker market's
    /// (margin removed), `weight` being the market's share. The scoreline
    /// distribution is rescaled within each result, so its shape is kept and
    /// every figure derived from it stays consistent with the blended odds.
    pub fn blend_market(&self, market: [f64; 3], weight: f64) -> Prediction {
        let w = weight.clamp(0.0, 1.0);
        let ours = [self.home_win, self.draw, self.away_win];
        let sum: f64 = market.iter().sum();
        if w == 0.0 || !(sum > 0.0) || ours.iter().any(|p| !(*p > 0.0)) {
            return self.clone();
        }
        let blended: Vec<f64> = (0..3).map(|k| (1.0 - w) * ours[k] + w * market[k] / sum).collect();
        let mut scores = self.scores.clone();
        for (h, row) in scores.iter_mut().enumerate() {
            for (a, cell) in row.iter_mut().enumerate() {
                let k = match h.cmp(&a) {
                    std::cmp::Ordering::Greater => 0,
                    std::cmp::Ordering::Equal => 1,
                    std::cmp::Ordering::Less => 2,
                };
                *cell *= blended[k] / ours[k];
            }
        }
        let mut out = Prediction::from_scores(scores, self.home_xg, self.away_xg);
        out.market_weight = w;
        out
    }
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
        Model::fit_with(history, as_of, cfg, None)
    }

    /// [`Model::fit`], optionally fitting several competitions together
    /// (see [`Pooling`]). Without pooling every match is treated as one
    /// competition and \`Match::comp\` is ignored.
    pub fn fit_with(
        history: &[Match],
        as_of: &str,
        cfg: &ModelConfig,
        pooling: Option<&Pooling>,
    ) -> Model {
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
            /// Competition index.
            c: usize,
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
                c: if pooling.is_some() { m.comp as usize } else { 0 },
            });
        }
        let n = index.len();
        let target = pooling.map_or(0, |p| p.target);
        let n_comps = rows.iter().map(|r| r.c).max().map_or(1, |c| c + 1).max(target + 1);
        // Group offsets are only fitted when pooling; otherwise one dummy group.
        let n_groups = pooling.map_or(1, |p| p.n_groups.max(1));
        let group: Vec<usize> = {
            let mut g = vec![0; n];
            if let Some(p) = pooling {
                for (name, &i) in &index {
                    g[i] = p.groups.get(name).copied().unwrap_or(p.default_group).min(n_groups - 1);
                }
            }
            g
        };
        let mut att = vec![0.0; n];
        let mut def = vec![0.0; n];
        // Group offsets are included in `att` and `def`; these track them.
        let mut off_att = vec![0.0; n_groups];
        let mut off_def = vec![0.0; n_groups];
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
        let inv_var_group = 1.0 / (cfg.league_sd * cfg.league_sd);
        let inv_var_level = 1.0 / (cfg.level_sd * cfg.level_sd);
        let mut mu = vec![0.1f64; n_comps];
        let mut home = vec![0.25f64; n_comps];
        for _sweep in 0..200 {
            let mut change = 0.0f64;
            // Attack.
            let mut g = vec![0.0; n];
            let mut hss = vec![0.0; n];
            for r in &rows {
                let lh = (mu[r.c] + r.home_edge * (home[r.c] + hadv[r.h]) + att[r.h] - def[r.a]).exp();
                let la = (mu[r.c] + att[r.a] - def[r.h]).exp();
                g[r.h] += r.w * (r.gh - lh);
                hss[r.h] += r.w * lh;
                g[r.a] += r.w * (r.ga - la);
                hss[r.a] += r.w * la;
            }
            for i in 0..n {
                let own = att[i] - off_att[group[i]];
                let step = (g[i] - (own - prior_mean[i]) * inv_var) / (hss[i] + inv_var);
                att[i] += step;
                change = change.max(step.abs());
            }
            // Defence.
            let mut g = vec![0.0; n];
            let mut hss = vec![0.0; n];
            for r in &rows {
                let lh = (mu[r.c] + r.home_edge * (home[r.c] + hadv[r.h]) + att[r.h] - def[r.a]).exp();
                let la = (mu[r.c] + att[r.a] - def[r.h]).exp();
                g[r.a] -= r.w * (r.gh - lh);
                hss[r.a] += r.w * lh;
                g[r.h] -= r.w * (r.ga - la);
                hss[r.h] += r.w * la;
            }
            for i in 0..n {
                let own = def[i] - off_def[group[i]];
                let step = (g[i] - (own - prior_mean[i]) * inv_var) / (hss[i] + inv_var);
                def[i] += step;
                change = change.max(step.abs());
            }
            // Group offsets: shift every club of a group together. This is what
            // ties leagues of different strength together through the matches
            // between their clubs.
            if pooling.is_some() {
                let mut ga = vec![0.0; n_groups];
                let mut ha = vec![0.0; n_groups];
                let mut gd = vec![0.0; n_groups];
                let mut hd = vec![0.0; n_groups];
                for r in &rows {
                    let lh = (mu[r.c] + r.home_edge * (home[r.c] + hadv[r.h]) + att[r.h] - def[r.a]).exp();
                    let la = (mu[r.c] + att[r.a] - def[r.h]).exp();
                    let (kh, ka) = (group[r.h], group[r.a]);
                    ga[kh] += r.w * (r.gh - lh);
                    ha[kh] += r.w * lh;
                    gd[kh] -= r.w * (r.ga - la);
                    hd[kh] += r.w * la;
                    ga[ka] += r.w * (r.ga - la);
                    ha[ka] += r.w * la;
                    gd[ka] -= r.w * (r.gh - lh);
                    hd[ka] += r.w * lh;
                }
                let mut shift_att = vec![0.0; n_groups];
                let mut shift_def = vec![0.0; n_groups];
                for k in 0..n_groups {
                    shift_att[k] = (ga[k] - off_att[k] * inv_var_group) / (ha[k] + inv_var_group);
                    shift_def[k] = (gd[k] - off_def[k] * inv_var_group) / (hd[k] + inv_var_group);
                    off_att[k] += shift_att[k];
                    off_def[k] += shift_def[k];
                    change = change.max(shift_att[k].abs()).max(shift_def[k].abs());
                }
                for i in 0..n {
                    att[i] += shift_att[group[i]];
                    def[i] += shift_def[group[i]];
                }
            }
            // Each club's own home advantage.
            if cfg.home_sd > 0.0 {
                let prec = 1.0 / (cfg.home_sd * cfg.home_sd);
                let mut g = vec![0.0; n];
                let mut hss = vec![0.0; n];
                for r in &rows {
                    let lh = (mu[r.c] + r.home_edge * (home[r.c] + hadv[r.h]) + att[r.h] - def[r.a]).exp();
                    g[r.h] += r.w * r.home_edge * (r.gh - lh);
                    hss[r.h] += r.w * r.home_edge * lh;
                }
                for i in 0..n {
                    let step = (g[i] - hadv[i] * prec) / (hss[i] + prec);
                    hadv[i] += step;
                    change = change.max(step.abs());
                }
            }
            // Goal level and home advantage of each competition.
            let mut goals = vec![0.0; n_comps];
            let mut expect = vec![0.0; n_comps];
            let mut hg = vec![0.0; n_comps];
            let mut he = vec![0.0; n_comps];
            for r in &rows {
                let lh = (mu[r.c] + r.home_edge * (home[r.c] + hadv[r.h]) + att[r.h] - def[r.a]).exp();
                let la = (mu[r.c] + att[r.a] - def[r.h]).exp();
                goals[r.c] += r.w * (r.gh + r.ga);
                expect[r.c] += r.w * (lh + la);
                hg[r.c] += r.w * r.home_edge * r.gh;
                he[r.c] += r.w * r.home_edge * lh;
            }
            if pooling.is_none() {
                if expect[0] > 0.0 {
                    let s = (goals[0] / expect[0]).ln();
                    mu[0] += s;
                    change = change.max(s.abs());
                }
                if he[0] > 0.0 {
                    let s = (hg[0] / he[0]).ln() * 0.5;
                    home[0] += s;
                    change = change.max(s.abs());
                }
            } else {
                // Competitions with little data stay close to the average level.
                let total: f64 = expect.iter().sum();
                let total_h: f64 = he.iter().sum();
                let mu_mean = if total > 0.0 {
                    expect.iter().zip(&mu).map(|(e, m)| e * m).sum::<f64>() / total
                } else {
                    0.0
                };
                let home_mean = if total_h > 0.0 {
                    he.iter().zip(&home).map(|(e, m)| e * m).sum::<f64>() / total_h
                } else {
                    0.0
                };
                for c in 0..n_comps {
                    if expect[c] > 0.0 {
                        let s = ((goals[c] - expect[c] - (mu[c] - mu_mean) * inv_var_level)
                            / (expect[c] + inv_var_level))
                            .clamp(-0.5, 0.5);
                        mu[c] += s;
                        change = change.max(s.abs());
                    }
                    if he[c] > 0.0 {
                        let s = ((hg[c] - he[c] - (home[c] - home_mean) * inv_var_level)
                            / (he[c] + inv_var_level))
                            .clamp(-0.5, 0.5)
                            * 0.5;
                        home[c] += s;
                        change = change.max(s.abs());
                    }
                }
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
                    let lh = (mu[r.c] + r.home_edge * (home[r.c] + hadv[r.h]) + att[r.h] - def[r.a]).exp();
                    let la = (mu[r.c] + att[r.a] - def[r.h]).exp();
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
        // A club never seen sits at its group's level, a little below average.
        let newcomer_offset = |g: usize| pooling.map_or((0.0, 0.0), |_| (off_att[g], off_def[g]));
        let default_group = pooling.map_or(0, |p| p.default_group.min(n_groups - 1));
        let (na, nd) = newcomer_offset(default_group);
        let levels: Vec<(f64, f64)> = mu.iter().zip(&home).map(|(&m, &h)| (m, h)).collect();
        Model {
            teams,
            mu: mu[target],
            home: home[target],
            rho: best.1,
            newcomer: Strength {
                attack: na - cfg.newcomer_penalty,
                defence: nd - cfg.newcomer_penalty,
                weight: 0.0,
                home: 0.0,
            },
            spread: cfg.spread,
            levels,
            target,
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

    /// Expected goals for each side, at the target competition's goal level.
    pub fn rates(&self, home: &str, away: &str) -> (f64, f64) {
        self.rates_in(self.target, home, away, false)
    }

    /// Expected goals for a match of competition `comp` (`Match::comp`), at that
    /// competition's goal level and home advantage. `neutral` removes both home
    /// effects. Competitions the model was not fitted on use the target's level.
    pub fn rates_in(&self, comp: usize, home: &str, away: &str, neutral: bool) -> (f64, f64) {
        let (h, a) = (self.strength(home), self.strength(away));
        let (mu, edge) = if comp == self.target {
            (self.mu, self.home)
        } else {
            self.levels.get(comp).copied().unwrap_or((self.mu, self.home))
        };
        let edge = if neutral { 0.0 } else { edge + h.home };
        (
            (mu + edge + self.spread * (h.attack - a.defence)).exp(),
            (mu + self.spread * (a.attack - h.defence)).exp(),
        )
    }

    /// Full prediction for `home` against `away`.
    pub fn predict(&self, home: &str, away: &str) -> Prediction {
        let (lh, la) = self.rates(home, away);
        let mut scores = vec![vec![0.0; MAX_GOALS + 1]; MAX_GOALS + 1];
        for (h, row) in scores.iter_mut().enumerate() {
            for (a, cell) in row.iter_mut().enumerate() {
                let p = poisson(h, lh) * poisson(a, la) * tau(h as u8, a as u8, lh, la, self.rho);
                *cell = p.max(0.0);
            }
        }
        Prediction::from_scores(scores, lh, la)
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
            comp: 0,
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

    #[test]
    fn predicted_score_agrees_with_the_most_likely_result() {
        let mut hist = Vec::new();
        for k in 0..20 {
            let d = crate::data::add_days("2025-08-01", k * 7);
            hist.push(m(&d, "Strong", "Weak", 3, 0));
            hist.push(m(&d, "Weak", "Strong", 0, 2));
            hist.push(m(&d, "Mid", "Weak", 1, 0));
            hist.push(m(&d, "Strong", "Mid", 2, 1));
            hist.push(m(&d, "Mid", "Strong", 1, 1));
        }
        let model = Model::fit(&hist, "2026-01-01", &ModelConfig::default());
        for (a, b) in [("Strong", "Weak"), ("Weak", "Strong"), ("Mid", "Weak"), ("Strong", "Mid"), ("Mid", "Strong")] {
            let p = model.predict(a, b);
            let (h, g) = p.predicted_score;
            let top = if p.home_win >= p.draw && p.home_win >= p.away_win { 0 } else if p.draw >= p.away_win { 1 } else { 2 };
            let shown = match h.cmp(&g) { std::cmp::Ordering::Greater => 0, std::cmp::Ordering::Equal => 1, _ => 2 };
            assert_eq!(top, shown, "{a} v {b}: {:?}", p.predicted_score);
            // No other score with the same result is more likely.
            let best = p.scores[h as usize][g as usize];
            for (x, row) in p.scores.iter().enumerate() {
                for (y, &q) in row.iter().enumerate() {
                    if x.cmp(&y) == h.cmp(&g) {
                        assert!(q <= best + 1e-12);
                    }
                }
            }
        }
    }

    #[test]
    fn pooling_lets_league_games_inform_a_cup() {
        // Strong and Weak only ever meet other clubs in the league (comp 0); the
        // cup (comp 1) has one unrelated match, so alone it knows nothing about them.
        let mut hist = Vec::new();
        for k in 0..20 {
            let d = crate::data::add_days("2025-08-01", k * 7);
            for (h, a, gh, ga) in [("Strong", "Mid", 3, 0), ("Mid", "Strong", 0, 2), ("Mid", "Weak", 2, 0), ("Weak", "Mid", 0, 1)] {
                hist.push(m(&d, h, a, gh, ga));
            }
        }
        let mut cup = m("2025-12-01", "X", "Y", 1, 1);
        cup.comp = 1;
        hist.push(cup);
        let cfg = ModelConfig::default();
        let alone: Vec<Match> = hist.iter().filter(|x| x.comp == 1).cloned().collect();
        let alone = Model::fit(&alone, "2026-01-01", &cfg).predict("Strong", "Weak");
        let pooling = Pooling { groups: BTreeMap::new(), default_group: 0, n_groups: 1, target: 1 };
        let pooled = Model::fit_with(&hist, "2026-01-01", &cfg, Some(&pooling)).predict("Strong", "Weak");
        assert!((alone.home_win - alone.away_win).abs() < 0.05, "{alone:?}");
        assert!(pooled.home_win > 0.7, "{pooled:?}");
    }

    #[test]
    fn blending_the_market_keeps_the_distribution_consistent() {
        let mut hist = Vec::new();
        for k in 0..20 {
            let d = crate::data::add_days("2025-08-01", k * 7);
            hist.push(m(&d, "Strong", "Weak", 3, 0));
            hist.push(m(&d, "Weak", "Strong", 0, 2));
            hist.push(m(&d, "Mid", "Weak", 1, 0));
            hist.push(m(&d, "Strong", "Mid", 2, 1));
        }
        let model = Model::fit(&hist, "2026-01-01", &ModelConfig::default());
        let ours = model.predict("Mid", "Weak");
        let market = [0.2, 0.3, 0.5];
        let half = ours.blend_market(market, 0.5);
        let total: f64 = half.scores.iter().flatten().sum();
        assert!((total - 1.0).abs() < 1e-9);
        assert!((half.home_win + half.draw + half.away_win - 1.0).abs() < 1e-9);
        for (blended, (own, theirs)) in [half.home_win, half.draw, half.away_win]
            .into_iter()
            .zip([ours.home_win, ours.draw, ours.away_win].into_iter().zip(market))
        {
            assert!((blended - 0.5 * (own + theirs)).abs() < 1e-9);
        }
        assert_eq!(half.market_weight, 0.5);
        // Weight 0 changes nothing; weight 1 is the market (a scaled copy of the shape).
        assert_eq!(ours.blend_market(market, 0.0).scores, ours.scores);
        let all = ours.blend_market(market, 1.0);
        assert!((all.away_win - 0.5).abs() < 1e-9 && (all.home_win - 0.2).abs() < 1e-9);
        // The predicted score still agrees with the (blended) most likely result.
        let (h, a) = half.predicted_score;
        let top = if half.home_win >= half.draw && half.home_win >= half.away_win { 0 } else if half.draw >= half.away_win { 1 } else { 2 };
        let shown = match h.cmp(&a) { std::cmp::Ordering::Greater => 0, std::cmp::Ordering::Equal => 1, _ => 2 };
        assert_eq!(top, shown);
    }
}

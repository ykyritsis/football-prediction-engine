//! Generic count model for match statistics (cards, corners, shots).
//!
//! For a statistic counted per side,
//! `E[home] = exp(mu + home + for[h] + against[a] + ref)` and
//! `E[away] = exp(mu + for[a] + against[h] + ref)`: `for` is how much of it a
//! club produces (corners won, cards picked up), `against` how much it draws
//! from opponents, `ref` a referee effect that scales both sides. Fitted by
//! weighted Poisson maximum likelihood with time decay and a ridge prior; a
//! negative-binomial dispersion for the match total is estimated from the
//! residuals, because cards and corners vary more than Poisson.

use crate::data::{day_number, Match};
use crate::stats::Stats;
use std::collections::BTreeMap;

/// Which statistic to model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    /// Yellow cards plus two per red (bookings points / 10).
    Cards,
    Corners,
    ShotsOnTarget,
    Shots,
    Fouls,
}

impl Stat {
    pub fn get(self, s: &Stats) -> (f64, f64) {
        let p = |x: (u16, u16)| (x.0 as f64, x.1 as f64);
        match self {
            Stat::Cards => (
                s.yellows.0 as f64 + 2.0 * s.reds.0 as f64,
                s.yellows.1 as f64 + 2.0 * s.reds.1 as f64,
            ),
            Stat::Corners => p(s.corners),
            Stat::ShotsOnTarget => p(s.shots_on_target),
            Stat::Shots => p(s.shots),
            Stat::Fouls => p(s.fouls),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Stat::Cards => "cards",
            Stat::Corners => "corners",
            Stat::ShotsOnTarget => "shots on target",
            Stat::Shots => "shots",
            Stat::Fouls => "fouls",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CountModel {
    pub stat: Stat,
    pub mu: f64,
    pub home: f64,
    pub produce: BTreeMap<String, f64>,
    pub concede: BTreeMap<String, f64>,
    pub referee: BTreeMap<String, f64>,
    /// Negative-binomial size of the match total (larger = closer to Poisson).
    pub size: f64,
}

/// Expected counts and the distribution of the match total.
#[derive(Debug, Clone)]
pub struct CountPrediction {
    pub home: f64,
    pub away: f64,
    /// `total[k]` = P(match total == k), up to the 99.9th percentile.
    pub total: Vec<f64>,
}

impl CountPrediction {
    pub fn over(&self, line: f64) -> f64 {
        self.total
            .iter()
            .enumerate()
            .filter(|(k, _)| *k as f64 > line)
            .map(|(_, p)| p)
            .sum()
    }
}

/// Negative-binomial pmf with mean `m` and size `r`, k = 0..
fn nb_pmf(m: f64, r: f64, max: usize) -> Vec<f64> {
    let p = r / (r + m);
    let mut out = Vec::with_capacity(max + 1);
    let mut cur = p.powf(r);
    for k in 0..=max {
        out.push(cur);
        cur *= (k as f64 + r) / (k as f64 + 1.0) * (1.0 - p);
    }
    let s: f64 = out.iter().sum();
    out.iter_mut().for_each(|x| *x /= s);
    out
}

impl CountModel {
    pub fn fit(history: &[Match], stat: Stat, as_of: &str, half_life_days: f64) -> CountModel {
        let today = day_number(as_of);
        let decay = std::f64::consts::LN_2 / half_life_days;
        let mut teams: BTreeMap<String, usize> = BTreeMap::new();
        let mut refs: BTreeMap<String, usize> = BTreeMap::new();
        struct Row {
            h: usize,
            a: usize,
            r: Option<usize>,
            yh: f64,
            ya: f64,
            w: f64,
        }
        let mut rows = Vec::new();
        for m in history {
            let Some(s) = &m.stats else { continue };
            if m.date.as_str() >= as_of {
                continue;
            }
            let w = (-decay * (today - day_number(&m.date)) as f64).exp();
            if w < 1e-3 {
                continue;
            }
            let n = teams.len();
            let h = *teams.entry(m.home.clone()).or_insert(n);
            let n = teams.len();
            let a = *teams.entry(m.away.clone()).or_insert(n);
            let r = s.referee.as_ref().map(|name| {
                let n = refs.len();
                *refs.entry(name.clone()).or_insert(n)
            });
            let (yh, ya) = stat.get(s);
            rows.push(Row { h, a, r, yh, ya, w });
        }
        let (nt, nr) = (teams.len(), refs.len());
        let (mut pro, mut con, mut rf) = (vec![0.0; nt], vec![0.0; nt], vec![0.0; nr]);
        // Ridge precision: team effects within ~±0.3, referees ~±0.2.
        let (team_prec, ref_prec) = (1.0 / 0.09, 1.0 / 0.04);
        let mean_total: f64 = rows.iter().map(|r| r.w * (r.yh + r.ya)).sum::<f64>()
            / rows.iter().map(|r| 2.0 * r.w).sum::<f64>().max(1e-9);
        let (mut mu, mut home) = (mean_total.max(0.05).ln(), 0.0f64);
        let rate = |mu: f64, home: f64, pro: &[f64], con: &[f64], rf: &[f64], r: &Row| {
            let e = r.r.map_or(0.0, |k| rf[k]);
            (
                (mu + home + pro[r.h] + con[r.a] + e).exp(),
                (mu + pro[r.a] + con[r.h] + e).exp(),
            )
        };
        for _ in 0..100 {
            let mut change = 0.0f64;
            for which in 0..3 {
                let len = if which == 2 { nr } else { nt };
                let (mut g, mut hs) = (vec![0.0; len], vec![0.0; len]);
                for r in &rows {
                    let (lh, la) = rate(mu, home, &pro, &con, &rf, r);
                    match which {
                        0 => {
                            g[r.h] += r.w * (r.yh - lh);
                            hs[r.h] += r.w * lh;
                            g[r.a] += r.w * (r.ya - la);
                            hs[r.a] += r.w * la;
                        }
                        1 => {
                            g[r.a] += r.w * (r.yh - lh);
                            hs[r.a] += r.w * lh;
                            g[r.h] += r.w * (r.ya - la);
                            hs[r.h] += r.w * la;
                        }
                        _ => {
                            if let Some(k) = r.r {
                                g[k] += r.w * (r.yh + r.ya - lh - la);
                                hs[k] += r.w * (lh + la);
                            }
                        }
                    }
                }
                let (v, prec) = match which {
                    0 => (&mut pro, team_prec),
                    1 => (&mut con, team_prec),
                    _ => (&mut rf, ref_prec),
                };
                for i in 0..len {
                    let step = (g[i] - v[i] * prec) / (hs[i] + prec);
                    v[i] += step;
                    change = change.max(step.abs());
                }
            }
            let (mut y, mut e, mut yh, mut eh) = (0.0, 0.0, 0.0, 0.0);
            for r in &rows {
                let (lh, la) = rate(mu, home, &pro, &con, &rf, r);
                y += r.w * (r.yh + r.ya);
                e += r.w * (lh + la);
                yh += r.w * r.yh;
                eh += r.w * lh;
            }
            if e > 0.0 && y > 0.0 {
                let s = (y / e).ln();
                mu += s;
                change = change.max(s.abs());
            }
            if eh > 0.0 && yh > 0.0 {
                let s = 0.5 * (yh / eh).ln();
                home += s;
                change = change.max(s.abs());
            }
            if change < 1e-6 {
                break;
            }
        }
        // Dispersion of the total by moments: Var = m + m^2 / size.
        let (mut num, mut den) = (0.0, 0.0);
        for r in &rows {
            let (lh, la) = rate(mu, home, &pro, &con, &rf, r);
            let m = lh + la;
            let resid = r.yh + r.ya - m;
            num += r.w * m * m;
            den += r.w * (resid * resid - m);
        }
        let size = if den > 0.0 {
            (num / den).clamp(2.0, 1e4)
        } else {
            1e4
        };
        let names = |idx: &BTreeMap<String, usize>, v: &[f64]| {
            idx.iter().map(|(k, &i)| (k.clone(), v[i])).collect()
        };
        CountModel {
            stat,
            mu,
            home,
            produce: names(&teams, &pro),
            concede: names(&teams, &con),
            referee: names(&refs, &rf),
            size,
        }
    }

    pub fn predict(&self, home: &str, away: &str, referee: Option<&str>) -> CountPrediction {
        let g = |m: &BTreeMap<String, f64>, k: &str| m.get(k).copied().unwrap_or(0.0);
        let e = referee.map_or(0.0, |r| g(&self.referee, r));
        let lh = (self.mu + self.home + g(&self.produce, home) + g(&self.concede, away) + e).exp();
        let la = (self.mu + g(&self.produce, away) + g(&self.concede, home) + e).exp();
        let m = lh + la;
        let max = (m + 6.0 * (m + m * m / self.size).sqrt()).ceil() as usize + 2;
        CountPrediction {
            home: lh,
            away: la,
            total: nb_pmf(m, self.size, max),
        }
    }
}

/// Walk-forward accuracy of a count model on match totals.
#[derive(Debug, Default, Clone)]
pub struct CountReport {
    pub matches: usize,
    /// Mean absolute error of the expected total.
    pub mae: f64,
    /// Same, predicting the running league average.
    pub base_mae: f64,
    /// Mean negative log-likelihood of the observed total.
    pub log_loss: f64,
    /// Same, for a negative binomial at the running league average.
    pub base_log_loss: f64,
    /// Predicted vs observed share of matches over the median line.
    pub over_line: f64,
    pub over_pred: f64,
    pub over_obs: f64,
}

pub fn backtest(all: &[Match], stat: Stat, from: &str, half_life_days: f64) -> CountReport {
    let mut rep = CountReport::default();
    let played: Vec<&Match> = all
        .iter()
        .filter(|m| m.stats.is_some() && m.date.as_str() >= from)
        .collect();
    if played.is_empty() {
        return rep;
    }
    // Median total in the test period sets the over/under line.
    let mut totals: Vec<f64> = played
        .iter()
        .map(|m| {
            let (h, a) = stat.get(m.stats.as_ref().unwrap());
            h + a
        })
        .collect();
    totals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    rep.over_line = totals[totals.len() / 2].floor() + 0.5;
    let (mut base_sum, mut base_n, mut base_sq) = (0.0, 0.0, 0.0);
    for m in all
        .iter()
        .filter(|m| m.stats.is_some() && m.date.as_str() < from)
    {
        let (h, a) = stat.get(m.stats.as_ref().unwrap());
        base_sum += h + a;
        base_sq += (h + a) * (h + a);
        base_n += 1.0;
    }
    let mut i = 0;
    while i < played.len() {
        let start = played[i].date.clone();
        let end = crate::data::add_days(&start, 7);
        let model = CountModel::fit(all, stat, &start, half_life_days);
        while i < played.len() && played[i].date < end {
            let m = played[i];
            let s = m.stats.as_ref().unwrap();
            let (h, a) = stat.get(s);
            let y = h + a;
            let p = model.predict(&m.home, &m.away, s.referee.as_deref());
            let mean = p.home + p.away;
            let idx = (y as usize).min(p.total.len() - 1);
            let n = rep.matches as f64;
            let w = |old: f64, x: f64| (old * n + x) / (n + 1.0);
            rep.mae = w(rep.mae, (mean - y).abs());
            rep.log_loss = w(rep.log_loss, -p.total[idx].max(1e-9).ln());
            rep.over_pred = w(rep.over_pred, p.over(rep.over_line));
            rep.over_obs = w(rep.over_obs, (y > rep.over_line) as u8 as f64);
            // Base: running league mean with its own dispersion.
            let bm = if base_n > 0.0 {
                base_sum / base_n
            } else {
                mean
            };
            let var = if base_n > 1.0 {
                base_sq / base_n - bm * bm
            } else {
                bm
            };
            let size = if var > bm {
                (bm * bm / (var - bm)).clamp(2.0, 1e4)
            } else {
                1e4
            };
            let bp = nb_pmf(bm, size, idx.max(60));
            rep.base_mae = w(rep.base_mae, (bm - y).abs());
            rep.base_log_loss = w(rep.base_log_loss, -bp[idx].max(1e-9).ln());
            base_sum += y;
            base_sq += y * y;
            base_n += 1.0;
            rep.matches += 1;
            i += 1;
        }
    }
    rep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nb_sums_to_one_and_has_the_right_mean() {
        let p = nb_pmf(4.2, 8.0, 60);
        let s: f64 = p.iter().sum();
        let m: f64 = p.iter().enumerate().map(|(k, q)| k as f64 * q).sum();
        assert!((s - 1.0).abs() < 1e-9);
        assert!((m - 4.2).abs() < 1e-3, "{m}");
    }

    #[test]
    fn dirty_team_gets_more_cards() {
        let mut hist = Vec::new();
        for k in 0..30 {
            let d = crate::data::add_days("2025-08-01", k * 7);
            for (h, a, yh, ya) in [
                ("Dirty", "Clean", 4, 1),
                ("Clean", "Dirty", 1, 4),
                ("Mid", "Clean", 2, 1),
            ] {
                hist.push(Match {
                    date: d.clone(),
                    home: h.into(),
                    away: a.into(),
                    score: Some((1, 1)),
                    neutral: false,
                    stats: Some(Stats {
                        yellows: (yh, ya),
                        ..Stats::default()
                    }),
                    comp: 0,
                });
            }
        }
        let m = CountModel::fit(&hist, Stat::Cards, "2026-06-01", 365.0);
        let p = m.predict("Dirty", "Mid", None);
        assert!(p.home > p.away + 1.0, "{p:?}");
    }
}

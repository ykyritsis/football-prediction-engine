//! # predictor
//!
//! Match prediction for the top five European leagues, fitted to real
//! results (openfootball data in `data/results/`, fetched with
//! `tools/fetch_results.py`).
//!
//! The model is Dixon–Coles (1997): each club has an attack and a defence
//! strength, the home side gets an advantage, goals are Poisson, and a
//! correction term fixes the under-prediction of low-scoring draws. Recent
//! matches weigh more (exponential decay), and a weak prior pulls clubs with
//! little data, such as promoted sides, towards slightly below average.
//!
//! Everything is a probability: a match prediction is a full scoreline
//! distribution, and a season projection is a Monte Carlo over the
//! remaining fixtures. [`backtest`] scores the model walk-forward, using only
//! matches played before each prediction, so its accuracy can be trusted.

pub mod counts;
pub mod data;
pub mod model;
pub mod season;
pub mod stats;

pub use counts::{CountModel, Stat};
pub use data::{load_league, Match};
pub use model::{Model, ModelConfig, Prediction};

/// Walk-forward evaluation over `matches` played on or after `from` (ISO
/// date). The model is refitted before each matchday on everything earlier.
pub fn backtest(all: &[Match], from: &str, cfg: &ModelConfig) -> BacktestReport {
    let mut report = BacktestReport::default();
    let mut dates: Vec<&str> = all
        .iter()
        .filter(|m| m.score.is_some() && m.date.as_str() >= from)
        .map(|m| m.date.as_str())
        .collect();
    dates.sort_unstable();
    dates.dedup();
    // Refit weekly: group dates into 7-day windows.
    let mut i = 0;
    while i < dates.len() {
        let start = dates[i];
        let end = data::add_days(start, 7);
        let history: Vec<Match> = all
            .iter()
            .filter(|m| m.score.is_some() && m.date.as_str() < start)
            .cloned()
            .collect();
        let model = Model::fit(&history, start, cfg);
        for m in all
            .iter()
            .filter(|m| m.date.as_str() >= start && m.date.as_str() < end.as_str())
        {
            let Some((h, a)) = m.score else { continue };
            let p = model.predict(&m.home, &m.away);
            report.add(&p, h, a);
        }
        while i < dates.len() && dates[i] < end.as_str() {
            i += 1;
        }
    }
    report
}

/// Accuracy of probabilistic predictions.
#[derive(Debug, Default, Clone)]
pub struct BacktestReport {
    pub matches: usize,
    /// Mean negative log-likelihood of the actual 1X2 outcome.
    pub log_loss: f64,
    /// Mean Brier score over home/draw/away.
    pub brier: f64,
    /// Share of matches where the most likely outcome happened.
    pub hit_rate: f64,
    /// Share of exact scores predicted as the single most likely scoreline.
    pub exact_score: f64,
    /// Calibration bins over predicted probabilities (10 bins of 10%):
    /// (sum of predicted p, number of events that happened, count).
    pub calibration: [(f64, f64, f64); 10],
    /// Same metrics for the base-rate predictor (league-wide 1X2 shares).
    pub base_log_loss: f64,
    base_counts: [f64; 3],
}

impl BacktestReport {
    fn add(&mut self, p: &Prediction, h: u8, a: u8) {
        let outcome = match h.cmp(&a) {
            std::cmp::Ordering::Greater => 0,
            std::cmp::Ordering::Equal => 1,
            std::cmp::Ordering::Less => 2,
        };
        let probs = [p.home_win, p.draw, p.away_win];
        let n = self.matches as f64;
        let w = |old: f64, x: f64| (old * n + x) / (n + 1.0);
        self.log_loss = w(self.log_loss, -probs[outcome].max(1e-9).ln());
        let brier: f64 = (0..3)
            .map(|k| (probs[k] - if k == outcome { 1.0 } else { 0.0 }).powi(2))
            .sum();
        self.brier = w(self.brier, brier);
        let best = (0..3)
            .max_by(|&x, &y| probs[x].partial_cmp(&probs[y]).unwrap())
            .unwrap();
        self.hit_rate = w(self.hit_rate, (best == outcome) as u8 as f64);
        self.exact_score = w(
            self.exact_score,
            (p.most_likely_score == (h, a)) as u8 as f64,
        );
        for (k, &pk) in probs.iter().enumerate() {
            let bin = ((pk * 10.0) as usize).min(9);
            self.calibration[bin].0 += pk;
            self.calibration[bin].1 += (k == outcome) as u8 as f64;
            self.calibration[bin].2 += 1.0;
        }
        // Base rate: running league-wide shares, predicted before this match.
        let total: f64 = self.base_counts.iter().sum::<f64>() + 3.0;
        let base_p = (self.base_counts[outcome] + 1.0) / total;
        self.base_log_loss = w(self.base_log_loss, -base_p.ln());
        self.base_counts[outcome] += 1.0;
        self.matches += 1;
    }
}

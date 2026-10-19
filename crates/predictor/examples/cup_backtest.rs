//! Walk-forward check of the club-cup model: does fitting a cup together with the
//! domestic leagues beat fitting it on its own matches alone?
//!
//! Every week the models are refitted on matches before that week and used to
//! predict the cup matches played in it. Run from the repository root:
//!
//! ```text
//! cargo run --release -p predictor --example cup_backtest -- 2025-09-01 2026-10-19
//! ```
use predictor::{
    dashboard::{is_club_cup, ClubPool, LEAGUES},
    data::{add_days, canonical_name},
    load_league, stats, Match, Model, ModelConfig, Prediction,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Default)]
struct Tally {
    n: f64,
    log_loss: f64,
    brier: f64,
    score_log_loss: f64,
    outcome_hit: f64,
    unseen: f64,
}

impl Tally {
    fn add(&mut self, p: &Prediction, goals: (u8, u8), unseen: bool) {
        let outcome = match goals.0.cmp(&goals.1) {
            std::cmp::Ordering::Greater => 0,
            std::cmp::Ordering::Equal => 1,
            std::cmp::Ordering::Less => 2,
        };
        let probs = [p.home_win, p.draw, p.away_win];
        let top = (0..3).max_by(|&a, &b| probs[a].partial_cmp(&probs[b]).unwrap()).unwrap();
        self.n += 1.0;
        self.log_loss -= probs[outcome].max(1e-9).ln();
        self.brier += (0..3)
            .map(|k| (probs[k] - if k == outcome { 1.0 } else { 0.0 }).powi(2))
            .sum::<f64>();
        let (h, a) = (goals.0.min(10) as usize, goals.1.min(10) as usize);
        self.score_log_loss -= p.scores[h][a].max(1e-9).ln();
        self.outcome_hit += (top == outcome) as u8 as f64;
        self.unseen += unseen as u8 as f64;
    }

    fn print(&self, label: &str) {
        let n = self.n.max(1.0);
        println!(
            "{label:<14} matches {:>4}  log loss {:.4}  brier {:.4}  score log loss {:.4}  result hit {:.3}  unseen club {:.3}",
            self.n as u64,
            self.log_loss / n,
            self.brier / n,
            self.score_log_loss / n,
            self.outcome_hit / n,
            self.unseen / n,
        );
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let from = args.first().map(String::as_str).unwrap_or("2025-09-01");
    let to = args.get(1).map(String::as_str).unwrap_or("2026-10-19");
    let dir = std::env::var_os("PREDICTOR_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data/results"));
    let mut loaded = BTreeMap::new();
    for (key, _) in LEAGUES.iter().filter(|(key, _)| !predictor::dashboard::is_international(key)) {
        let mut results = load_league(&dir, key)?;
        stats::attach(&dir.parent().unwrap_or(&dir).join("stats"), key, &mut results);
        loaded.insert(key.to_string(), results);
    }
    let pool = ClubPool::new(&loaded);
    let mut cfg = ModelConfig::default();
    // Optional overrides for trying settings: LEAGUE_SD, LEVEL_SD, SPREAD.
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok());
    cfg.league_sd = env("LEAGUE_SD").unwrap_or(cfg.league_sd);
    cfg.level_sd = env("LEVEL_SD").unwrap_or(cfg.level_sd);
    cfg.spread = env("SPREAD").unwrap_or(cfg.spread);

    // Cup matches to predict, with canonical club names and their competition.
    let cups: Vec<&str> = LEAGUES.iter().map(|(k, _)| *k).filter(|k| is_club_cup(k)).collect();
    let mut tests: Vec<(String, Match)> = Vec::new();
    for key in &cups {
        for m in &loaded[*key] {
            if m.score.is_some() && m.date.as_str() >= from && m.date.as_str() <= to {
                let mut m = m.clone();
                m.home = canonical_name(&m.home).to_string();
                m.away = canonical_name(&m.away).to_string();
                tests.push((key.to_string(), m));
            }
        }
    }
    let mut dates: Vec<String> = tests.iter().map(|(_, m)| m.date.clone()).collect();
    dates.sort();
    dates.dedup();

    let (mut own, mut pooled) = (Tally::default(), Tally::default());
    let mut i = 0;
    while i < dates.len() {
        let start = dates[i].clone();
        let end = add_days(&start, 7);
        let week: Vec<_> = tests
            .iter()
            .filter(|(_, m)| m.date.as_str() >= start.as_str() && m.date.as_str() < end.as_str())
            .collect();
        for key in &cups {
            let games: Vec<_> = week.iter().filter(|(k, _)| k == key).collect();
            if games.is_empty() {
                continue;
            }
            let cup_only: Vec<Match> = pool
                .matches
                .iter()
                .filter(|m| m.date < start && m.comp as usize == pool_index(&pool, key))
                .cloned()
                .collect();
            let alone = Model::fit(&cup_only, &start, &cfg);
            let together = Model::fit_with(
                &pool.matches,
                &start,
                &cfg,
                pool.pooling(key).as_ref(),
            );
            for (_, m) in games {
                let goals = m.score.unwrap();
                let (pa, pp) = if m.neutral {
                    (alone.predict_neutral(&m.home, &m.away), together.predict_neutral(&m.home, &m.away))
                } else {
                    (alone.predict(&m.home, &m.away), together.predict(&m.home, &m.away))
                };
                own.add(&pa, goals, !alone.teams.contains_key(&m.home) || !alone.teams.contains_key(&m.away));
                pooled.add(&pp, goals, !together.teams.contains_key(&m.home) || !together.teams.contains_key(&m.away));
            }
        }
        while i < dates.len() && dates[i].as_str() < end.as_str() {
            i += 1;
        }
    }
    println!("Club cup matches {from} to {to}, predicted from earlier results only");
    own.print("cup games only");
    pooled.print("pooled");
    Ok(())
}

/// Competition index of \`key\` inside the pool (cups come after the domestic leagues).
fn pool_index(pool: &ClubPool, key: &str) -> usize {
    pool.pooling(key).map(|p| p.target).unwrap_or(0)
}

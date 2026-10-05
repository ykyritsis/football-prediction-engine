//! Command-line front end.
//!
//! ```text
//! predict backtest [LEAGUE|all] [FROM]      walk-forward accuracy (default from 2024-08-01)
//! predict tune     [LEAGUE|all] [FROM]      backtest several decay half-lives
//! predict fixtures [LEAGUE|all] [DAYS]      next matches (default 10 days from the next fixture)
//! predict table    [LEAGUE|all] [RUNS]      season projection (default 20000 runs)
//! ```
//!
//! "Today" is the day after the last played match in the data, so results
//! are reproducible from the committed files. Data lives in `data/results`
//! (override with `PREDICTOR_DATA`).

use predictor::data::add_days;
use predictor::{backtest, load_league, season, Match, Model, ModelConfig};
use std::path::PathBuf;

const LEAGUES: [(&str, &str); 5] = [
    ("premier_league", "Premier League"),
    ("la_liga", "La Liga"),
    ("serie_a", "Serie A"),
    ("bundesliga", "Bundesliga"),
    ("ligue_1", "Ligue 1"),
];

fn data_dir() -> PathBuf {
    std::env::var_os("PREDICTOR_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data/results"))
}

fn leagues(arg: Option<&str>) -> Vec<(&'static str, &'static str)> {
    match arg {
        None | Some("all") => LEAGUES.to_vec(),
        Some(k) => {
            let found: Vec<_> = LEAGUES
                .iter()
                .copied()
                .filter(|(key, name)| *key == k || name.eq_ignore_ascii_case(k))
                .collect();
            if found.is_empty() {
                eprintln!(
                    "unknown league {k}; use one of: all {}",
                    LEAGUES.map(|l| l.0).join(" ")
                );
                std::process::exit(2);
            }
            found
        }
    }
}

fn load(key: &str) -> Vec<Match> {
    load_league(&data_dir(), key).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    })
}

/// The day after the latest played match.
fn today(all: &[Match]) -> String {
    let last = all
        .iter()
        .filter(|m| m.score.is_some())
        .map(|m| m.date.as_str())
        .max()
        .unwrap_or("2026-08-01");
    add_days(last, 1)
}

/// Fixtures of the season containing the latest played match: matches
/// between the start of that season (first match on or after 1 July) and
/// a year later.
fn current_season(all: &[Match], today: &str) -> Vec<Match> {
    let year: i32 = today[0..4].parse().unwrap();
    let start_year = if &today[5..7] >= "07" { year } else { year - 1 };
    let start = format!("{start_year}-07-01");
    let end = format!("{}-07-01", start_year + 1);
    all.iter()
        .filter(|m| m.date >= start && m.date < end)
        .cloned()
        .collect()
}

fn pct(x: f64) -> String {
    format!("{:>3.0}%", x * 100.0)
}

fn cmd_backtest(args: &[String]) {
    let from = args.get(1).map(String::as_str).unwrap_or("2024-08-01");
    let cfg = ModelConfig::default();
    println!("Walk-forward backtest from {from} (refit weekly, only past data used)\n");
    let mut all_reports = Vec::new();
    for (key, name) in leagues(args.first().map(String::as_str)) {
        let data = load(key);
        all_reports.push(backtest(&data, from, &cfg));
        let r = all_reports.last().unwrap();
        println!(
            "{name:<15} {:>5} matches  log-loss {:.4} (base {:.4}, {:+.1}%)  Brier {:.4}  hit {:.1}%  exact {:.1}%",
            r.matches,
            r.log_loss,
            r.base_log_loss,
            (r.base_log_loss - r.log_loss) / r.base_log_loss * 100.0,
            r.brier,
            r.hit_rate * 100.0,
            r.exact_score * 100.0
        );
    }
    // Pooled calibration.
    let mut bins = [(0.0, 0.0, 0.0); 10];
    for r in &all_reports {
        for (b, c) in bins.iter_mut().zip(r.calibration.iter()) {
            b.0 += c.0;
            b.1 += c.1;
            b.2 += c.2;
        }
    }
    println!("\nCalibration (all outcomes pooled): predicted vs observed");
    for (i, (p, hit, n)) in bins.iter().enumerate() {
        if *n >= 1.0 {
            println!(
                "  {:>3}-{:<3}%  n={:<5}  predicted {:>5.1}%  observed {:>5.1}%",
                i * 10,
                i * 10 + 10,
                *n as usize,
                p / n * 100.0,
                hit / n * 100.0
            );
        }
    }
}

fn cmd_tune(args: &[String]) {
    let from = args.get(1).map(String::as_str).unwrap_or("2024-08-01");
    let sets: Vec<_> = leagues(args.first().map(String::as_str))
        .into_iter()
        .map(|(k, n)| (n, load(k)))
        .collect();
    for half_life in [120.0, 180.0, 240.0, 365.0, 540.0] {
        for prior_sd in [0.25, 0.35, 0.5] {
            let cfg = ModelConfig {
                half_life_days: half_life,
                prior_sd,
                ..ModelConfig::default()
            };
            let (mut n, mut ll, mut hit) = (0usize, 0.0, 0.0);
            for (_, data) in &sets {
                let r = backtest(data, from, &cfg);
                n += r.matches;
                ll += r.log_loss * r.matches as f64;
                hit += r.hit_rate * r.matches as f64;
            }
            println!(
                "half-life {half_life:>4} d  prior sd {prior_sd:.2}  log-loss {:.4}  hit {:.1}%",
                ll / n as f64,
                hit / n as f64 * 100.0
            );
        }
    }
}

fn cmd_fixtures(args: &[String]) {
    let days: i64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
    for (key, name) in leagues(args.first().map(String::as_str)) {
        let all = load(key);
        let today = today(&all);
        // Start at the next scheduled match (international breaks).
        let start = all
            .iter()
            .filter(|m| m.score.is_none() && m.date >= today)
            .map(|m| m.date.clone())
            .min()
            .unwrap_or_else(|| today.clone());
        let end = add_days(&start, days);
        let model = Model::fit(&all, &today, &ModelConfig::default());
        let upcoming: Vec<_> = all
            .iter()
            .filter(|m| m.score.is_none() && m.date >= start && m.date < end)
            .collect();
        println!("\n{name} — fixtures {start} to {end} (fitted to results before {today})");
        println!(
            "  {:<10} {:<26} {:<26}  {:>4} {:>4} {:>4}   xG          likely  O2.5  BTTS",
            "date", "home", "away", "1", "X", "2"
        );
        for m in upcoming {
            let p = model.predict(&m.home, &m.away);
            println!(
                "  {:<10} {:<26} {:<26}  {} {} {}   {:.2}-{:.2}   {}-{}    {}  {}",
                m.date,
                m.home,
                m.away,
                pct(p.home_win),
                pct(p.draw),
                pct(p.away_win),
                p.home_xg,
                p.away_xg,
                p.most_likely_score.0,
                p.most_likely_score.1,
                pct(p.over_2_5),
                pct(p.both_score)
            );
        }
    }
}

fn cmd_table(args: &[String]) {
    let runs: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20_000);
    for (key, name) in leagues(args.first().map(String::as_str)) {
        let all = load(key);
        let today = today(&all);
        let model = Model::fit(&all, &today, &ModelConfig::default());
        let fixtures = current_season(&all, &today);
        let proj = season::project(&fixtures, &model, runs, 0x5eed);
        println!("\n{name} — projection as of {today} ({runs} simulated seasons)");
        println!(
            "  {:<3} {:<26} {:>3} {:>4}  {:>6}  {:>5}  {:>5}  {:>5}  attack defence",
            "", "club", "P", "Pts", "xPts", "title", "top4", "bot3"
        );
        for (i, p) in proj.iter().enumerate() {
            let s = model.strength(&p.team);
            println!(
                "  {:<3} {:<26} {:>3} {:>4}  {:>6.1}  {}  {}  {}  {:+.2}  {:+.2}",
                i + 1,
                p.team,
                p.played,
                p.points_now,
                p.mean_points,
                pct(p.title),
                pct(p.top4),
                pct(p.bottom3),
                s.attack,
                s.defence
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]).to_vec();
    match args.first().map(String::as_str) {
        Some("backtest") => cmd_backtest(&rest),
        Some("tune") => cmd_tune(&rest),
        Some("fixtures") => cmd_fixtures(&rest),
        Some("table") => cmd_table(&rest),
        _ => {
            eprintln!("usage: predict <backtest|tune|fixtures|table> [LEAGUE|all] [ARG]");
            std::process::exit(2);
        }
    }
}

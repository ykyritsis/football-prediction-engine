//! Command-line front end.
//!
//! ```text
//! predict backtest [LEAGUE|all] [FROM]      walk-forward accuracy (default from 2024-08-01)
//! predict tune     [LEAGUE|all] [FROM]      backtest several decay half-lives
//! predict fixtures [LEAGUE|all] [DAYS]      next matches (default 10 days from the next fixture)
//! predict table    [LEAGUE|all] [RUNS]      season projection (default 20000 runs)
//! predict stats    [LEAGUE|all] [FROM]      backtest cards, corners, shots on target, fouls
//! predict match    LEAGUE HOME AWAY [REF]   full report for one match (names may be partial)
//! ```
//!
//! "Today" is the day after the last played match in the data, so results
//! are reproducible from the committed files. Data lives in `data/results`
//! (override with `PREDICTOR_DATA`).

use predictor::counts::{self, CountModel, Stat};
use predictor::data::add_days;
use predictor::{backtest, load_league, season, stats, Match, Model, ModelConfig};
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

/// Results plus match statistics (from `data/stats`, beside the results).
fn load(key: &str) -> Vec<Match> {
    let dir = data_dir();
    let mut all = load_league(&dir, key).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let stats_dir = dir.parent().map(|p| p.join("stats")).unwrap_or_default();
    stats::attach(&stats_dir, key, &mut all);
    all
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
    // Vary one setting at a time around the defaults.
    let base = ModelConfig::default();
    let mut grid = vec![base];
    for spread in [1.1, 1.2, 1.3] {
        grid.push(ModelConfig { spread, ..base });
    }
    for cfg in grid {
        let (mut n, mut ll, mut hit) = (0usize, 0.0, 0.0);
        for (_, data) in &sets {
            let r = backtest(data, from, &cfg);
            n += r.matches;
            ll += r.log_loss * r.matches as f64;
            hit += r.hit_rate * r.matches as f64;
        }
        println!(
            "half-life {:>4} d  prior sd {:.2}  shot weight {:.1}  home sd {:.2}  spread {:.2}  log-loss {:.4}  hit {:.1}%",
            cfg.half_life_days,
            cfg.prior_sd,
            cfg.shot_weight,
            cfg.home_sd,
            cfg.spread,
            ll / n as f64,
            hit / n as f64 * 100.0
        );
    }
}

const STATS: [Stat; 4] = [Stat::Cards, Stat::Corners, Stat::ShotsOnTarget, Stat::Fouls];
const STAT_HALF_LIFE: f64 = 365.0;

fn cmd_stats(args: &[String]) {
    let from = args.get(1).map(String::as_str).unwrap_or("2024-08-01");
    println!("Walk-forward backtest of match totals from {from} (cards = yellows + 2 per red)\n");
    for (key, name) in leagues(args.first().map(String::as_str)) {
        let data = load(key);
        println!(
            "{name} ({} matches with statistics)",
            data.iter().filter(|m| m.stats.is_some()).count()
        );
        for stat in STATS {
            let r = counts::backtest(&data, stat, from, STAT_HALF_LIFE);
            println!(
                "  {:<16} {:>4} matches  MAE {:.2} (base {:.2})  log-loss {:.3} (base {:.3})  over {:.1}: predicted {:.0}%, observed {:.0}%",
                stat.name(),
                r.matches,
                r.mae,
                r.base_mae,
                r.log_loss,
                r.base_log_loss,
                r.over_line,
                r.over_pred * 100.0,
                r.over_obs * 100.0
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
        let cards = CountModel::fit(&all, Stat::Cards, &today, STAT_HALF_LIFE);
        let corners = CountModel::fit(&all, Stat::Corners, &today, STAT_HALF_LIFE);
        let upcoming: Vec<_> = all
            .iter()
            .filter(|m| m.score.is_none() && m.date >= start && m.date < end)
            .collect();
        println!("\n{name} — fixtures {start} to {end} (fitted to results before {today})");
        println!(
            "  {:<10} {:<26} {:<26}  {:>4} {:>4} {:>4}   xG          likely  O2.5  BTTS   cards  O4.5  corners  O9.5",
            "date", "home", "away", "1", "X", "2"
        );
        for m in upcoming {
            let p = model.predict(&m.home, &m.away);
            let c = cards.predict(&m.home, &m.away, None);
            let k = corners.predict(&m.home, &m.away, None);
            println!(
                "  {:<10} {:<26} {:<26}  {} {} {}   {:.2}-{:.2}   {}-{}    {}  {}   {:.1}-{:.1}  {}  {:.1}-{:.1}  {}",
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
                pct(p.both_score),
                c.home,
                c.away,
                pct(c.over(4.5)),
                k.home,
                k.away,
                pct(k.over(9.5))
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

/// The club in `all` whose name contains `q` (case-insensitive), preferring
/// clubs from the latest season.
fn find_team(all: &[Match], q: &str) -> String {
    let q = q.to_lowercase();
    let mut seen: Vec<&str> = Vec::new();
    for m in all.iter().rev() {
        for t in [&m.home, &m.away] {
            if !seen.contains(&t.as_str()) {
                seen.push(t);
            }
        }
    }
    seen.iter()
        .find(|t| t.to_lowercase() == q)
        .or_else(|| seen.iter().find(|t| t.to_lowercase().contains(&q)))
        .map(|t| t.to_string())
        .unwrap_or_else(|| {
            eprintln!("no club matching {q}");
            std::process::exit(2)
        })
}

fn form(all: &[Match], team: &str, n: usize) -> String {
    let mut out: Vec<String> = all
        .iter()
        .rev()
        .filter(|m| m.score.is_some() && (m.home == team || m.away == team))
        .take(n)
        .map(|m| {
            let (h, a) = m.score.unwrap();
            let (gf, ga, opp, venue) = if m.home == team {
                (h, a, &m.away, "H")
            } else {
                (a, h, &m.home, "A")
            };
            let r = match gf.cmp(&ga) {
                std::cmp::Ordering::Greater => "W",
                std::cmp::Ordering::Equal => "D",
                std::cmp::Ordering::Less => "L",
            };
            format!("{r} {gf}-{ga} {venue} {opp}")
        })
        .collect();
    out.reverse();
    out.join(", ")
}

fn cmd_match(args: &[String]) {
    if args.len() < 3 {
        eprintln!("usage: predict match LEAGUE HOME AWAY [REFEREE]");
        std::process::exit(2);
    }
    let (key, name) = leagues(Some(&args[0]))[0];
    let all = load(key);
    let (home, away) = (find_team(&all, &args[1]), find_team(&all, &args[2]));
    let referee = args.get(3).map(String::as_str);
    let today = today(&all);
    let model = Model::fit(&all, &today, &ModelConfig::default());
    let p = model.predict(&home, &away);
    println!("{name}: {home} v {away}  (fitted to results before {today})\n");
    for t in [&home, &away] {
        let s = model.strength(t);
        println!(
            "  {t:<28} attack {:+.2}  defence {:+.2}  own home edge {:+.2}  data weight {:.0}",
            s.attack, s.defence, s.home, s.weight
        );
        println!("  {:<28} last 6: {}", "", form(&all, t, 6));
    }
    println!(
        "\n  Result     home {}  draw {}  away {}",
        pct(p.home_win),
        pct(p.draw),
        pct(p.away_win)
    );
    println!("  Expected goals  {:.2} - {:.2}", p.home_xg, p.away_xg);
    let mut cells: Vec<(f64, usize, usize)> = p
        .scores
        .iter()
        .enumerate()
        .flat_map(|(h, row)| row.iter().enumerate().map(move |(a, &q)| (q, h, a)))
        .collect();
    cells.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let top: Vec<String> = cells
        .iter()
        .take(8)
        .map(|(q, h, a)| format!("{h}-{a} {:.1}%", q * 100.0))
        .collect();
    println!("  Scorelines  {}", top.join("  "));
    let goals_over = |line: f64| -> f64 {
        cells
            .iter()
            .filter(|(_, h, a)| (h + a) as f64 > line)
            .map(|c| c.0)
            .sum()
    };
    println!(
        "  Goals      over 1.5 {}  over 2.5 {}  over 3.5 {}  both score {}",
        pct(goals_over(1.5)),
        pct(goals_over(2.5)),
        pct(goals_over(3.5)),
        pct(p.both_score)
    );
    for stat in STATS {
        let m = CountModel::fit(&all, stat, &today, STAT_HALF_LIFE);
        let c = m.predict(&home, &away, referee);
        let mean = c.home + c.away;
        let line = mean.round() - 0.5;
        println!(
            "  {:<16} {:.1} - {:.1} (total {:.1})  over {:.1} {}  over {:.1} {}  over {:.1} {}",
            stat.name(),
            c.home,
            c.away,
            mean,
            line - 1.0,
            pct(c.over(line - 1.0)),
            line,
            pct(c.over(line)),
            line + 1.0,
            pct(c.over(line + 1.0))
        );
        if stat == Stat::Cards {
            if let Some(r) = referee {
                match m.referee.get(r) {
                    Some(e) => println!(
                        "  {:<16} referee {r}: {:+.0}% cards vs average",
                        "",
                        (e.exp() - 1.0) * 100.0
                    ),
                    None => println!(
                        "  {:<16} referee {r} not in the data (Premier League only)",
                        ""
                    ),
                }
            }
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
        Some("stats") => cmd_stats(&rest),
        Some("match") => cmd_match(&rest),
        _ => {
            eprintln!(
                "usage: predict <backtest|tune|fixtures|table|stats|match> [LEAGUE|all] [ARG]"
            );
            std::process::exit(2);
        }
    }
}

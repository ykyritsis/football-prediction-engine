//! Browser-facing forecasts. Historical rows are reconstructed, not archived forecasts.
use crate::{
    data::{add_days, canonical_name},
    model::Pooling,
    Match, Model, ModelConfig, Prediction,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Serialize)]
pub struct FormAdjustment {
    pub attack: f64,
    pub defence: f64,
}

/// Conservative residual adjustments against the base model, using only earlier
/// results. Pseudo-goals shrink short runs; caps prevent five games dominating.
/// With `pooling`, a club's last five games count whichever competition they
/// were in, and the scoring level is calibrated on the target competition only.
fn current_model(
    history: &[Match],
    before: &str,
    cfg: &ModelConfig,
    pooling: Option<&Pooling>,
) -> (Model, BTreeMap<String, FormAdjustment>, f64, usize) {
    let mut model = Model::fit_with(history, before, cfg, pooling);
    let base = model.clone();
    let target = pooling.map_or(0, |p| p.target);
    let comp_of = |m: &Match| if pooling.is_some() { m.comp as usize } else { 0 };
    // Each club's played matches, in history order.
    let mut by_team: BTreeMap<&str, Vec<&Match>> = BTreeMap::new();
    for m in history
        .iter()
        .filter(|m| m.date.as_str() < before && m.score.is_some())
    {
        by_team.entry(m.home.as_str()).or_default().push(m);
        by_team.entry(m.away.as_str()).or_default().push(m);
    }
    let mut adjustments = BTreeMap::new();
    for name in base.teams.keys() {
        let mut games = by_team.get(name.as_str()).cloned().unwrap_or_default();
        games.sort_by(|a, b| b.date.cmp(&a.date));
        games.truncate(5);
        let mut totals = [0.0; 4];
        for m in &games {
            let (xh, xa) = base.rates_in(comp_of(m), &m.home, &m.away, m.neutral);
            let (h, a) = m.score.unwrap();
            let r = if m.home == *name {
                [h as f64, a as f64, xh, xa]
            } else {
                [a as f64, h as f64, xa, xh]
            };
            for i in 0..4 {
                totals[i] += r[i];
            }
        }
        let adjustment = if games.len() >= 3 {
            FormAdjustment {
                attack: ((totals[0] + 6.0) / (totals[2] + 6.0))
                    .powf(0.35)
                    .clamp(0.85, 1.15),
                defence: ((totals[3] + 6.0) / (totals[1] + 6.0))
                    .powf(0.35)
                    .clamp(0.85, 1.15),
            }
        } else {
            FormAdjustment {
                attack: 1.0,
                defence: 1.0,
            }
        };
        model.adjust(name, adjustment.attack.ln(), adjustment.defence.ln());
        adjustments.insert(name.clone(), adjustment);
    }
    // Reconcile the recent competition goal environment after form adjustments.
    // Forty pseudo-matches shrink noisy periods; no later outcomes are read.
    let start = add_days(before, -90);
    let mut actual = 0.0;
    let mut expected = 0.0;
    let mut count = 0;
    for m in history.iter().filter(|m| {
        m.date >= start
            && m.date.as_str() < before
            && m.score.is_some()
            && comp_of(m) == target
    }) {
        let (xh, xa) = model.rates_in(target, &m.home, &m.away, m.neutral);
        let (h, a) = m.score.unwrap();
        actual += h as f64 + a as f64;
        expected += xh + xa;
        count += 1;
    }
    let factor = if count >= 20 && expected > 0.0 {
        let prior = 40.0 * expected / count as f64;
        ((actual + prior) / (expected + prior)).clamp(0.90, 1.15)
    } else {
        1.0
    };
    model.mu += factor.ln();
    (model, adjustments, factor, count)
}

pub const LEAGUES: [(&str, &str); 22] = [
    ("premier_league", "Premier League"),
    ("la_liga", "La Liga"),
    ("serie_a", "Serie A"),
    ("bundesliga", "Bundesliga"),
    ("ligue_1", "Ligue 1"),
    ("primeira_liga", "Primeira Liga"),
    ("eredivisie", "Eredivisie"),
    ("belgian_pro_league", "Belgian Pro League"),
    ("super_lig", "Süper Lig"),
    ("chance_liga", "Chance Liga"),
    ("champions_league", "Champions League"),
    ("europa_league", "Europa League"),
    ("conference_league", "Conference League"),
    ("club_world_cup", "Club World Cup"),
    ("world_cup", "World Cup"),
    ("european_championship", "European Championship"),
    ("nations_league", "UEFA Nations League"),
    ("world_cup_qualifying", "European World Cup Qualifiers"),
    ("copa_america", "Copa América"),
    ("africa_cup", "Africa Cup of Nations"),
    ("gold_cup", "Gold Cup"),
    ("concacaf_nations", "Concacaf Nations League"),
];

/// Bookmaker probabilities (home, draw, away; margin removed) by `market_key`.
pub type Market = BTreeMap<String, [f64; 3]>;

/// Share of the bookmaker market in the result probabilities where it is known.
/// Walk-forward on 7,082 big-five matches, 50% lifted the hit rate from 52.6% to
/// 53.4% and log loss from 0.988 to 0.975 (the market alone: 54.0%, 0.969).
pub const MARKET_WEIGHT: f64 = 0.5;

/// Key of a league match in the odds file: season, home and away club. A pairing
/// is played once a season, so a rescheduled date does not matter.
pub fn market_key(date: &str, home: &str, away: &str) -> String {
    let year: u32 = date[..4].parse().unwrap_or(0);
    let year = if &date[5..7] < "07" { year.saturating_sub(1) } else { year };
    format!("{year}-{:02}|{home}|{away}", (year + 1) % 100)
}

/// Club cups, fitted together with the domestic leagues.
pub fn is_club_cup(key: &str) -> bool {
    matches!(
        key,
        "champions_league" | "europa_league" | "conference_league" | "club_world_cup"
    )
}

/// Every domestic league, in `LEAGUES` order.
fn is_domestic(key: &str) -> bool {
    !is_international(key) && !is_club_cup(key)
}

/// All club competitions as one training set for the club cups: each domestic
/// league and cup is a competition with its own goal level, club names are
/// canonical so one club is one club everywhere, and each club belongs to the
/// domestic league it plays most in (clubs from leagues outside the data share
/// a last group).
pub struct ClubPool {
    pub matches: Vec<Match>,
    comps: Vec<&'static str>,
    groups: BTreeMap<String, usize>,
}

impl ClubPool {
    /// `loaded` maps each competition key to its results.
    pub fn new(loaded: &BTreeMap<String, Vec<Match>>) -> ClubPool {
        let comps: Vec<&'static str> = LEAGUES
            .iter()
            .map(|(key, _)| *key)
            .filter(|key| is_domestic(key))
            .chain(LEAGUES.iter().map(|(key, _)| *key).filter(|key| is_club_cup(key)))
            .collect();
        let mut matches = Vec::new();
        let mut played: BTreeMap<String, BTreeMap<usize, usize>> = BTreeMap::new();
        for (index, key) in comps.iter().enumerate() {
            for m in loaded.get(*key).map(Vec::as_slice).unwrap_or_default() {
                let mut m = m.clone();
                m.home = canonical_name(&m.home).to_string();
                m.away = canonical_name(&m.away).to_string();
                m.comp = index as u16;
                if is_domestic(key) {
                    for team in [&m.home, &m.away] {
                        *played.entry(team.clone()).or_default().entry(index).or_default() += 1;
                    }
                }
                matches.push(m);
            }
        }
        matches.sort_by(|a, b| a.date.cmp(&b.date));
        let groups = played
            .into_iter()
            .filter_map(|(team, counts)| {
                counts
                    .into_iter()
                    .max_by_key(|&(index, count)| (count, std::cmp::Reverse(index)))
                    .map(|(index, _)| (team, index))
            })
            .collect();
        ClubPool { matches, comps, groups }
    }

    /// Pooling that predicts competition `key`, if it is part of the pool.
    pub fn pooling(&self, key: &str) -> Option<Pooling> {
        let target = self.comps.iter().position(|c| *c == key)?;
        let other = self.comps.iter().filter(|c| is_domestic(c)).count();
        Some(Pooling {
            groups: self.groups.clone(),
            default_group: other,
            n_groups: other + 1,
            target,
        })
    }
}

/// Pooled fits know clubs by canonical name; register the feed's own names too.
fn alias_feed_names(model: &mut Model, feed_names: &BTreeSet<String>) {
    for name in feed_names {
        let canonical = canonical_name(name);
        if canonical != name {
            if let Some(strength) = model.teams.get(canonical).copied() {
                model.teams.insert(name.clone(), strength);
            }
        }
    }
}

pub fn is_international(key: &str) -> bool {
    matches!(key, "world_cup" | "european_championship" | "nations_league"
        | "world_cup_qualifying" | "copa_america" | "africa_cup" | "gold_cup" | "concacaf_nations")
}

pub fn valid_date(s: &str) -> bool {
    s.len() == 10
        && s.bytes().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
        && &s[..4] >= "1900"
        && &s[..4] <= "2200"
        && add_days(s, 0) == s
}

#[derive(Debug, Serialize)]
pub struct FormMatch {
    pub date: String,
    pub opponent: String,
    pub venue: &'static str,
    pub goals: (u8, u8),
    pub result: &'static str,
}

pub fn recent_form(all: &[Match], team: &str, before: &str) -> Vec<FormMatch> {
    let mut games: Vec<_> = all
        .iter()
        .filter(|m| {
            m.date.as_str() < before && m.score.is_some() && (m.home == team || m.away == team)
        })
        .collect();
    games.sort_by(|a, b| b.date.cmp(&a.date));
    let mut out: Vec<_> = games
        .into_iter()
        .take(5)
        .map(|m| {
            let (h, a) = m.score.unwrap();
            let (goals, opponent, venue) = if m.home == team {
                ((h, a), &m.away, "H")
            } else {
                ((a, h), &m.home, "A")
            };
            FormMatch {
                date: m.date.clone(),
                opponent: opponent.clone(),
                venue,
                goals,
                result: match goals.0.cmp(&goals.1) {
                    std::cmp::Ordering::Greater => "W",
                    std::cmp::Ordering::Equal => "D",
                    _ => "L",
                },
            }
        })
        .collect();
    out.reverse();
    out
}

#[derive(Serialize)]
pub struct Forecast {
    pub home: String,
    pub away: String,
    pub prediction: Prediction,
}
#[derive(Serialize)]
pub struct Fixture {
    pub date: String,
    pub home: String,
    pub away: String,
    pub neutral: bool,
    pub prediction: Prediction,
}
#[derive(Serialize)]
pub struct Historical {
    pub date: String,
    pub home: String,
    pub away: String,
    pub actual: (u8, u8),
    pub trained_before: String,
    pub prediction: Prediction,
}
#[derive(Serialize)]
pub struct Team {
    pub name: String,
    pub form: Vec<FormMatch>,
    pub history_matches: usize,
    pub played: usize,
    pub wins: usize,
    pub draws: usize,
    pub losses: usize,
    pub goals_for: usize,
    pub goals_against: usize,
    pub form_adjustment: FormAdjustment,
}
#[derive(Serialize)]
pub struct ArchivedMatch {
    pub date: String,
    pub home: String,
    pub away: String,
    pub score: (u8, u8),
}
#[derive(Serialize)]
pub struct League {
    pub key: String,
    pub name: String,
    pub latest_result: Option<String>,
    pub history_matches: usize,
    pub stats_matches: usize,
    pub overdue: usize,
    pub teams: Vec<Team>,
    pub forecasts: Vec<Forecast>,
    pub fixtures: Vec<Fixture>,
    pub historical: Vec<Historical>,
    pub scoring_factor: f64,
    pub scoring_sample: usize,
    pub archive: Vec<ArchivedMatch>,
    pub shared_international_model: bool,
    /// Club cup fitted together with the domestic leagues (see [`ClubPool`]).
    pub pooled_club_model: bool,
    pub training_pool_matches: usize,
}

/// Same-season teams only; historical predictions refit in seven-day windows.
/// Neither current team-news overrides nor subsequent results enter historical fits.
pub fn build_league(
    key: &str,
    name: &str,
    all: &[Match],
    as_of: &str,
    from: &str,
    cfg: &ModelConfig,
    eligible: Option<&[String]>,
) -> League {
    build_league_with_training(key, name, all, all, as_of, from, cfg, eligible)
}

/// National tournaments share dated national-team results, never domestic club
/// results. Their own archive, fixtures and summaries remain competition scoped.
pub fn build_league_with_training(
    key: &str, name: &str, all: &[Match], training_all: &[Match], as_of: &str,
    from: &str, cfg: &ModelConfig, eligible: Option<&[String]>,
) -> League {
    build_league_pooled(key, name, all, training_all, None, as_of, from, cfg, eligible, None)
}

/// [`build_league_with_training`] with the model fitted on several competitions
/// together (`training_all` then holds all of them and `pooling` says which one
/// this league is). Archive, fixtures and season tables stay competition scoped.
pub fn build_league_pooled(
    key: &str, name: &str, all: &[Match], training_all: &[Match], pooling: Option<&Pooling>,
    as_of: &str, from: &str, cfg: &ModelConfig, eligible: Option<&[String]>, market: Option<&Market>,
) -> League {
    // Where the bookmakers' odds are known for a match, blend them into its prediction.
    let with_market = |m: &Match, p: Prediction| -> Prediction {
        match market.and_then(|odds| odds.get(&market_key(&m.date, &m.home, &m.away))) {
            Some(o) => p.blend_market(*o, MARKET_WEIGHT),
            None => p,
        }
    };
    // Pooled training data uses canonical club names.
    let lookup = |t: &str| -> String {
        if pooling.is_some() { canonical_name(t).to_string() } else { t.to_string() }
    };
    let feed_names: BTreeSet<String> = all
        .iter()
        .flat_map(|m| [m.home.clone(), m.away.clone()])
        .collect();
    let history: Vec<_> = all
        .iter()
        .filter(|m| m.score.is_some() && m.date.as_str() < as_of)
        .cloned()
        .collect();
    let year: u32 = as_of[..4].parse().unwrap();
    let year = if &as_of[5..7] < "07" { year - 1 } else { year };
    let season_start = format!("{year}-07-01");
    let season_end = format!("{}-07-01", year + 1);
    let mut names: BTreeSet<_> = all
        .iter()
        .filter(|m| m.date >= season_start && m.date < season_end)
        .flat_map(|m| [&m.home, &m.away])
        .cloned()
        .filter(|t| eligible.map_or(true, |names| names.contains(t)))
        .collect();
    // A tournament between editions still supports its registered participants.
    if let Some(registered) = eligible {
        names = registered.iter().cloned().collect();
    }
    let training_history: Vec<_> = training_all.iter()
        .filter(|m| m.score.is_some() && m.date.as_str() < as_of).cloned().collect();
    let (mut model, adjustments, scoring_factor, scoring_sample) =
        current_model(&training_history, as_of, cfg, pooling);
    if pooling.is_some() {
        alias_feed_names(&mut model, &names.iter().chain(&feed_names).cloned().collect());
    }
    let teams = names
        .iter()
        .map(|t| {
            let games: Vec<_> = history
                .iter()
                .filter(|m| m.date >= season_start && (m.home == *t || m.away == *t))
                .collect();
            let scores: Vec<_> = games
                .iter()
                .map(|m| {
                    let (h, a) = m.score.unwrap();
                    if m.home == *t {
                        (h, a)
                    } else {
                        (a, h)
                    }
                })
                .collect();
            Team {
                form_adjustment: adjustments.get(&lookup(t)).copied().unwrap_or(FormAdjustment {
                    attack: 1.0,
                    defence: 1.0,
                }),
                name: t.clone(),
                form: recent_form(&training_history, &lookup(t), as_of),
                history_matches: {
                    let t = lookup(t);
                    training_history
                        .iter()
                        .filter(|m| m.home == t || m.away == t)
                        .count()
                },
                played: scores.len(),
                wins: scores.iter().filter(|(h, a)| h > a).count(),
                draws: scores.iter().filter(|(h, a)| h == a).count(),
                losses: scores.iter().filter(|(h, a)| h < a).count(),
                goals_for: scores.iter().map(|(h, _)| *h as usize).sum(),
                goals_against: scores.iter().map(|(_, a)| *a as usize).sum(),
            }
        })
        .collect();
    let mut forecasts = Vec::new();
    for home in &names {
        for away in &names {
            if home != away {
                forecasts.push(Forecast {
                    home: home.clone(),
                    away: away.clone(),
                    prediction: if is_international(key) { model.predict_neutral(home, away) }
                        else { model.predict(home, away) },
                });
            }
        }
    }
    let fixtures = all
        .iter()
        .filter(|m| {
            m.score.is_none()
                && m.date.as_str() >= as_of
                && m.date < season_end
                && names.contains(&m.home)
                && names.contains(&m.away)
        })
        .map(|m| Fixture {
            date: m.date.clone(),
            home: m.home.clone(),
            away: m.away.clone(),
            neutral: m.neutral,
            prediction: with_market(
                m,
                if m.neutral {
                    model.predict_neutral(&m.home, &m.away)
                } else {
                    model.predict(&m.home, &m.away)
                },
            ),
        })
        .collect();
    let mut historical = Vec::new();
    let mut dates: Vec<_> = history
        .iter()
        .filter(|m| m.date.as_str() >= from)
        .map(|m| m.date.as_str())
        .collect();
    dates.sort_unstable();
    dates.dedup();
    let mut i = 0;
    while i < dates.len() {
        let start = dates[i];
        let end = add_days(start, 7);
        let training: Vec<_> = training_history
            .iter()
            .filter(|m| m.date.as_str() < start)
            .cloned()
            .collect();
        // Avoid reporting an unfitted prior as a measured prediction.
        if training.len() >= 100 {
            let (mut fitted, _, _, _) = current_model(&training, start, cfg, pooling);
            if pooling.is_some() {
                alias_feed_names(&mut fitted, &feed_names);
            }
            for m in history
                .iter()
                .filter(|m| m.date.as_str() >= start && m.date < end)
            {
                historical.push(Historical {
                    date: m.date.clone(),
                    home: m.home.clone(),
                    away: m.away.clone(),
                    actual: m.score.unwrap(),
                    trained_before: start.to_string(),
                    prediction: with_market(
                        m,
                        if m.neutral {
                            fitted.predict_neutral(&m.home, &m.away)
                        } else {
                            fitted.predict(&m.home, &m.away)
                        },
                    ),
                });
            }
        }
        while i < dates.len() && dates[i] < end.as_str() {
            i += 1;
        }
    }
    League {
        archive: history
            .iter()
            .map(|m| ArchivedMatch {
                date: m.date.clone(),
                home: m.home.clone(),
                away: m.away.clone(),
                score: m.score.unwrap(),
            })
            .collect(),
        key: key.into(),
        name: name.into(),
        latest_result: history.iter().map(|m| m.date.clone()).max(),
        history_matches: history.len(),
        stats_matches: history.iter().filter(|m| m.stats.is_some()).count(),
        overdue: all
            .iter()
            .filter(|m| m.score.is_none() && m.date >= season_start && m.date.as_str() < as_of)
            .count(),
        teams,
        forecasts,
        fixtures,
        historical,
        scoring_factor,
        scoring_sample,
        shared_international_model: is_international(key),
        pooled_club_model: pooling.is_some(),
        training_pool_matches: training_history.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game(date: &str, score: (u8, u8)) -> Match {
        Match {
            date: date.into(),
            home: "A".into(),
            away: "B".into(),
            score: Some(score),
            neutral: false,
            stats: None,
            comp: 0,
        }
    }
    #[test]
    fn dates_reject_invalid_input() {
        for d in [
            "",
            "ééééé",
            "2026-02-29",
            "2026-13-01",
            "2026-00-01",
            "2026-01-00",
        ] {
            assert!(!valid_date(d), "{d}");
        }
        assert!(valid_date("2024-02-29"));
    }
    #[test]
    fn national_pool_preserves_tournament_scope_and_cutoff() {
        let own = vec![game("2026-06-01", (2, 1))];
        let registered = vec!["A".to_string(), "B".to_string()];
        let mut pool = own.clone();
        pool.push(game("2026-09-01", (1, 1)));
        let base = build_league_with_training("world_cup", "World Cup", &own, &pool,
            "2026-10-08", "2026-07-01", &ModelConfig::default(), Some(&registered));
        pool.push(game("2026-10-08", (9, 0)));
        let later = build_league_with_training("world_cup", "World Cup", &own, &pool,
            "2026-10-08", "2026-07-01", &ModelConfig::default(), Some(&registered));
        assert_eq!(base.teams.len(), 2);
        assert_eq!(base.archive.len(), 1);
        assert_eq!(base.training_pool_matches, 2);
        assert_eq!(base.teams[0].history_matches, 2);
        assert_eq!(base.teams[0].form.len(), 2);
        assert_eq!(base.forecasts[0].prediction.scores, later.forecasts[0].prediction.scores);
        assert!(base.shared_international_model);
    }
    #[test]
    fn form_has_cutoff_and_chronological_order() {
        let games = vec![
            game("2026-08-03", (9, 0)),
            game("2026-08-01", (1, 0)),
            game("2026-08-02", (0, 2)),
        ];
        let f = recent_form(&games, "B", "2026-08-03");
        assert_eq!(
            f.iter().map(|g| g.result).collect::<Vec<_>>(),
            vec!["L", "W"]
        );
        assert_eq!(f[1].goals, (2, 0));
    }
    #[test]
    fn fit_ignores_future_and_same_day_scores_and_stats() {
        let past = vec![game("2026-08-01", (2, 1)), game("2026-08-02", (1, 2))];
        let mut contaminated = past.clone();
        let mut future = game("2026-08-03", (9, 0));
        future.stats = Some(crate::stats::Stats {
            shots_on_target: (99, 1),
            ..Default::default()
        });
        contaminated.push(future);
        contaminated.push(game("2026-09-01", (0, 9)));
        let cfg = ModelConfig::default();
        assert_eq!(
            Model::fit(&past, "2026-08-03", &cfg)
                .predict("A", "B")
                .scores,
            Model::fit(&contaminated, "2026-08-03", &cfg)
                .predict("A", "B")
                .scores
        );
    }
    #[test]
    fn recent_form_is_bounded_and_respects_cutoff() {
        let mut games: Vec<_> = (0..100)
            .map(|i| game(&add_days("2025-01-01", i), (1, 1)))
            .collect();
        for i in 0..5 {
            games.push(game(&add_days("2026-08-01", i), (0, 4)));
        }
        let cfg = ModelConfig::default();
        let (first, a, factor, count) = current_model(&games, "2026-08-06", &cfg, None);
        assert!(a["A"].attack < 1.0);
        assert!(a["A"].defence < 1.0);
        assert!(a
            .values()
            .all(|v| (0.85..=1.15).contains(&v.attack) && (0.85..=1.15).contains(&v.defence)));
        assert_eq!(count, 5);
        assert_eq!(factor, 1.0); // Too few recent matches to calibrate the league.
        games.push(game("2026-08-06", (20, 0)));
        games.push(game("2026-09-01", (20, 0)));
        let (second, _, _, _) = current_model(&games, "2026-08-06", &cfg, None);
        assert_eq!(
            first.predict("A", "B").scores,
            second.predict("A", "B").scores
        );
    }
    #[test]
    fn historical_predictions_do_not_change_when_outcome_changes() {
        let mut all: Vec<_> = (0..110)
            .map(|i| game(&add_days("2025-01-01", i), (2, 1)))
            .collect();
        all.push(game("2026-08-01", (0, 0)));
        let cfg = ModelConfig::default();
        let first = build_league("x", "X", &all, "2026-08-03", "2026-08-01", &cfg, None);
        all.last_mut().unwrap().score = Some((8, 3));
        let second = build_league("x", "X", &all, "2026-08-03", "2026-08-01", &cfg, None);
        assert_eq!(first.historical.len(), 1);
        assert_eq!(
            first.historical[0].prediction.scores,
            second.historical[0].prediction.scores
        );
    }
}

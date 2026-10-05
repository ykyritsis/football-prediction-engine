//! Loading openfootball results.

use serde::Deserialize;

/// One fixture, played (`score` set) or still to come.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    /// ISO date, `YYYY-MM-DD`.
    pub date: String,
    pub home: String,
    pub away: String,
    /// Full-time score, home first.
    pub score: Option<(u8, u8)>,
    /// Shots, corners, cards and so on, when known (see [`crate::stats`]).
    pub stats: Option<crate::stats::Stats>,
}

#[derive(Deserialize)]
struct RawFile {
    matches: Vec<RawMatch>,
}

#[derive(Deserialize)]
struct RawMatch {
    date: Option<String>,
    team1: String,
    team2: String,
    score: Option<RawScore>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawScore {
    Full { ft: Option<Vec<u8>> },
    // Some files use an empty list or other shapes for unplayed games.
    Other(#[allow(dead_code)] serde_json::Value),
}

/// Parse one openfootball season file.
pub fn parse_season(json: &str) -> Result<Vec<Match>, String> {
    let raw: RawFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(raw
        .matches
        .into_iter()
        .filter_map(|m| {
            let date = m.date?;
            let score = match m.score {
                Some(RawScore::Full { ft: Some(ft) }) if ft.len() == 2 => Some((ft[0], ft[1])),
                _ => None,
            };
            Some(Match {
                date,
                home: m.team1,
                away: m.team2,
                score,
                stats: None,
            })
        })
        .collect())
}

/// All seasons of one league (file stem such as `premier_league`) found
/// under `dir` (`data/results`), oldest first.
pub fn load_league(dir: &std::path::Path, league: &str) -> Result<Vec<Match>, String> {
    let mut seasons: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join(format!("{league}.json")).exists())
        .collect();
    seasons.sort();
    let mut out = Vec::new();
    for s in seasons {
        let path = s.join(format!("{league}.json"));
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        out.extend(parse_season(&text).map_err(|e| format!("{}: {e}", path.display()))?);
    }
    // One name per club across seasons.
    if let Ok(text) = std::fs::read_to_string(dir.join("aliases.json")) {
        let aliases: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_str(&text).map_err(|e| format!("aliases.json: {e}"))?;
        for m in &mut out {
            for name in [&mut m.home, &mut m.away] {
                if let Some(serde_json::Value::String(to)) = aliases.get(name.as_str()) {
                    *name = to.clone();
                }
            }
        }
    }
    out.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(out)
}

/// Days since 1970-01-01 for an ISO date (proleptic Gregorian).
pub fn day_number(date: &str) -> i64 {
    let y: i64 = date[0..4].parse().unwrap_or(1970);
    let m: i64 = date[5..7].parse().unwrap_or(1);
    let d: i64 = date[8..10].parse().unwrap_or(1);
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// ISO date `n` days after `date`.
pub fn add_days(date: &str, n: i64) -> String {
    let z = day_number(date) + n + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        assert_eq!(day_number("1970-01-01"), 0);
        assert_eq!(add_days("2024-02-27", 3), "2024-03-01");
        assert_eq!(add_days("2025-12-30", 5), "2026-01-04");
        assert_eq!(day_number("2026-01-04") - day_number("2025-12-30"), 5);
    }

    #[test]
    fn parses_played_and_unplayed() {
        let json = r#"{"name":"x","matches":[
            {"date":"2026-08-15","team1":"A","team2":"B","score":{"ht":[0,0],"ft":[2,1]}},
            {"date":"2026-08-22","team1":"B","team2":"A"}]}"#;
        let m = parse_season(json).unwrap();
        assert_eq!(m[0].score, Some((2, 1)));
        assert_eq!(m[1].score, None);
    }
}

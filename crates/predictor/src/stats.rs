//! Match statistics (shots, corners, fouls, cards, referee) from
//! football-data.co.uk files, mirrored in `data/stats/<season>/<league>.csv`,
//! and their link to the openfootball results.

use crate::data::{day_number, Match};
use std::collections::{BTreeMap, HashMap};

/// Per-match statistics, home first in each pair.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    pub shots: (u16, u16),
    pub shots_on_target: (u16, u16),
    pub fouls: (u16, u16),
    pub corners: (u16, u16),
    pub yellows: (u16, u16),
    pub reds: (u16, u16),
    /// Premier League only in the source data.
    pub referee: Option<String>,
}

/// One row of a statistics file, with the source's own team names.
#[derive(Debug, Clone)]
struct Row {
    date: String,
    home: String,
    away: String,
    goals: (u8, u8),
    stats: Stats,
}

fn parse_csv(text: &str) -> Vec<Row> {
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let cols: HashMap<&str, usize> = header
        .trim_start_matches('\u{feff}')
        .split(',')
        .enumerate()
        .map(|(i, c)| (c.trim(), i))
        .collect();
    fn get<'a>(cols: &HashMap<&str, usize>, f: &[&'a str], k: &str) -> Option<&'a str> {
        cols.get(k).and_then(|&i| f.get(i)).map(|s| s.trim())
    }
    let num = |f: &[&str], k: &str| get(&cols, f, k).and_then(|s| s.parse::<u16>().ok());
    lines
        .filter_map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            let pair = |h: &str, a: &str| Some((num(&f, h)?, num(&f, a)?));
            let date = get(&cols, &f, "Date")?;
            // ISO dates in the mirror; accept dd/mm/yyyy too.
            let date = if date.len() == 10 && &date[2..3] == "/" {
                format!("{}-{}-{}", &date[6..10], &date[3..5], &date[0..2])
            } else {
                date.to_string()
            };
            let goals = pair("FTHG", "FTAG")?;
            Some(Row {
                date,
                home: get(&cols, &f, "HomeTeam")?.to_string(),
                away: get(&cols, &f, "AwayTeam")?.to_string(),
                goals: (goals.0 as u8, goals.1 as u8),
                stats: Stats {
                    shots: pair("HS", "AS")?,
                    shots_on_target: pair("HST", "AST")?,
                    fouls: pair("HF", "AF")?,
                    corners: pair("HC", "AC")?,
                    yellows: pair("HY", "AY")?,
                    reds: pair("HR", "AR")?,
                    referee: get(&cols, &f, "Referee")
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                },
            })
        })
        .collect()
}

/// Load every season's statistics for `league` under `dir` (`data/stats`)
/// and attach them to the matching results in `matches` (same league).
/// Team names differ between the sources, so they are linked by date and
/// score. Returns how many matches got statistics.
pub fn attach(dir: &std::path::Path, league: &str, matches: &mut [Match]) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut rows = Vec::new();
    for e in entries.flatten() {
        if let Ok(text) = std::fs::read_to_string(e.path().join(format!("{league}.csv"))) {
            rows.extend(parse_csv(&text));
        }
    }
    // Index results by day.
    let mut by_day: HashMap<i64, Vec<usize>> = HashMap::new();
    for (i, m) in matches.iter().enumerate() {
        by_day.entry(day_number(&m.date)).or_default().push(i);
    }
    let near = |date: &str| {
        let d = day_number(date);
        (d - 1..=d + 1)
            .flat_map(|k| by_day.get(&k).cloned().unwrap_or_default())
            .collect::<Vec<_>>()
    };
    // Vote on name pairs: a stats row and a result on the same day with the
    // same score are probably the same match.
    let mut votes: BTreeMap<(String, String), u32> = BTreeMap::new();
    for r in &rows {
        let cands: Vec<usize> = near(&r.date)
            .into_iter()
            .filter(|&i| matches[i].score == Some(r.goals))
            .collect();
        for i in cands {
            *votes
                .entry((r.home.clone(), matches[i].home.clone()))
                .or_default() += 1;
            *votes
                .entry((r.away.clone(), matches[i].away.clone()))
                .or_default() += 1;
        }
    }
    let mut best: HashMap<String, (u32, String)> = HashMap::new();
    for ((src, dst), n) in votes {
        let e = best.entry(src).or_insert((0, String::new()));
        if n > e.0 {
            *e = (n, dst);
        }
    }
    let mut linked = 0;
    for r in rows {
        let (Some((_, home)), Some((_, away))) = (best.get(&r.home), best.get(&r.away)) else {
            continue;
        };
        if let Some(i) = near(&r.date)
            .into_iter()
            .find(|&i| &matches[i].home == home && &matches[i].away == away)
        {
            if matches[i].stats.is_none() {
                matches[i].stats = Some(r.stats);
                linked += 1;
            }
        }
    }
    linked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_row() {
        let csv = "Date,HomeTeam,AwayTeam,FTHG,FTAG,FTR,HTHG,HTAG,HTR,Referee,HS,AS,HST,AST,HF,AF,HC,AC,HY,AY,HR,AR\n\
                   2026-05-24,West Ham,Leeds,3,0,H,0,0,D,A Taylor,16,13,9,3,11,14,6,4,0,3,0,0\n";
        let r = parse_csv(csv);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].goals, (3, 0));
        assert_eq!(r[0].stats.shots_on_target, (9, 3));
        assert_eq!(r[0].stats.yellows, (0, 3));
        assert_eq!(r[0].stats.referee.as_deref(), Some("A Taylor"));
    }
}

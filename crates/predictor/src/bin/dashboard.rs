//! Generate a self-contained dashboard using the existing Rust prediction model.
use predictor::{
    dashboard::{self, LEAGUES},
    data::add_days,
    load_league, stats, ModelConfig,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn run() -> Result<(), String> {
    let raw: Vec<_> = std::env::args().skip(1).collect();
    let saved_only = raw.iter().any(|a| a == "--static");
    let args: Vec<_> = raw.into_iter().filter(|a| a != "--static").collect();
    if args.is_empty() || args.len() > 3 || !dashboard::valid_date(&args[0]) {
        return Err(
            "usage: dashboard AS_OF [OUTPUT.html] [HISTORY_FROM] [--static] (dates: YYYY-MM-DD)".into(),
        );
    }
    let as_of = &args[0];
    let output = args.get(1).map(String::as_str).unwrap_or("dashboard.html");
    let from = args.get(2).cloned().unwrap_or_else(|| add_days(as_of, -90));
    if !dashboard::valid_date(&from) || from.as_str() >= as_of.as_str() {
        return Err("HISTORY_FROM must be a valid date before AS_OF".into());
    }
    let dir = std::env::var_os("PREDICTOR_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data/results"));
    let cfg = ModelConfig::default();
    let catalog_path = dir.parent().unwrap_or(&dir).join("catalog/catalog.json");
    let catalog: serde_json::Value = match std::fs::read_to_string(&catalog_path) {
        Ok(text) => {
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", catalog_path.display()))?
        }
        Err(_) => json!({"teams":{},"competitions":[],"fixtures":{}}),
    };
    let mut leagues = Vec::new();
    let mut national_history = Vec::new();
    for (key, _) in LEAGUES.iter().filter(|(key, _)| dashboard::is_international(key)) {
        national_history.extend(load_league(&dir, key)?);
    }
    national_history.sort_by(|a,b| (&a.date,&a.home,&a.away).cmp(&(&b.date,&b.home,&b.away)));
    national_history.dedup_by(|a,b| a.date == b.date && a.home == b.home && a.away == b.away);
    // Club cups are fitted together with the domestic leagues, so a club's league
    // games count towards its European odds.
    let mut loaded = BTreeMap::new();
    for (key, _) in LEAGUES.iter().filter(|(key, _)| !dashboard::is_international(key)) {
        let mut results = load_league(&dir, key)?;
        stats::attach(&dir.parent().unwrap_or(&dir).join("stats"), key, &mut results);
        loaded.insert(key.to_string(), results);
    }
    let club_pool = dashboard::ClubPool::new(&loaded);
    for (key, name) in LEAGUES {
        eprintln!("Building {name}...");
        let mut all = match loaded.get(key) {
            Some(results) => results.clone(),
            None => load_league(&dir, key)?,
        };
        if all.is_empty() && ["premier_league", "la_liga", "serie_a", "bundesliga", "ligue_1"].contains(&key) {
            return Err(format!("No results found for {name} in {}", dir.display()));
        }
        if all.iter().any(|m| !dashboard::valid_date(&m.date)) {
            return Err(format!("Invalid match date in {name}"));
        }
        if !loaded.contains_key(key) {
            stats::attach(&dir.parent().unwrap_or(&dir).join("stats"), key, &mut all);
        }
        let eligible: Option<Vec<String>> = catalog["teams"]
            .as_object()
            .map(|teams| {
                teams
                    .iter()
                    .filter(|(_, t)| {
                        t["competitions"]
                            .as_array()
                            .is_some_and(|cs| cs.iter().any(|c| c.as_str() == Some(key)))
                    })
                    .map(|(n, _)| n.clone())
                    .collect()
            })
            .filter(|v: &Vec<String>| !v.is_empty());
        let pooling = if dashboard::is_club_cup(key) { club_pool.pooling(key) } else { None };
        leagues.push(dashboard::build_league_pooled(
            key,
            name,
            &all,
            if pooling.is_some() {
                &club_pool.matches
            } else if dashboard::is_international(key) {
                &national_history
            } else {
                &all
            },
            pooling.as_ref(),
            as_of,
            &from,
            &cfg,
            eligible.as_deref(),
        ));
    }
    let tm_path = catalog_path.with_file_name("transfermarkt.json");
    let transfermarkt: serde_json::Value = match std::fs::read_to_string(&tm_path) {
        Ok(text) => {
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", tm_path.display()))?
        }
        Err(_) => json!({}),
    };
    let data = json!({"as_of":as_of,"history_from":from,"saved_only":saved_only,"half_life_days":cfg.half_life_days,"leagues":leagues,"catalog":catalog,"transfermarkt":transfermarkt});
    // Never allow a data string to terminate the inert JSON script element.
    let safe = serde_json::to_string(&data)
        .map_err(|e| e.to_string())?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    let html = include_str!("../../web/index.html")
        .replace("__PRESENTATION_CODE__", include_str!("../../web/presentation.js"))
        .replace("__PROFILE_CODE__", include_str!("../../web/profiles.js"))
        .replace("__BROADCAST_CODE__", include_str!("../../web/broadcast.js"))
        .replace("__APP_CSS__", include_str!("../../web/app.css"))
        .replace("__APP_JS__", include_str!("../../web/app.js"))
        .replace(
            "__EXPERIENCE_CODE__",
            include_str!("../../web/experience.js"),
        )
        .replace("__HISTORY_CODE__", include_str!("../../web/history.js"))
        .replace(
            "__SIMULATION_CODE__",
            include_str!("../../web/simulation.js"),
        )
        .replace("__PREDICTOR_DATA__", &safe);
    std::fs::write(output, html).map_err(|e| format!("{output}: {e}"))?;
    eprintln!("Saved {output}. Open it in a browser; no server or dependencies required.");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

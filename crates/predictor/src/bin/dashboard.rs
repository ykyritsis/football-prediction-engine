//! Generate a self-contained dashboard using the existing Rust prediction model.
use predictor::{
    dashboard::{self, LEAGUES},
    data::add_days,
    load_league, stats, ModelConfig,
};
use serde_json::json;
use std::path::PathBuf;

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args.len() > 3 || !dashboard::valid_date(&args[0]) {
        return Err(
            "usage: dashboard AS_OF [OUTPUT.html] [HISTORY_FROM] (dates: YYYY-MM-DD)".into(),
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
    for (key, name) in LEAGUES {
        eprintln!("Building {name}...");
        let mut all = load_league(&dir, key)?;
        if all.is_empty()
            && !["champions_league", "europa_league", "conference_league"].contains(&key)
        {
            return Err(format!("No results found for {name} in {}", dir.display()));
        }
        if all.iter().any(|m| !dashboard::valid_date(&m.date)) {
            return Err(format!("Invalid match date in {name}"));
        }
        stats::attach(&dir.parent().unwrap_or(&dir).join("stats"), key, &mut all);
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
        leagues.push(dashboard::build_league(
            key,
            name,
            &all,
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
    let data = json!({"as_of":as_of,"history_from":from,"half_life_days":cfg.half_life_days,"leagues":leagues,"catalog":catalog,"transfermarkt":transfermarkt});
    // Never allow a data string to terminate the inert JSON script element.
    let safe = serde_json::to_string(&data)
        .map_err(|e| e.to_string())?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    let html = include_str!("../../web/index.html")
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

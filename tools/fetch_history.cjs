#!/usr/bin/env node
// Fetches final league tables for each domestic league from 2000-01 to 2025-26, using ESPN's
// public standings feed. Writes data/history/standings.json: { [competitionKey]: { [startYear]: rows } }.
// Each row: { pos, team, pts, gp, w, d, l, gf, ga, gd }. Cached per season, so reruns only fetch what is missing.
// Usage: node tools/fetch_history.cjs [fromYear] [toYear]
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const outFile = path.join(root, "data", "history", "standings.json");
const competitions = require(path.join(root, "data", "catalog", "competitions.json"));
const list = Array.isArray(competitions) ? competitions : competitions.competitions || Object.values(competitions);
const from = Number(process.argv[2] || 2000);
const to = Number(process.argv[3] || 2025);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function statMap(stats = []) {
  const out = {};
  for (const s of stats) out[s.name] = s.value;
  return out;
}

function parseRows(json) {
  const group = json?.children?.[0]?.standings;
  const entries = group?.entries || [];
  return entries.map((e, i) => {
    const st = statMap(e.stats);
    return {
      pos: st.rank ?? i + 1,
      team: e.team?.displayName || e.team?.name || "Unknown",
      pts: st.points ?? 0,
      gp: st.gamesPlayed ?? 0,
      w: st.wins ?? 0,
      d: st.ties ?? st.draws ?? 0,
      l: st.losses ?? 0,
      gf: st.pointsFor ?? st.goalsFor ?? 0,
      ga: st.pointsAgainst ?? st.goalsAgainst ?? 0,
      gd: st.pointDifferential ?? st.goalDifferential ?? 0,
    };
  });
}

async function fetchSeason(slug, startYear) {
  const url = `https://site.api.espn.com/apis/v2/sports/soccer/${slug}/standings?season=${startYear}`;
  for (let attempt = 1; attempt <= 3; attempt++) {
    const res = await fetch(url);
    if (res.status === 429) { await sleep(30000); continue; }
    if (!res.ok) return null;
    const rows = parseRows(await res.json());
    return rows.length ? rows : null;
  }
  return null;
}

(async () => {
  const data = fs.existsSync(outFile) ? JSON.parse(fs.readFileSync(outFile, "utf8")) : {};
  const leagues = list.filter((c) => c.kind === "domestic" && c.slug);
  let fetched = 0, missing = 0;
  for (const league of leagues) {
    data[league.key] ||= {};
    for (let y = from; y <= to; y++) {
      if (data[league.key][y]) continue;
      const rows = await fetchSeason(league.slug, y);
      if (rows) { data[league.key][y] = rows; fetched++; } else missing++;
      await sleep(250);
    }
    fs.mkdirSync(path.dirname(outFile), { recursive: true });
    fs.writeFileSync(outFile, JSON.stringify(data));
    console.log(`${league.key}: ${Object.keys(data[league.key]).length} season(s)`);
  }
  console.log(`fetched ${fetched} season(s), ${missing} unavailable`);
})();

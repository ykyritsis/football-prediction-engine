#!/usr/bin/env node
// Writes a short AI recap for each competition's results in the last 7 days.
// The model only sees scores saved in data/results, and each recap is cached in
// data/ai/recaps.json, so a competition-week is only generated once.
// Without DEEPSEEK_API_KEY the cache is copied and nothing is generated.
// Usage: node tools/ai_recaps.cjs [distDir]   (runs automatically after the site build)
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const cacheFile = path.join(root, "data", "ai", "recaps.json");
const site = require(path.join(root, "data", "catalog", "site.json"));
const WINDOW_DAYS = 7;
const MAX_NEW_PER_RUN = 20;

function addDays(iso, days) {
  const d = new Date(iso + "T00:00:00Z");
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

// Collect played matches in the window, grouped by competition.
function recentResults() {
  const start = addDays(site.as_of, -WINDOW_DAYS);
  const byComp = new Map();
  const resultsDir = path.join(root, "data", "results");
  for (const season of fs.readdirSync(resultsDir)) {
    const dir = path.join(resultsDir, season);
    if (!fs.statSync(dir).isDirectory()) continue;
    for (const file of fs.readdirSync(dir)) {
      if (!file.endsWith(".json")) continue;
      const data = JSON.parse(fs.readFileSync(path.join(dir, file), "utf8"));
      const key = file.replace(/\.json$/, "");
      for (const m of data.matches || []) {
        if (!m.score?.ft || m.date < start || m.date > site.as_of) continue;
        if (!byComp.has(key)) byComp.set(key, { name: data.name || key, matches: [] });
        byComp.get(key).matches.push({ date: m.date, home: m.team1, away: m.team2, ft: m.score.ft });
      }
    }
  }
  return byComp;
}

async function generate(name, matches, apiKey) {
  const lines = matches
    .sort((a, b) => a.date.localeCompare(b.date))
    .map((m) => `${m.date}: ${m.home} ${m.ft[0]}-${m.ft[1]} ${m.away}`)
    .join("\n");
  const prompt = `Write a short recap of recent ${name} results for a football fan website.
Use ONLY these results. Do not add goals, scorers, injuries, transfers or any other facts.
Two or three sentences. Mention the biggest margin and any draws.

Results:
${lines}

Return only a JSON object: {"recap": "text"}`;
  const res = await fetch("https://api.deepseek.com/chat/completions", {
    method: "POST",
    headers: { "Content-Type": "application/json", Authorization: `Bearer ${apiKey}` },
    body: JSON.stringify({
      model: process.env.DEEPSEEK_MODEL || "deepseek-chat",
      messages: [{ role: "user", content: prompt }],
      response_format: { type: "json_object" },
      temperature: 0.3,
      max_tokens: 250,
    }),
  });
  if (!res.ok) throw new Error(`DeepSeek returned ${res.status}`);
  const data = await res.json();
  const parsed = JSON.parse(data?.choices?.[0]?.message?.content || "{}");
  return String(parsed.recap || "").trim();
}

async function run(distDir) {
  const cache = fs.existsSync(cacheFile) ? JSON.parse(fs.readFileSync(cacheFile, "utf8")) : {};
  const apiKey = process.env.DEEPSEEK_API_KEY;
  const weekEnd = site.as_of;
  const results = recentResults();

  if (!apiKey) {
    console.log("DEEPSEEK_API_KEY not set: using cached recaps only");
  } else {
    let generated = 0;
    for (const [key, { name, matches }] of results) {
      const id = `${key}|${weekEnd}`;
      if (cache[id]?.text || generated >= MAX_NEW_PER_RUN) continue;
      try {
        const text = await generate(name, matches, apiKey);
        if (text) {
          cache[id] = { text, name, generated_at: new Date().toISOString() };
          generated += 1;
        }
      } catch (err) {
        console.error(`recap skipped for ${key}: ${err.message}`);
      }
    }
    console.log(`generated ${generated} new recap(s)`);
  }

  fs.mkdirSync(path.dirname(cacheFile), { recursive: true });
  fs.writeFileSync(cacheFile, JSON.stringify(cache, null, 2) + "\n");

  // The site reads only the current week's recaps: key -> { name, text }.
  const current = {};
  for (const [id, v] of Object.entries(cache)) {
    if (id.endsWith(`|${weekEnd}`)) current[id.slice(0, -(weekEnd.length + 1))] = { name: v.name, text: v.text };
  }
  fs.mkdirSync(path.join(distDir, "assets"), { recursive: true });
  fs.writeFileSync(path.join(distDir, "assets", "recaps.json"), JSON.stringify(current));
  console.log(`recaps.json: ${Object.keys(current).length} competition(s) this week`);
}

module.exports = { run };

if (require.main === module) {
  run(path.resolve(root, process.argv[2] || "dist")).catch((err) => {
    console.error(err);
    process.exit(1);
  });
}

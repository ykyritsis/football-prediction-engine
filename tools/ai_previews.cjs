#!/usr/bin/env node
// Writes short AI match previews for fixtures in the next 10 days using the DeepSeek API.
// Results are cached in data/ai/previews.json, so each fixture is only generated once.
// Without DEEPSEEK_API_KEY the cache is copied as-is and nothing is generated.
// Usage: node tools/ai_previews.cjs [distDir]   (runs automatically after the site build)
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const cacheFile = path.join(root, "data", "ai", "previews.json");
const site = require(path.join(root, "data", "catalog", "site.json"));
const WINDOW_DAYS = 10;
const MAX_NEW_PER_RUN = 12;

function dashboardData(distDir) {
  const html = fs.readFileSync(path.join(distDir, "index.html"), "utf8");
  const match = html.match(/<script id="data"[^>]*>([\s\S]*?)<\/script>/);
  if (!match) throw new Error("dashboard data block not found in index.html");
  return JSON.parse(match[1]);
}

function addDays(iso, days) {
  const d = new Date(iso + "T00:00:00Z");
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

function upcomingFixtures(data) {
  const end = addDays(site.as_of, WINDOW_DAYS);
  const out = [];
  for (const league of data.leagues || []) {
    for (const m of league.fixtures || []) {
      if (m.date >= site.as_of && m.date <= end) {
        out.push({ key: [league.key, m.date, m.home, m.away].join("|"), league, m });
      }
    }
  }
  return out;
}

async function generate(fixture, apiKey) {
  const { league, m } = fixture;
  const [h, a] = m.prediction?.most_likely_score || [];
  const forecast = Number.isInteger(h) && Number.isInteger(a) ? `${h}-${a}` : "not available";
  const prompt = `Write a two-sentence neutral match preview for a football fan website.
Use only these facts: competition ${league.name}, date ${m.date}, home team ${m.home}, away team ${m.away}.
The statistical model's most likely score is ${forecast}. Say it is a model forecast, not a prediction of the result.
Do not mention injuries, form, recent results, transfers or any other facts.
Return only a JSON object: {"preview": "text"}`;

  const res = await fetch("https://api.deepseek.com/chat/completions", {
    method: "POST",
    headers: { "Content-Type": "application/json", Authorization: `Bearer ${apiKey}` },
    body: JSON.stringify({
      model: process.env.DEEPSEEK_MODEL || "deepseek-chat",
      messages: [{ role: "user", content: prompt }],
      response_format: { type: "json_object" },
      temperature: 0.3,
      max_tokens: 200,
    }),
  });
  if (!res.ok) throw new Error(`DeepSeek returned ${res.status}`);
  const data = await res.json();
  const parsed = JSON.parse(data?.choices?.[0]?.message?.content || "{}");
  return String(parsed.preview || "").trim();
}

async function run(distDir) {
  const cache = fs.existsSync(cacheFile) ? JSON.parse(fs.readFileSync(cacheFile, "utf8")) : {};
  const apiKey = process.env.DEEPSEEK_API_KEY;
  const fixtures = upcomingFixtures(dashboardData(distDir));

  if (!apiKey) {
    console.log("DEEPSEEK_API_KEY not set: using cached previews only");
  } else {
    let generated = 0;
    for (const fixture of fixtures) {
      if (cache[fixture.key]?.text || generated >= MAX_NEW_PER_RUN) continue;
      try {
        const text = await generate(fixture, apiKey);
        if (text) {
          cache[fixture.key] = { text, date: fixture.m.date, generated_at: new Date().toISOString() };
          generated += 1;
        }
      } catch (err) {
        console.error(`preview skipped for ${fixture.key}: ${err.message}`);
      }
    }
    console.log(`generated ${generated} new preview(s)`);
  }

  fs.mkdirSync(path.dirname(cacheFile), { recursive: true });
  fs.writeFileSync(cacheFile, JSON.stringify(cache, null, 2) + "\n");

  // The site reads a flat map of key -> text.
  const flat = Object.fromEntries(Object.entries(cache).map(([k, v]) => [k, v.text]));
  fs.mkdirSync(path.join(distDir, "assets"), { recursive: true });
  fs.writeFileSync(path.join(distDir, "assets", "previews.json"), JSON.stringify(flat));
  console.log(`previews.json: ${Object.keys(flat).length} entries`);
}

module.exports = { run };

if (require.main === module) {
  run(path.resolve(root, process.argv[2] || "dist")).catch((err) => {
    console.error(err);
    process.exit(1);
  });
}

#!/usr/bin/env node
// Two short AI features generated with the DeepSeek API during the site build:
//   1. "Why" notes for upcoming fixtures: what the model leans on, from probabilities and recent form.
//   2. Weekly form storylines for teams on a hot or cold run (4+ wins or losses in their last 5).
// Results are cached in data/ai/extras.json, so each item is generated once per fixture or week.
// Without DEEPSEEK_API_KEY the cache is copied as-is and nothing new is generated.
// Usage: node tools/ai_extras.cjs [distDir]   (runs automatically after the site build)
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const cacheFile = path.join(root, "data", "ai", "extras.json");
const site = require(path.join(root, "data", "catalog", "site.json"));
const WINDOW_DAYS = 10;
const MAX_NEW_PER_RUN = 16;

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

// The Monday of the as_of week, used as the cache key for weekly storylines.
function weekKey(iso) {
  const d = new Date(iso + "T00:00:00Z");
  const offset = (d.getUTCDay() + 6) % 7;
  d.setUTCDate(d.getUTCDate() - offset);
  return d.toISOString().slice(0, 10);
}

function lastFive(league, name) {
  const team = (league.teams || []).find((t) => t.name === name);
  return (team?.form || []).slice(-5);
}

function formLine(f) {
  const [gf, ga] = f.goals || [0, 0];
  return `${f.result} ${gf}-${ga} vs ${f.opponent} (${f.venue === "H" ? "home" : "away"}, ${f.date})`;
}

async function complete(prompt, apiKey) {
  const res = await fetch("https://api.deepseek.com/chat/completions", {
    method: "POST",
    headers: { "Content-Type": "application/json", Authorization: `Bearer ${apiKey}` },
    body: JSON.stringify({
      model: process.env.DEEPSEEK_MODEL || "deepseek-chat",
      messages: [{ role: "user", content: prompt }],
      response_format: { type: "json_object" },
      temperature: 0.3,
      max_tokens: 220,
    }),
  });
  if (!res.ok) throw new Error(`DeepSeek returned ${res.status}`);
  const data = await res.json();
  const parsed = JSON.parse(data?.choices?.[0]?.message?.content || "{}");
  return String(parsed.text || "").trim();
}

function whyPrompt(league, m) {
  const p = m.prediction || {};
  const [h, a] = p.most_likely_score || [];
  const homeForm = lastFive(league, m.home).map(formLine);
  const awayForm = lastFive(league, m.away).map(formLine);
  return {
    key: [league.key, m.date, m.home, m.away].join("|"),
    prompt: `Write one short sentence, under 30 words, saying what drives the forecast lean for this fixture. Do not list the percentages.
Competition: ${league.name}. Date: ${m.date}. Home: ${m.home}. Away: ${m.away}.
Predicted result: ${p.home_win === Math.max(p.home_win, p.draw, p.away_win) ? m.home + " to win" : p.away_win === Math.max(p.home_win, p.draw, p.away_win) ? m.away + " to win" : "a draw"}. Your sentence must agree with the predicted result.
Model probabilities: home win ${(p.home_win * 100).toFixed(0)}%, draw ${(p.draw * 100).toFixed(0)}%, away win ${(p.away_win * 100).toFixed(0)}%.
Most likely score: ${Number.isInteger(h) && Number.isInteger(a) ? h + "-" + a : "not available"}.
Recent form, most recent last: ${m.home}: ${homeForm.join("; ") || "not available"}. ${m.away}: ${awayForm.join("; ") || "not available"}.
Use only these facts. Do not mention injuries, transfers or anything else.
Return only a JSON object: {"text": "..."}`,
  };
}

function storyPrompt(league, team, kind, form) {
  return `Write two neutral sentences about a football team's recent form for a fan website.
Competition: ${league.name}. Team: ${team}. The team is on a ${kind === "hot" ? "good" : "poor"} run.
Last five results, most recent last: ${form.map(formLine).join("; ")}.
Use only these facts. Do not mention injuries, transfers, or anything not listed.
Return only a JSON object: {"text": "..."}`;
}

async function run(distDir) {
  const cache = fs.existsSync(cacheFile) ? JSON.parse(fs.readFileSync(cacheFile, "utf8")) : { why: {}, storylines: {} };
  cache.why ||= {};
  cache.storylines ||= {};
  const apiKey = process.env.DEEPSEEK_API_KEY;
  const data = dashboardData(distDir);
  const end = addDays(site.as_of, WINDOW_DAYS);
  const week = weekKey(site.as_of);

  const whyJobs = [];
  const storyJobs = [];
  for (const league of data.leagues || []) {
    for (const m of league.fixtures || []) {
      if (m.date >= site.as_of && m.date <= end) whyJobs.push({ league, m });
    }
    for (const team of league.teams || []) {
      const form = (team.form || []).slice(-5);
      const wins = form.filter((f) => f.result === "W").length;
      const losses = form.filter((f) => f.result === "L").length;
      const kind = wins >= 4 ? "hot" : losses >= 4 ? "cold" : null;
      if (kind) storyJobs.push({ league, team: team.name, kind, form, key: `${league.key}|${team.name}|${week}` });
    }
  }

  if (!apiKey) {
    console.log("DEEPSEEK_API_KEY not set: using cached AI extras only");
  } else {
    let generated = 0;
    for (const job of whyJobs) {
      const { key, prompt } = whyPrompt(job.league, job.m);
      if (cache.why[key]?.text || generated >= MAX_NEW_PER_RUN) continue;
      try {
        const text = await complete(prompt, apiKey);
        if (text) cache.why[key] = { text, generated_at: new Date().toISOString() };
        generated += 1;
      } catch (err) {
        console.error(`why note skipped for ${key}: ${err.message}`);
      }
    }
    for (const job of storyJobs) {
      if (cache.storylines[job.key]?.text || generated >= MAX_NEW_PER_RUN) continue;
      try {
        const text = await complete(storyPrompt(job.league, job.team, job.kind, job.form), apiKey);
        if (text) cache.storylines[job.key] = { text, kind: job.kind, generated_at: new Date().toISOString() };
        generated += 1;
      } catch (err) {
        console.error(`storyline skipped for ${job.team}: ${err.message}`);
      }
    }
    console.log(`generated ${generated} new AI extra(s)`);
  }

  fs.mkdirSync(path.dirname(cacheFile), { recursive: true });
  fs.writeFileSync(cacheFile, JSON.stringify(cache, null, 2) + "\n");

  // The site reads flat maps: fixture key -> text, and this week's storylines.
  const why = {};
  for (const { league, m } of whyJobs) {
    const { key } = whyPrompt(league, m);
    if (cache.why[key]?.text) why[key] = cache.why[key].text;
  }
  const storylines = storyJobs
    .filter((job) => cache.storylines[job.key]?.text)
    .map((job) => ({ league: job.league.name, team: job.team, kind: job.kind, text: cache.storylines[job.key].text }));

  fs.mkdirSync(path.join(distDir, "assets"), { recursive: true });
  fs.writeFileSync(path.join(distDir, "assets", "why.json"), JSON.stringify(why));
  fs.writeFileSync(path.join(distDir, "assets", "storylines.json"), JSON.stringify(storylines));
  console.log(`why.json: ${Object.keys(why).length} note(s); storylines.json: ${storylines.length} this week`);
}

module.exports = { run };

if (require.main === module) {
  run(path.resolve(root, process.argv[2] || "dist")).catch((err) => {
    console.error(err);
    process.exit(1);
  });
}

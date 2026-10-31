// Vercel serverless function: answers a question about one fixture.
// The browser sends the fixture facts it already shows (teams, date, forecast). The model
// is told to use only those facts. The DeepSeek key stays on Vercel as DEEPSEEK_API_KEY.

const WINDOW_MS = 10 * 60 * 1000;
const LIMIT = 10;
const MAX_QUESTION = 200;
const hits = new Map();

function allowed(ip) {
  const now = Date.now();
  const entry = hits.get(ip);
  if (!entry || now > entry.resetAt) {
    hits.set(ip, { count: 1, resetAt: now + WINDOW_MS });
    return true;
  }
  if (entry.count >= LIMIT) return false;
  entry.count += 1;
  return true;
}

function clean(value, max = 120) {
  return String(value ?? "").replace(/[\r\n]+/g, " ").slice(0, max);
}

function teamLine(c) {
  if (!c || typeof c !== "object") return "not available";
  return [clean(c.name, 60), c.standing ? "league: " + clean(c.standing, 80) : null, c.form ? "last five: " + clean(c.form, 240) : null, c.top_scorers ? "top scorers: " + clean(c.top_scorers, 160) : null].filter(Boolean).join("; ");
}

// Web sources for the question from DeepSeek's Search API. Returns "" if search is unavailable.
let lastSearchError = null;
async function webSources(apiKey, f, question) {
  lastSearchError = null;
  const query = clean(`${f.home || ""} ${f.away || ""} ${question}`, 128).trim();
  if (query.length < 3) return "";
  try {
    const res = await fetch("https://api.deepseek.com/v1/tools/search", {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${apiKey}` },
      body: JSON.stringify({ query, count: 5, need_content: false, ori_question: clean(question, 128) }),
    });
    if (!res.ok) throw new Error(`search returned ${res.status}`);
    const data = await res.json();
    return (data.results || [])
      .filter((r) => r.url)
      .slice(0, 5)
      .map((r) => `- ${clean(r.title, 160)} (${clean(r.site_name || "unknown site", 60)}, ${clean(r.date || "date unknown", 30)}): ${clean(r.summary || r.snippet, 500)} [${clean(r.url, 300)}]`)
      .join("\n");
  } catch (err) {
    console.error("web search skipped:", err.message);
    lastSearchError = err.message;
    return "";
  }
}

module.exports = async function handler(req, res) {
  res.setHeader("Content-Type", "application/json");
  if (req.method !== "POST") return res.status(405).json({ error: "Use POST." });

  const apiKey = process.env.DEEPSEEK_API_KEY;
  if (!apiKey) return res.status(503).json({ error: "Match questions are not configured yet." });

  const ip = String(req.headers["x-forwarded-for"] || "").split(",")[0].trim() || "unknown";
  if (!allowed(ip)) return res.status(429).json({ error: "Too many questions. Try again in a few minutes." });

  let body = req.body;
  if (typeof body === "string") {
    try { body = JSON.parse(body); } catch { body = {}; }
  }
  const question = clean(body?.question, MAX_QUESTION).trim();
  const f = body?.fixture || {};
  if (question.length < 3) return res.status(400).json({ error: "Ask a question of at least three characters." });

  const probs = f.probabilities || {};
  const pct = (x) => (Number.isFinite(x) ? `${Math.round(x * 100)}%` : "not available");
  const sources = await webSources(apiKey, f, question);
  const prompt = `You answer questions about one football fixture for a fan website.
Answer any question about this match: the teams, players, venue, competition, history, head-to-head, form or what the forecast means. Start from these facts and the web sources, and add general football knowledge where it helps. When you use a web source, name it and include its link. Say when squads, injuries, managers, transfers or form may have changed since this data was compiled. Never invent exact results or statistics; if you are not sure, say so. Do not give betting advice. If a question has nothing to do with this match, say you can only help with this match.
Competition: ${clean(f.competition)}
Date: ${clean(f.date, 20)}
Home team: ${clean(f.home)}
Away team: ${clean(f.away)}
Model forecast: home win ${pct(probs.home_win)}, draw ${pct(probs.draw)}, away win ${pct(probs.away_win)}.
Predicted score in the model: ${clean(f.most_likely_score, 20) || "not available"}.
Web sources found for this question (may be out of date; name the source and give its link when you use one):
${sources || "none found"}
Kick-off: ${clean(f.kickoff, 60) || "not available"}
Home ground: ${clean(f.venue, 120) || "not available"}
Head-to-head in the archive: ${clean(f.head_to_head, 400) || "no recent meetings in the archive"}
Home team context: ${teamLine(f.home_context)}
Away team context: ${teamLine(f.away_context)}
When you talk about the forecast, say it is a model estimate, not a fact about the result. Keep the answer to two to four sentences.

Return only a JSON object: {"answer": "text"}
Question: ${question}`;

  try {
    const upstream = await fetch("https://api.deepseek.com/chat/completions", {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${apiKey}` },
      body: JSON.stringify({
        model: process.env.DEEPSEEK_MODEL || "deepseek-chat",
        messages: [{ role: "user", content: prompt }],
        response_format: { type: "json_object" },
        temperature: 0.2,
        max_tokens: 400,
      }),
    });
    if (!upstream.ok) throw new Error(`DeepSeek returned ${upstream.status}`);
    const data = await upstream.json();
    const parsed = JSON.parse(data?.choices?.[0]?.message?.content || "{}");
    const sourceCount = sources ? sources.split("\n").length : 0;
    return res.status(200).json({ answer: String(parsed.answer || "No answer was returned."), sources: sourceCount, search_error: lastSearchError });
  } catch (err) {
    console.error("match-question failed", err.message);
    return res.status(502).json({ error: "The answer could not be generated. Try again later." });
  }
};

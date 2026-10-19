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
  const prompt = `You answer questions about one football fixture for a fan website.
Use ONLY these facts. If the question needs anything else (injuries, form, team news, odds), say you do not have that information.
Competition: ${clean(f.competition)}
Date: ${clean(f.date, 20)}
Home team: ${clean(f.home)}
Away team: ${clean(f.away)}
Model forecast: home win ${pct(probs.home_win)}, draw ${pct(probs.draw)}, away win ${pct(probs.away_win)}.
Most likely score in the model: ${clean(f.most_likely_score, 20) || "not available"}.
Say that these are model estimates, not facts about the result. Keep the answer to two or three sentences.

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
        max_tokens: 300,
      }),
    });
    if (!upstream.ok) throw new Error(`DeepSeek returned ${upstream.status}`);
    const data = await upstream.json();
    const parsed = JSON.parse(data?.choices?.[0]?.message?.content || "{}");
    return res.status(200).json({ answer: String(parsed.answer || "No answer was returned.") });
  } catch (err) {
    console.error("match-question failed", err.message);
    return res.status(502).json({ error: "The answer could not be generated. Try again later." });
  }
};

// Seeded scoreline sampling from the Rust model's normalized joint distribution.
function simulate(scores, runs, seed) {
  if (!Number.isInteger(runs) || runs < 1 || runs > 100000)
    throw new Error("Runs must be 1–100,000");
  if (!Number.isInteger(seed) || seed < 0 || seed > 4294967295)
    throw new Error("Seed must be an unsigned 32-bit integer");
  const cells = [];
  let total = 0;
  scores.forEach((row, h) =>
    row.forEach((p, a) => {
      if (!Number.isFinite(p) || p < 0) throw new Error("Invalid probability");
      if (p > 0) {
        total += p;
        cells.push({ h, a, cumulative: total });
      }
    }),
  );
  if (Math.abs(total - 1) > 1e-6)
    throw new Error("Probabilities must sum to one");
  let state = seed >>> 0;
  const outcomes = [0, 0, 0],
    counts = new Map();
  let goals = [0, 0];
  for (let i = 0; i < runs; i++) {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t ^= t + Math.imul(t ^ (t >>> 7), 61 | t);
    const u = (((t ^ (t >>> 14)) >>> 0) / 4294967296) * total;
    const c = cells.find((c) => u < c.cumulative) || cells[cells.length - 1];
    outcomes[c.h > c.a ? 0 : c.h === c.a ? 1 : 2]++;
    goals[0] += c.h;
    goals[1] += c.a;
    const key = `${c.h}–${c.a}`;
    counts.set(key, (counts.get(key) || 0) + 1);
  }
  return {
    outcomes,
    goals: goals.map((g) => g / runs),
    scores: [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])),
  };
}
function seededRandom(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t ^= t + Math.imul(t ^ (t >>> 7), 61 | t);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// Squad-share heuristic, not a trained player model or a confirmed lineup.
// Competition season totals plus small position priors smooth low sample sizes.
function playerForecast(players, stats, scores, side) {
  const active = players.filter(
    (p) => (stats?.[p.id]?.appearances || 0) > 0 && p.position_code !== "G",
  );
  const candidates = active.length
    ? active
    : players.filter((p) => p.position_code !== "G");
  const rows = candidates.map((p) => {
    const s = stats?.[p.id],
      apps = Math.max(1, s?.appearances || 0);
    const priors =
      p.position_code === "F"
        ? [0.16, 0.09]
        : p.position_code === "M"
          ? [0.07, 0.1]
          : [0.025, 0.035];
    return {
      id: p.id,
      name: p.name,
      goalWeight:
        Math.max(0, s?.totalGoals || 0) +
        0.12 * Math.max(0, s?.shotsOnTarget || 0) +
        priors[0] * apps,
      assistWeight: Math.max(0, s?.goalAssists || 0) + priors[1] * apps,
      appearances: s?.appearances ?? null,
    };
  });
  const goalTotal = rows.reduce((s, p) => s + p.goalWeight, 0),
    assistTotal = rows.reduce((s, p) => s + p.assistWeight, 0);
  rows.forEach((p) => {
    p.goalShare = p.goalWeight / goalTotal;
    p.assistShare = p.assistWeight / assistTotal;
  });
  for (const p of rows) {
    // 30% unassisted prior; the scorer can never assist their own goal.
    const eventAssistShare =
      0.7 *
      p.assistShare *
      rows.reduce(
        (s, q) =>
          s +
          (q.id !== p.id && q.assistShare < 1
            ? q.goalShare / (1 - q.assistShare)
            : 0),
        0,
      );
    p.goalChance = 0;
    p.assistChance = 0;
    p.expectedGoals = 0;
    scores.forEach((row, h) =>
      row.forEach((prob, a) => {
        const goals = side === 0 ? h : a;
        p.goalChance += prob * (1 - (1 - p.goalShare) ** goals);
        p.assistChance += prob * (1 - (1 - eventAssistShare) ** goals);
        p.expectedGoals += prob * goals * p.goalShare;
      }),
    );
  }
  return rows.sort((a, b) => b.goalChance - a.goalChance);
}

function sampleMatch(scores, players, seed) {
  // Validate through the aggregate sampler; use its first draw as the score.
  const result = simulate(scores, 1, seed).scores[0][0].split("–").map(Number);
  const random = seededRandom(seed ^ 0xa5a5a5a5);
  function pick(rows, key) {
    const total = rows.reduce((s, p) => s + p[key], 0);
    let draw = random() * total;
    for (const p of rows) {
      draw -= p[key];
      if (draw < 0) return p;
    }
    return rows.at(-1) || null;
  }
  const events = [];
  result.forEach((goals, side) => {
    for (let i = 0; i < goals; i++) {
      const scorer = pick(players[side], "goalShare");
      const assister =
        random() < 0.7
          ? pick(
              players[side].filter((p) => p.id !== scorer?.id),
              "assistShare",
            )
          : null;
      events.push({
        minute: 1 + Math.floor(random() * 90),
        side,
        scorer: scorer ? { id: scorer.id, name: scorer.name } : null,
        assister: assister ? { id: assister.id, name: assister.name } : null,
      });
    }
  });
  events.sort((a, b) => a.minute - b.minute || a.side - b.side);
  return { score: result, events };
}
if (typeof module !== "undefined")
  module.exports = { simulate, playerForecast, sampleMatch };

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
// A synthetic broadcast story around the sampled score. These are entertainment
// events and never claimed as observed match statistics or a tactical model.
function matchStory(match,players,seed){
 const random=seededRandom(seed^0x1b873593),events=match.events.map(e=>({...e,kind:'goal'}));
 const possession=38+Math.floor(random()*25),shots=[0,0],onTarget=[0,0],saves=[0,0];
 for(let side=0;side<2;side++){
  const count=Math.max(match.score[side]+3,7+Math.floor(random()*11));
  for(let i=0;i<count-match.score[side];i++){
   const saved=random()<.5,rows=players[side];
   const shooter=rows.length?rows[Math.floor(random()*rows.length)]:null;
   events.push({minute:2+Math.floor(random()*87),side,kind:saved?'save':'wide',scorer:shooter});
  }
 }
 events.sort((a,b)=>a.minute-b.minute||(a.kind==='goal'?-1:1));
 for(const e of events){shots[e.side]++;if(e.kind==='goal'||e.kind==='save')onTarget[e.side]++;if(e.kind==='save')saves[1-e.side]++;}
 return {events,shots,onTarget,saves,possession:[possession,100-possession]};
}
if (typeof module !== "undefined")
  module.exports = { simulate, playerForecast, sampleMatch, matchStory };

// ---- Cup knockout simulator --------------------------------------------------------------
// Each team's attack and defence are its goals scored and conceded per game relative to the
// league average. Expected goals for a tie come from those rates; the score grid gives the
// chance of each result. Draws go to extra time and penalties, split by expected goals.
function poissonPmf(lambda, k) {
  let p = Math.exp(-lambda);
  for (let i = 1; i <= k; i++) p *= lambda / i;
  return p;
}
function tieProbabilities(lambdaA, lambdaB) {
  let winA = 0, draw = 0, winB = 0;
  for (let a = 0; a <= 9; a++)
    for (let b = 0; b <= 9; b++) {
      const p = poissonPmf(lambdaA, a) * poissonPmf(lambdaB, b);
      if (a > b) winA += p;
      else if (a === b) draw += p;
      else winB += p;
    }
  const total = winA + draw + winB || 1;
  const share = lambdaA / (lambdaA + lambdaB || 1);
  return { a: (winA + draw * share) / total, b: (winB + draw * (1 - share)) / total };
}
// Power-of-two sized knockout order, so the strongest seeds meet last.
function bracketOrder(size) {
  let order = [1, 2];
  while (order.length < size) {
    const n = order.length * 2;
    order = order.flatMap((s) => [s, n + 1 - s]);
  }
  return order;
}
// Runs the knockout many times. teams: [{ name, attack, defence }] in seed order (best first).
function simulateKnockout(teams, runs, seed) {
  const size = 1 << Math.floor(Math.log2(Math.max(2, teams.length)));
  const seeded = teams.slice(0, size);
  const rounds = Math.log2(size);
  // Average goals per team in a knockout tie; a team attack and defence scale it up or down.
  const BASE_GOALS = 1.35;
  const reach = seeded.map(() => new Array(rounds + 1).fill(0));
  const rng = seededRandom(seed >>> 0);
  const lambda = (x, y) => Math.max(0.2, BASE_GOALS * x.attack * y.defence);
  for (let r = 0; r < runs; r++) {
    let alive = bracketOrder(size).map((s) => s - 1);
    seeded.forEach((_, i) => reach[i][0] += 1);
    for (let round = 1; round <= rounds; round++) {
      const next = [];
      for (let i = 0; i < alive.length; i += 2) {
        const ia = alive[i], ib = alive[i + 1];
        const a = seeded[ia], b = seeded[ib];
        const probs = tieProbabilities(lambda(a, b), lambda(b, a));
        const winner = rng() < probs.a ? ia : ib;
        reach[winner][round] += 1;
        next.push(winner);
      }
      alive = next;
    }
  }
  return seeded.map((t, i) => ({
    name: t.name,
    seed: i + 1,
    reach: reach[i].map((n) => n / runs),
  }));
}

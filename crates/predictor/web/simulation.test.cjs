const { test } = require("node:test");
const assert = require("node:assert/strict");
const { simulate, playerForecast, sampleMatch } = require("./simulation.js");
test("deterministic seeds, including zero, and conserved sample counts", () => {
  const p = [
    [0.2, 0.3],
    [0.4, 0.1],
  ];
  for (const seed of [0, 1, 4294967295]) {
    const s = simulate(p, 10000, seed);
    assert.deepEqual(s, simulate(p, 10000, seed));
    assert.equal(
      s.outcomes.reduce((a, b) => a + b),
      10000,
    );
    assert.equal(
      s.scores.reduce((a, b) => a + b[1], 0),
      10000,
    );
    [0.4, 0.3, 0.3].forEach((x, i) =>
      assert.ok(Math.abs(s.outcomes[i] / 10000 - x) < 0.025),
    );
  }
});
test("impossible scores cannot be sampled", () => {
  assert.deepEqual(
    simulate(
      [
        [0, 0],
        [1, 0],
      ],
      100,
      12,
    ).outcomes,
    [100, 0, 0],
  );
});
test("invalid inputs fail explicitly", () => {
  for (const n of [0, -1, 1.5, 100001, NaN])
    assert.throws(() => simulate([[1]], n, 0));
  assert.throws(() => simulate([[0.4]], 10, 0));
  assert.throws(() => simulate([[-1, 2]], 10, 0));
  assert.throws(() => simulate([[1]], 10, -1));
});
test("player goal allocations conserve team expected goals and respect zero-goal outcomes", () => {
  const players = [
    { id: "a", name: "A", position_code: "F" },
    { id: "b", name: "B", position_code: "M" },
    { id: "g", name: "G", position_code: "G" },
  ];
  const stats = {
    a: { appearances: 5, totalGoals: 3, shotsOnTarget: 8, goalAssists: 1 },
    b: { appearances: 5, totalGoals: 1, shotsOnTarget: 3, goalAssists: 3 },
  };
  const scores = [
      [0.1, 0.1],
      [0.2, 0.2],
      [0.2, 0.2],
    ],
    p = playerForecast(players, stats, scores, 0);
  assert.equal(p.length, 2);
  assert.ok(Math.abs(p.reduce((s, p) => s + p.expectedGoals, 0) - 1.2) < 1e-9);
  assert.ok(
    p.every(
      (p) =>
        p.goalChance >= 0 &&
        p.goalChance <= 1 &&
        p.assistChance >= 0 &&
        p.assistChance <= 1,
    ),
  );
  assert.ok(
    playerForecast(players, stats, [[1]], 0).every(
      (p) => p.goalChance === 0 && p.assistChance === 0,
    ),
  );
  assert.deepEqual(playerForecast([], {}, scores, 0), []);
});
test("match replay is deterministic, conserves the sampled score and forbids self-assists", () => {
  const scores = [
      [0, 0],
      [0, 0],
      [0, 1],
    ],
    p = [
      { id: "a", name: "A", goalShare: 0.6, assistShare: 0.4 },
      { id: "b", name: "B", goalShare: 0.4, assistShare: 0.6 },
    ];
  for (let seed = 0; seed < 100; seed++) {
    const m = sampleMatch(scores, [p, p], seed);
    assert.deepEqual(m, sampleMatch(scores, [p, p], seed));
    assert.deepEqual(m.score, [2, 1]);
    assert.equal(m.events.length, 3);
    assert.equal(m.events.filter((e) => e.side === 0).length, 2);
    assert.ok(
      m.events.every(
        (e) =>
          e.minute >= 1 && e.minute <= 90 && e.scorer?.id !== e.assister?.id,
      ),
    );
    assert.ok(
      m.events.every((e, i) => !i || m.events[i - 1].minute <= e.minute),
    );
  }
  assert.deepEqual(sampleMatch([[1]], [[], []], 0), {
    score: [0, 0],
    events: [],
  });
});

// Presentation helpers for form, player estimates and a progressively revealed match.
let replayTimer = null;
let previousSeed = null;
function stopReplay() {
  replayStage?.destroy();
  replayStage=null;
  cancelAnimationFrame(replayTimer);
  replayTimer = null;
}
function formSignal(t) {
  const form = t?.form || [],
    points = form.reduce(
      (s, m) => s + (m.result === "W" ? 3 : m.result === "D" ? 1 : 0),
      0,
    );
  const kind =
    form.length < 3
      ? "limited"
      : points / form.length >= 2
        ? "good"
        : points / form.length <= 1
          ? "poor"
          : "mixed";
  return {
    kind,
    points,
    label: {
      good: "Good form",
      poor: "Poor form",
      mixed: "Mixed form",
      limited: "Limited form",
    }[kind],
    count: form.length,
  };
}
function formBadge(t) {
  const f = formSignal(t);
  return `<span class="form-status ${f.kind}" title="${f.points} points from ${f.count} available games">${f.kind === "good" ? "↗" : f.kind === "poor" ? "↘" : "→"} ${f.label}</span>`;
}
function formImpact(t) {
  const a = t?.form_adjustment;
  if (!a) return "";
  const signed = (x) => (x >= 0 ? "+" : "") + (x * 100).toFixed(0) + "%";
  // Strength deltas are exponentiated with the existing model spread of 1.15.
  return `<small class="form-impact">Recent-form input: attack ${signed(a.attack ** 1.15 - 1)} · opponent scoring ${signed(a.defence ** -1.15 - 1)}</small>`;
}
function matchPlayerForecasts(l, m) {
  return [m.home, m.away].map((name, side) => {
    const t = profile(name);
    return playerForecast(
      t.players || [],
      t.player_stats?.[l.key]?.players,
      m.prediction.scores,
      side,
    );
  });
}
function playerPicksHTML(l, m) {
  const picks = matchPlayerForecasts(l, m);
  return `<section class="player-picks"><div class="section-head"><h3>Players to make the difference</h3><small>Anytime goal / assist</small></div><div class="detail-columns">${picks
    .map(
      (rows, side) =>
        `<div><div class="table-club">${crest([m.home, m.away][side])}<strong>${esc(short([m.home, m.away][side]))}</strong></div>${
          [
            ...new Map(
              [
                ...rows.slice(0, 3),
                ...[...rows]
                  .sort((a, b) => b.assistChance - a.assistChance)
                  .slice(0, 2),
              ].map((p) => [p.id, p]),
            ).values(),
          ]
            .map((p) => {
              const t = profile([m.home, m.away][side]),
                full = t.players.find((x) => x.id === p.id);
              return `<div class="pick-row">${playerButton(t, full, `${avatar(full)}<span>${esc(p.name)}</span>`, "player-link")}<span><strong>${pct(p.goalChance)}</strong><small>Goal</small></span><span><strong>${pct(p.assistChance)}</strong><small>Assist</small></span></div>`;
            })
            .join("") || '<p class="fine-print">Player data unavailable.</p>'
        }</div>`,
    )
    .join(
      "",
    )}</div><p class="fine-print">Experimental squad-share estimates from competition goals, shots on target, appearances and position priors. Assumes usual roles and availability; lineups are unconfirmed. Assist allocation assumes 70% of goals have an assist. These probabilities are not yet calibrated against player outcomes.</p></section>`;
}
// 0 = home win, 1 = draw, 2 = away win. A predicted score counts as a pick too, so a predicted draw
// is right when the match is drawn, even if a win was the single most likely outcome.
const outcomeOf = (h, a) => (h > a ? 0 : h === a ? 1 : 2);
function outcomeBadge(m) {
  const probs = [m.prediction.home_win, m.prediction.draw, m.prediction.away_win],
    argmax = probs.indexOf(Math.max(...probs)),
    scoreline = (m.prediction.predicted_score||m.prediction.most_likely_score) ? outcomeOf(...(m.prediction.predicted_score||m.prediction.most_likely_score)) : argmax,
    actual = outcomeOf(m.actual[0], m.actual[1]),
    correct = actual === argmax || actual === scoreline,
    picked = actual === scoreline ? scoreline : argmax;
  return `<span class="outcome-badge ${correct ? "correct" : "incorrect"}">${correct ? "✓ Correct" : "✕ Incorrect"}</span><small class="outcome-pick">Picked ${["home win", "draw", "away win"][picked]}</small>`;
}

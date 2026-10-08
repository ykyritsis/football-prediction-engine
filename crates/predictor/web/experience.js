// Presentation helpers for form, player estimates and a progressively revealed match.
let replayTimer = null;
let previousSeed = null;
function stopReplay() {
  clearInterval(replayTimer);
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
function simulationControlsHTML() {
  return `<section class="sim-section"><div class="section-head"><h3>Play out the match</h3><span class="live-label">ONE POSSIBLE STORY</span></div><p class="fine-print">Watch the clock, goals and contributors unfold. Each run draws a new outcome from the forecast.</p><div class="sim-controls"><label class="field">Playback<select id="playback"><option value="12000">Quick · 12 seconds</option><option value="24000">Matchday · 24 seconds</option><option value="0">Instant result</option></select></label><label class="seed-option"><input type="checkbox" id="fixedSeed"> Use a repeatable seed</label><label class="field" id="seedField" hidden>Seed<input type="number" id="seed" min="0" max="4294967295" step="1" value="42"></label><button class="primary" id="runSimulation">▶ Play match</button></div><div id="simulationOutput"></div><p class="fine-print" style="margin-top:13px">Simulated events, not a live commentary feed. Goal timings and player contributions are illustrative. Team results follow the forecast distribution.</p></section>`;
}
function bindSimulation(m, l) {
  stopReplay();
  $("fixedSeed").onchange = () => {
    $("seedField").hidden = !$("fixedSeed").checked;
  };
  $("runSimulation").onclick = () => {
    stopReplay();
    try {
      let seed;
      if ($("fixedSeed").checked) {
        if (!$("seed").value.trim()) throw new Error("Enter a seed.");
        seed = Number($("seed").value);
      } else {
        seed = crypto.getRandomValues(new Uint32Array(1))[0];
        if (seed === previousSeed) seed = (seed + 1) >>> 0;
      }
      const forecast = matchPlayerForecasts(l, m),
        match = sampleMatch(m.prediction.scores, forecast, seed),
        aggregate = simulate(m.prediction.scores, 10000, seed);
      previousSeed = seed;
      const duration = matchMedia("(prefers-reduced-motion: reduce)").matches
        ? 0
        : Number($("playback").value);
      const names = [m.home, m.away];
      $("simulationOutput").innerHTML =
        `<div class="match-theatre"><div class="theatre-top"><span class="live-label" id="replayPhase">SIMULATION · KICK-OFF</span><span id="replayClock">0′</span></div><div class="replay-scoreboard"><div>${crest(m.home)}<strong>${esc(short(m.home))}</strong></div><div class="replay-score" id="replayScore">0 <span>–</span> 0</div><div>${crest(m.away)}<strong>${esc(short(m.away))}</strong></div></div><div class="mini-pitch" aria-hidden="true"><div class="pitch-circle"></div><div class="pitch-box left"></div><div class="pitch-box right"></div><span class="match-ball" id="matchBall">●</span><span class="pitch-caption" id="pitchCaption">The teams are ready.</span></div><div class="match-progress"><span id="replayProgress"></span></div><div class="replay-controls"><button class="text-button" id="pauseReplay">Pause</button><button class="text-button" id="skipReplay">Reveal full-time →</button><span>Seed <strong id="usedSeed">${seed}</strong></span></div><div class="replay-events" id="replayEvents" role="log" aria-live="polite"><div class="replay-event neutral"><span>0′</span><div><strong>Kick-off</strong><small>A fresh simulation is underway.</small></div></div></div><div id="replaySummary"></div></div>`;
      let minute = 0,
        eventIndex = 0,
        home = 0,
        away = 0,
        paused = false,
        halfTime = false,
        finished = false;
      function eventHTML(e) {
        return `<div class="replay-event goal"><span>${e.minute}′</span><div><strong>⚽ ${esc(e.scorer?.name || short(names[e.side]) + " scorer")}</strong><small>${esc(short(names[e.side]))}${e.assister ? " · Assist: " + esc(e.assister.name) : " · No assist assigned"}</small></div><b>${home}–${away}</b></div>`;
      }
      function update(target) {
        if (!$("replayScore")) {
          stopReplay();
          return;
        }
        minute = Math.min(90, target);
        while (
          eventIndex < match.events.length &&
          match.events[eventIndex].minute <= minute
        ) {
          const e = match.events[eventIndex++];
          e.side === 0 ? home++ : away++;
          $("replayEvents").insertAdjacentHTML("beforeend", eventHTML(e));
          $("replayScore").textContent = `${home} – ${away}`;
          $("replayScore").classList.remove("goal-pop");
          void $("replayScore").offsetWidth;
          $("replayScore").classList.add("goal-pop");
        }
        if (minute >= 45 && !halfTime) {
          halfTime = true;
          $("replayPhase").textContent = "SIMULATION · SECOND HALF";
        }
        $("replayClock").textContent = minute + "′";
        $("replayProgress").style.width = (minute / 90) * 100 + "%";
        $("matchBall").style.left = 18 + ((minute * 17) % 65) + "%";
        $("matchBall").style.top = 28 + ((minute * 11) % 42) + "%";
        $("pitchCaption").textContent =
          minute < 45
            ? "First half · simulated play"
            : "Second half · simulated play";
        if (minute === 90 && !finished) {
          finished = true;
          stopReplay();
          $("replayPhase").textContent = "SIMULATION · FULL TIME";
          $("pitchCaption").textContent = "Full-time";
          $("pauseReplay").hidden = true;
          $("skipReplay").hidden = true;
          $("replayEvents").insertAdjacentHTML(
            "beforeend",
            `<div class="replay-event neutral"><span>FT</span><div><strong>${home === away ? "Honours even" : esc(short(names[home > away ? 0 : 1])) + " win"}</strong><small>One simulated match · ${home}–${away}</small></div></div>`,
          );
          $("replaySummary").innerHTML =
            `<details class="ensemble"><summary>Compare with 10,000 simulations</summary>${probabilities({ home_win: aggregate.outcomes[0] / 10000, draw: aggregate.outcomes[1] / 10000, away_win: aggregate.outcomes[2] / 10000 })}<p>Average goals: <strong>${aggregate.goals.map((x) => x.toFixed(2)).join(" – ")}</strong>. The most likely single score is ${score(m.prediction.most_likely_score)}; it is not the average.</p></details>`;
          $("runSimulation").textContent = "↻ Play another match";
        }
      }
      $("pauseReplay").onclick = () => {
        paused = !paused;
        $("pauseReplay").textContent = paused ? "Resume" : "Pause";
      };
      $("skipReplay").onclick = () => update(90);
      document
        .querySelector(".match-theatre")
        ?.scrollIntoView({
          behavior: duration === 0 ? "instant" : "smooth",
          block: "start",
        });
      if (duration === 0) update(90);
      else
        replayTimer = setInterval(() => {
          if (!paused) update(minute + 1);
        }, duration / 90);
    } catch (e) {
      $("simulationOutput").textContent = e.message;
    }
  };
}
function outcomeBadge(m) {
  const probs = [
      m.prediction.home_win,
      m.prediction.draw,
      m.prediction.away_win,
    ],
    predicted = probs.indexOf(Math.max(...probs)),
    actual =
      m.actual[0] > m.actual[1] ? 0 : m.actual[0] === m.actual[1] ? 1 : 2,
    correct = predicted === actual;
  return `<span class="outcome-badge ${correct ? "correct" : "incorrect"}">${correct ? "✓ Correct" : "✕ Incorrect"}</span><small class="outcome-pick">Picked ${["home win", "draw", "away win"][predicted]}</small>`;
}

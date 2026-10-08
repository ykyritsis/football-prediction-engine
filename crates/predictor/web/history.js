let selectedSeason = null;
function tmStore() {
  return data.transfermarkt || (data.transfermarkt = {});
}
const seasonYear = (date) =>
  Number(date.slice(0, 4)) - (date.slice(5, 7) < "07" ? 1 : 0);
const seasonName = (year) => year + "/" + String(Number(year) + 1).slice(-2);
const money = (n) =>
  Number.isFinite(n)
    ? new Intl.NumberFormat("en-GB", {
        style: "currency",
        currency: "EUR",
        notation: "compact",
        maximumFractionDigits: 1,
      }).format(n)
    : "—";
async function tmFetch(kind, params) {
  if (data.saved_only)
    throw new Error(
      "This edition includes saved history. More profiles will appear in future data updates.",
    );
  if (location.protocol === "file:")
    throw new Error(
      "Open the local server to load more history: node tools/serve_dashboard.cjs dashboard.html. Previously saved history remains available offline.",
    );
  const r = await fetch(
    "/api/transfermarkt/" + kind + "?" + new URLSearchParams(params),
  );
  if (!r.headers.get("content-type")?.includes("application/json"))
    throw new Error(
      "Historical API needs the Touchline server. Run node tools/serve_dashboard.cjs dashboard.html.",
    );
  const d = await r.json();
  if (!r.ok) throw new Error(d.error || "History is temporarily unavailable.");
  return d;
}
function tmCaption(date) {
  return `<p class="table-caption">Transfermarkt via <a class="text-button" href="https://github.com/felipeall/transfermarkt-api" target="_blank" rel="noopener">felipeall/transfermarkt-api</a> · source fetched ${esc(date?.slice(0, 10) || "date unavailable")}. Historical context is separate from the prediction fit.</p>`;
}
function trophiesHTML(d) {
  return `<div class="trophy-grid">${
    (d?.achievements || [])
      .filter((a) => a.title?.trim())
      .map(
        (a) =>
          `<div class="trophy-card"><span class="trophy-symbol">★</span><strong>${esc(a.count)}× ${esc(a.title)}</strong><small>${(a.details || []).map((x) => esc(x.season?.name || x.season?.id || "")).join(" · ")}</small></div>`,
      )
      .join("") ||
    '<p class="fine-print">No achievements returned by the provider.</p>'
  }</div>`;
}
function clubSeasonSummary(t) {
  const l =
      activeLeagues().find((l) => l.teams.some((x) => x.name === t.name)) ||
      selectedLeague(),
    groups = new Map();
  for (const m of l.archive || []) {
    if (m.home !== t.name && m.away !== t.name) continue;
    const year = seasonYear(m.date);
    if (!groups.has(year))
      groups.set(year, { played: 0, w: 0, d: 0, l: 0, gf: 0, ga: 0 });
    const s = groups.get(year),
      [h, a] = m.home === t.name ? m.score : [m.score[1], m.score[0]];
    s.played++;
    s.gf += h;
    s.ga += a;
    h > a ? s.w++ : h === a ? s.d++ : s.l++;
  }
  return `<div class="table-wrap"><table><thead><tr><th>Season · ${esc(l.name)}</th><th>Played</th><th>W / D / L</th><th>Goals</th><th>Points</th></tr></thead><tbody>${[
    ...groups,
  ]
    .sort((a, b) => b[0] - a[0])
    .map(
      ([y, s]) =>
        `<tr><td><strong>${seasonName(y)}</strong></td><td>${s.played}</td><td>${s.w} / ${s.d} / ${s.l}</td><td>${s.gf}–${s.ga}</td><td>${s.w * 3 + s.d}</td></tr>`,
    )
    .join(
      "",
    )}</tbody></table></div><p class="table-caption">Available match archive, including partial seasons. Points are calculated from results, not adjusted for league deductions.</p>`;
}
function clubHistoryDetails(d) {
  const p = d.profile;
  return `<div class="stats-strip">${statBlock(p.coach?.name, "Manager · current profile")}${statBlock(p.stadiumSeats?.toLocaleString(), "Stadium capacity")}${statBlock(money(p.currentMarketValue), "Squad market value")}</div>${p.historicalNames?.length ? '<p class="fine-print">Earlier names: ' + p.historicalNames.map((n) => esc(n.name) + " (" + esc(n.seasonId) + ")").join(" · ") + "</p>" : ""}<div class="section-head"><h3>Honours through the years</h3></div>${trophiesHTML(d.achievements)}${tmCaption(p.updatedAt)}`;
}
function clubHistoryHTML(t) {
  const d = tmStore().clubs?.[t.name];
  return `<section class="history-section"><div class="section-head"><h2>Club history</h2><small>Season by season</small></div>${clubSeasonSummary(t)}<div id="clubTm">${d ? clubHistoryDetails(d) : data.saved_only ? '<p class="fine-print">Honours and heritage are not included for this club in the current edition. The available season results are shown above.</p>' : `<button class="secondary" data-tm-club="${esc(t.name)}">Load honours & club heritage ↗</button><p class="fine-print">Historical names, trophies, manager and valuation from Transfermarkt.</p>`}</div></section>`;
}
async function loadClubHistory(name) {
  const target = $("clubTm");
  if (!target) return;
  target.innerHTML =
    '<p class="loading-state">Loading club history from Transfermarkt…</p>';
  try {
    const d = await tmFetch("club", { name });
    (tmStore().clubs ??= {})[name] = d;
    if (target.isConnected) target.innerHTML = clubHistoryDetails(d);
  } catch (e) {
    if (target.isConnected)
      target.innerHTML = `<p class="notice">${esc(e.message)}</p><button class="text-button" data-tm-club="${esc(name)}">Try again</button>`;
  }
}
function marketChart(rows) {
  const pts = rows.filter((x) => Number.isFinite(x.marketValue)).slice(-24);
  if (pts.length < 2) return "";
  const max = Math.max(...pts.map((x) => x.marketValue), 1),
    coordinates = pts
      .map(
        (p, i) =>
          `${10 + (i * 580) / (pts.length - 1)},${100 - (p.marketValue / max) * 85}`,
      )
      .join(" ");
  return `<div class="market-chart"><div class="section-head"><h3>Market value history</h3><small>EUR · peak in chart ${money(max)}</small></div><svg viewBox="0 0 600 115" role="img" aria-label="Market value from ${esc(pts[0].date)} to ${esc(pts.at(-1).date)}"><path d="M10 100H590" fill="none" stroke="#dae2e7"/><polyline points="${coordinates}" fill="none" stroke="#2f678d" stroke-width="3"/>${pts.map((p, i) => `<circle cx="${10 + (i * 580) / (pts.length - 1)}" cy="${100 - (p.marketValue / max) * 85}" r="3" fill="#2f678d"><title>${esc(p.date)} · ${money(p.marketValue)}</title></circle>`).join("")}</svg><div class="chart-range"><span>${esc(pts[0].date)}</span><span>${esc(pts.at(-1).date)}</span></div></div>`;
}
function playerHistoryDetails(d) {
  const stats = [...(d.stats?.stats || [])].sort((a, b) =>
      String(b.seasonId).localeCompare(String(a.seasonId)),
    ),
    transfers = d.transfers?.transfers || [];
  return `<div class="stats-strip">${statBlock(money(d.profile.marketValue), "Market value")}${statBlock(d.profile.foot, "Preferred foot")}${statBlock(d.profile.club?.contractExpires, "Contract through")}</div>${marketChart([...(d.market?.marketValueHistory || [])].sort((a, b) => a.date.localeCompare(b.date)))}<details class="history-disclosure" open><summary>Career by season & competition</summary><div class="table-wrap"><table><thead><tr><th>Season</th><th>Competition</th><th>Apps</th><th>Goals</th><th>Assists</th><th>Minutes</th></tr></thead><tbody>${stats.map((s) => `<tr><td>${esc(seasonName(s.seasonId))}</td><td>${esc(s.competitionName || s.competitionId)}</td><td>${val(s.appearances)}</td><td>${val(s.goals)}</td><td>${val(s.assists)}</td><td>${val(s.minutesPlayed)}</td></tr>`).join("")}</tbody></table></div></details><details class="history-disclosure"><summary>Transfers · ${transfers.length}</summary><div class="transfer-list">${transfers.map((t) => `<div><small>${esc(t.date || t.season || "Date unavailable")}</small><strong>${esc(t.clubFrom.name)} → ${esc(t.clubTo.name)}</strong><span>${esc(t.transferType || "Transfer")} · ${money(t.fee)}</span></div>`).join("") || "<p>No transfers returned.</p>"}</div></details><details class="history-disclosure"><summary>Honours & achievements</summary>${trophiesHTML(d.achievements)}</details>${tmCaption(d.stats?.updatedAt)}<a class="text-button" href="${url(d.profile.url)}" target="_blank" rel="noopener">Transfermarkt profile ↗</a>`;
}
function playerHistoryShell(t, p) {
  const key = t.id + ":" + p.id,
    d = tmStore().players?.[key];
  return `<section class="history-section"><div class="section-head"><h2>Career history</h2><small>Transfermarkt</small></div><div id="playerTm">${d ? playerHistoryDetails(d) : data.saved_only ? '<p class="fine-print">Career history is not included for this player in the current edition. Available competition statistics are shown above.</p>' : `<button class="secondary" data-tm-player="${esc(key)}">Explore career, transfers & honours ↗</button><p class="fine-print">Names and birth dates are verified before histories are linked. Loading an uncached profile can take a few seconds.</p>`}</div></section>`;
}
async function loadPlayerHistory(t, p) {
  const target = $("playerTm");
  if (!target) return;
  target.innerHTML =
    '<p class="loading-state">Matching identity and loading career history…</p>';
  try {
    const d = await tmFetch("player", { club: t.name, id: p.id });
    (tmStore().players ??= {})[t.id + ":" + p.id] = d;
    if (target.isConnected) target.innerHTML = playerHistoryDetails(d);
  } catch (e) {
    if (target.isConnected)
      target.innerHTML = `<p class="notice">${esc(e.message)}</p><button class="text-button" data-tm-player="${esc(t.id + ":" + p.id)}">Try again</button>`;
  }
}
function tablesHTML(d) {
  return (
    (d.tables || [])
      .map(
        (g) =>
          `<h3 class="table-group-title">${esc(g.name || d.name)}</h3><div class="table-wrap"><table><thead><tr><th>#</th><th>Club</th><th>P</th><th>W</th><th>D</th><th>L</th><th>GF–GA</th><th>Pts</th></tr></thead><tbody>${(g.rows || []).map((r) => `<tr><td>${val(r.position)}</td><td><strong>${esc(r.clubName)}</strong>${r.zone ? '<small class="table-zone">' + esc(r.zone) + "</small>" : ""}</td><td>${val(r.matches)}</td><td>${val(r.wins)}</td><td>${val(r.draws)}</td><td>${val(r.losses)}</td><td>${val(r.goalsFor)}–${val(r.goalsAgainst)}</td><td><strong>${val(r.points)}</strong></td></tr>`).join("")}</tbody></table></div>`,
      )
      .join("") + tmCaption(d.updatedAt)
  );
}
function renderSeasons() {
  if (state.competition === "all") {
    state.competition = "premier_league";
    renderNavigation();
  }
  const l = selectedLeague(),
    c = tmStore().competitions?.[l.key],
    years = [
      ...new Set([
        ...(l.archive || []).map((m) => seasonYear(m.date)),
        ...(data.saved_only
          ? Object.keys(tmStore().tables || {})
              .filter((k) => k.startsWith(l.key + ":"))
              .map((k) => Number(k.split(":")[1]))
          : (c?.seasons || []).map((s) => Number(s.id))),
      ]),
    ]
      .filter((y) => y >= 1900 && y <= seasonYear(data.as_of))
      .sort((a, b) => b - a);
  if (!years.includes(selectedSeason))
    selectedSeason = years.find((y) => y < seasonYear(data.as_of)) || years[0];
  heading(
    "SEASON ARCHIVE",
    l.name,
    "Explore earlier campaigns, league tables and cup results.",
  );
  $("viewTools").innerHTML =
    `<label class="field">Season<select id="archiveSeason">${years.map((y) => `<option value="${y}" ${y === selectedSeason ? "selected" : ""}>${seasonName(y)}</option>`).join("")}</select></label>`;
  const rows = (l.archive || [])
      .filter(
        (m) =>
          seasonYear(m.date) === selectedSeason &&
          matchesQuery(short(m.home) + " " + short(m.away)),
      )
      .sort((a, b) => b.date.localeCompare(a.date)),
    cached = tmStore().tables?.[l.key + ":" + selectedSeason];
  $("content").innerHTML =
    `<section class="panel panel-pad"><div class="section-head"><h2>${seasonName(selectedSeason)} standings</h2><small>Transfermarkt</small></div><p class="fine-print">For cups, tables represent the group or league phase; knockout champions are not inferred from first place.</p><div id="seasonTable">${cached ? tablesHTML(cached) : data.saved_only ? '<p class="notice">Standings for this season are not included in the current edition. Available match results are listed below.</p>' : '<p class="loading-state">Loading historical standings…</p>'}</div></section><div class="section-head"><h2>Results archive</h2><small>${rows.length} available matches</small></div><div class="table-wrap"><table><thead><tr><th>Date</th><th>Home</th><th>Score</th><th>Away</th></tr></thead><tbody>${rows
      .slice(0, state.limit)
      .map(
        (m) =>
          `<tr><td>${esc(m.date)}</td><td>${esc(short(m.home))}</td><td><strong>${score(m.score)}</strong></td><td>${esc(short(m.away))}</td></tr>`,
      )
      .join(
        "",
      )}</tbody></table></div>${rows.length > state.limit ? '<button class="more" data-more>Show more results</button>' : ""}${!rows.length ? '<p class="notice">Match-level results for this season are outside the bundled archive. Transfermarkt standings may still be available above.</p>' : ""}<p class="table-caption">Archived results are factual scores, not reconstructed predictions. See Model results for evaluated pre-match estimates.</p>`;
  $("archiveSeason").onchange = () => {
    selectedSeason = Number($("archiveSeason").value);
    state.limit = 30;
    renderSeasons();
  };
  if (!cached && !data.saved_only) {
    const target = $("seasonTable"),
      year = selectedSeason;
    tmFetch("table", { competition: l.key, season: year })
      .then((d) => {
        (tmStore().tables ??= {})[l.key + ":" + year] = d;
        if (target.isConnected) target.innerHTML = tablesHTML(d);
      })
      .catch((e) => {
        if (target.isConnected)
          target.innerHTML = `<p class="notice">${esc(e.message)}</p>`;
      });
  }
}

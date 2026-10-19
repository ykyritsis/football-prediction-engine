"use strict";
// All feed strings are escaped before insertion; image and source URLs require HTTPS.
const data = JSON.parse(document.getElementById("data").textContent),
  catalog = data.catalog || { teams: {}, competitions: [], fixtures: {} };
const $ = (id) => document.getElementById(id),
  esc = (s) =>
    String(s ?? "").replace(
      /[&<>"']/g,
      (c) =>
        ({
          "&": "&amp;",
          "<": "&lt;",
          ">": "&gt;",
          '"': "&quot;",
          "'": "&#39;",
        })[c],
    );
const url = (s) => {
  try {
    const u = new URL(s);
    return u.protocol === "https:" ? esc(u.href) : "";
  } catch {
    return "";
  }
};
const pct = (x) => Math.round(x * 100) + "%",
  score = (s) => s.join("–"),
  val = (n) => (n === null || n === undefined ? "—" : esc(n));
const cleanName = (s) => s.replace(/^ESPN:\d+:/, ""),
  short = (n) =>
    catalog.teams[n]?.short_name || cleanName(n).replace(/ FC$| AFC$/, ""),
  initials = (n) =>
    cleanName(n)
      .split(" ")
      .filter(Boolean)
      .slice(0, 2)
      .map((s) => s[0])
      .join("");
const dateLabel = (s) =>
    new Date(s + "T12:00:00Z").toLocaleDateString("en-GB", {
      day: "numeric",
      month: "short",
      timeZone: "UTC",
    }),
  color = (t) =>
    /^[0-9a-f]{6}$/i.test(t?.color || "") ? "#" + t.color : "#101d35";
const state = {
  view: "matches",
  competition: "premier_league",
  query: "",
  days: 14,
  limit: 30,
  club: null,
  sort: "totalGoals",
  home: "",
  away: "",
  homeShape: "4-3-3",
  awayShape: "4-3-3",
};
const matchLookup = new Map(),
  playerLookup = new Map();
for (const t of Object.values(catalog.teams || {}))
  for (const p of t.players || [])
    playerLookup.set(t.id + ":" + p.id, { t, p });
const compInfo = (k) => catalog.competitions.find((c) => c.key === k),
  league = (k) => data.leagues.find((l) => l.key === k),
  activeLeagues = () =>
    data.leagues.filter(
      (l) => state.competition === "all" || l.key === state.competition,
    ),
  selectedLeague = () => league(state.competition) || data.leagues[0];
const modelTeam = (n, l = selectedLeague()) =>
    l?.teams.find((t) => t.name === n),
  matchesQuery = (s) =>
    s.toLocaleLowerCase().includes(state.query.toLocaleLowerCase()),
  profile = (n) =>
    catalog.teams[n] || {
      name: n,
      display_name: cleanName(n),
      short_name: short(n),
      players: [],
      competitions: [],
      player_stats: {},
    };
const photoFallbacks=new Map();
for(const t of Object.values(catalog.teams))for(const p of t.players||[])if(p.portrait?.url&&p.alternative_portrait?.url&&p.portrait.url!==p.alternative_portrait.url)photoFallbacks.set(p.portrait.url,p.alternative_portrait.url);
function image(src, alt, cls = "") {
  return url(src)
    ? `<img src="${url(src)}" alt="${esc(alt)}" class="${cls}" ${photoFallbacks.has(src)?`data-fallback-src="${url(photoFallbacks.get(src))}"`:""} loading="lazy" referrerpolicy="no-referrer">`
    : "";
}
function crest(n) {
  return (
    image(profile(n).logo, short(n) + " crest") ||
    `<span class="crest-placeholder" aria-label="${esc(short(n))}">${esc(initials(n))}</span>`
  );
}
function competitionLogo(k) {
  return image(compInfo(k)?.logo, league(k)?.name || "Competition");
}
function formHTML(t) {
  return `${formBadge(t)}<span class="form">${(t?.form || []).map((m) => `<span class="${m.result}" tabindex="0" title="${esc(m.date + " · " + m.venue + " v " + short(m.opponent) + " · " + score(m.goals))}" aria-label="${esc(m.result + " " + m.date + " " + short(m.opponent) + " " + score(m.goals))}">${m.result}</span>`).join("") || '<small class="muted">No results</small>'}</span>`;
}
function statBlock(n, label) {
  return `<div class="stat-block"><strong>${val(n)}</strong><span>${esc(label)}</span></div>`;
}
function compForTeam(t) {
  return state.competition !== "all" &&
    t.competitions.includes(state.competition)
    ? state.competition
    : t.competitions[0];
}
function statsFor(t, p) {
  return t.player_stats?.[compForTeam(t)]?.players?.[p.id] ?? null;
}
function playerRows(club = null) {
  return Object.values(catalog.teams)
    .filter(
      (t) =>
        (state.competition === "all" ||
          t.competitions.includes(state.competition)) &&
        (!club || t.name === club),
    )
    .flatMap((t) => (t.players || []).map((p) => ({ t, p, s: statsFor(t, p) })))
    .filter(({ t, p }) =>
      matchesQuery(p.name + " " + t.name + " " + t.display_name),
    )
    .sort(
      (a, b) =>
        (b.s?.[state.sort] ?? -1) - (a.s?.[state.sort] ?? -1) ||
        a.p.name.localeCompare(b.p.name),
    );
}
function avatar(p) {
  return (
    image(portraitFor(p)?.url, p.name, "avatar") ||
    `<span class="avatar portrait-fallback" style="font-size:11px;letter-spacing:0">${esc(initials(p.name))}</span>`
  );
}

const localZone = Intl.DateTimeFormat().resolvedOptions().timeZone;
function matchTime(l,m){return kickoffInfo(fixtureMeta(l,m),m.date,localZone);}
function portraitFor(p){return p.portrait||p.alternative_portrait;}
function playerButton(t, p, body, cls = "") {
  return `<button class="${cls}" data-player="${esc(t.id + ":" + p.id)}">${body}</button>`;
}
function playerCard({ t, p, s }) {
  return playerButton(
    t,
    p,
    `<div class="player-photo">${image(p.portrait?.url, p.name) || `<div class="portrait-fallback">${esc(initials(p.name))}</div>`}<div class="player-club">${crest(t.name)}</div></div><div class="player-card-copy"><small>${esc(p.position || "Player")} · ${esc(short(t.name))}</small><h3>${esc(p.name)}</h3><div class="player-card-stats">${statBlock(s?.totalGoals, "Goals")}${statBlock(s?.goalAssists, "Assists")}${statBlock(s?.appearances, "Apps")}</div></div>`,
    "player-card",
  );
}
function playerTable(rows) {
  return `<div class="mobile-player-list">${squadCardsHTML(rows)}</div><div class="table-wrap desktop-player-table"><table><thead><tr><th>Player</th><th>Club / competition</th><th>Position</th><th class="number">Apps</th><th class="number">Goals</th><th class="number">Assists</th><th class="number">Shots</th><th class="number">On target</th><th class="number">YC</th></tr></thead><tbody>${rows.map(({ t, p, s }) => `<tr><td>${playerButton(t, p, `${avatar(p)}<span><strong>${esc(p.name)}</strong><small>${p.number ? "#" + esc(p.number) + " · " : ""}${esc(p.nationality || "Nationality unavailable")}</small></span>`, "player-link")}</td><td><span class="table-club">${crest(t.name)}${esc(short(t.name))}</span><small class="muted">${esc(league(compForTeam(t))?.name || "")}</small></td><td>${esc(p.position_code || "—")}</td>${["appearances", "totalGoals", "goalAssists", "totalShots", "shotsOnTarget", "yellowCards"].map((k) => `<td class="number">${val(s?.[k])}</td>`).join("")}</tr>`).join("")}</tbody></table></div>`;
}
function heading(e, t, s) {
  $("eyebrow").textContent = e;
  $("title").textContent = t;
  $("subtitle").textContent = s;
}
function renderNavigation() {
  document.querySelector(".search").hidden = ["about", "simulator"].includes(
    state.view,
  );
  $("snapshot").textContent =
    "Forecast snapshot · " +
    dateLabel(data.as_of) +
    " " +
    data.as_of.slice(0, 4);
  const y =
    Number(data.as_of.slice(0, 4)) - (data.as_of.slice(5, 7) < "07" ? 1 : 0);
  document.querySelector(".season-label").innerHTML =
    y + "/" + String(y + 1).slice(-2) + "<span>SEASON</span>";
  $("competitions").innerHTML =
    `<button class="competition ${state.competition === "all" ? "active" : ""}" data-comp="all"><span class="all-symbol">≡</span>All competitions</button>` +
    data.leagues
      .map(
        (l, i) =>
          (i === 0 ? '<div class="sidebar-section">DOMESTIC LEAGUES</div>' : i === 10 ? '<div class="sidebar-section">CLUB TOURNAMENTS</div>' : i === 14 ? '<div class="sidebar-section">INTERNATIONAL FOOTBALL</div>' : "") +
          `<button class="competition ${state.competition === l.key ? "active" : ""}" data-comp="${l.key}">${competitionLogo(l.key)}<span>${esc(l.name)}</span></button>`,
      )
      .join("");
  document.querySelectorAll(".topnav [data-view], .mobile-nav [data-view]").forEach((b) => {
    b.classList.toggle("active", b.dataset.view === state.view);
    b.setAttribute(
      "aria-current",
      b.dataset.view === state.view ? "page" : "false",
    );
  });
  $('moreNavigation').classList.toggle('active',!['matches','clubs','players','simulator'].includes(state.view));
  $('mobileCompetition').innerHTML='<option value="all">All competitions</option>'+['domestic','cup','international'].map(kind=>`<optgroup label="${({domestic:'Domestic leagues',cup:'Club tournaments',international:'International football'})[kind]}">${data.leagues.filter(l=>compInfo(l.key)?.kind===kind).map(l=>`<option value="${l.key}" ${l.key===state.competition?'selected':''}>${esc(l.name)}</option>`).join('')}</optgroup>`).join('');
  $('mobileCompetition').value=state.competition;
  $('compChips').innerHTML=`<button class="comp-chip ${state.competition==='all'?'active':''}" data-comp="all"><span class="all-symbol">≡</span>All</button>`+data.leagues.map(l=>`<button class="comp-chip ${state.competition===l.key?'active':''}" data-comp="${l.key}">${competitionLogo(l.key)}<span>${esc(l.name)}</span></button>`).join('');
  const activeChip=$('compChips').querySelector('.active');if(activeChip)$('compChips').scrollLeft=activeChip.offsetLeft-$('compChips').clientWidth/2+activeChip.clientWidth/2;
  $('heroStats').innerHTML='';
  const inProfile=state.view==='clubs'&&state.club;
  document.querySelector('main').classList.toggle('showing-profile',!!inProfile);
  document.querySelector('.search').hidden=['about','simulator'].includes(state.view)||!!inProfile;
  $('clubStrip').classList.toggle('profile-hidden',!!inProfile);
  const names = [
    ...new Set(activeLeagues().flatMap((l) => l.teams.map((t) => t.name))),
  ];
  $("clubStrip").innerHTML = names
    .slice(0, state.competition === "all" ? 28 : 100)
    .map(
      (n) =>
        `<button data-club="${esc(n)}" title="${esc(short(n))}" aria-label="View ${esc(short(n))}">${crest(n)}</button>`,
    )
    .join("");
  $("clubStrip").hidden = ["about", "history", "seasons"].includes(state.view);
  const issues = activeLeagues().filter(
    (l) =>
      !l.latest_result ||
      Date.parse(data.as_of) - Date.parse(l.latest_result) > 10 * 86400000 ||
      l.overdue,
  );
  $("freshness").innerHTML =
    ["matches", "simulator", "history"].includes(state.view) && issues.length
      ? `<div class="freshness"><details><summary>Data coverage: ${issues.length === 1 ? esc(issues[0].name) + " results through " + esc(issues[0].latest_result || "unavailable") : issues.length + " competitions have older or incomplete results"}. Predictions use the available snapshot.</summary><ul>${issues.map((l) => `<li>${esc(l.name)}: latest result ${esc(l.latest_result || "unavailable")}${l.overdue ? " · " + l.overdue + " earlier fixtures without results" : ""}.</li>`).join("")}</ul></details></div>`
      : "";
}
let renderRevision=0;
const teamLoads=new Map();
const leagueLoads=new Map();
async function loadLeague(l){
 if(!l?.lazy)return;if(leagueLoads.has(l.key))return leagueLoads.get(l.key);
 const pending=(async()=>{const r=await fetch(new URL(l.asset,document.baseURI));if(!r.ok)throw Error('Competition data is temporarily unavailable.');Object.assign(l,await r.json(),{lazy:false});})();leagueLoads.set(l.key,pending);try{await pending;}finally{leagueLoads.delete(l.key);}
}
async function loadTeam(t){
 if(!t?.lazy)return;
 if(teamLoads.has(t.name))return teamLoads.get(t.name);
 const loading=(async()=>{const response=await fetch(new URL(t.asset,document.baseURI));if(!response.ok)throw Error('Team details are temporarily unavailable.');const detail=await response.json();Object.assign(t,detail,{lazy:false});if(detail.heritage)(tmStore().clubs??={})[t.name]=detail.heritage;for(const p of t.players||[]){playerLookup.set(t.id+':'+p.id,{t,p});if(p.portrait?.url&&p.alternative_portrait?.url&&p.portrait.url!==p.alternative_portrait.url)photoFallbacks.set(p.portrait.url,p.alternative_portrait.url);}})();
 teamLoads.set(t.name,loading);try{await loading;}finally{teamLoads.delete(t.name);}
}
async function loadTeams(teams){let index=0;await Promise.all(Array.from({length:Math.min(6,teams.length)},async()=>{while(index<teams.length)await loadTeam(teams[index++]);}));}
async function render() {
  const revision=++renderRevision;
  if(state.competition==='all'&&['simulator','seasons'].includes(state.view))state.competition='premier_league';
  stopReplay();
  renderNavigation();
  $("viewTools").innerHTML = "";
  const teamViews=state.view==='players'||state.view==='simulator'||(state.view==='matches'&&state.competition!=='all');
  const teamNames=state.view==='clubs'&&state.club?[state.club]:teamViews?[...new Set(activeLeagues().flatMap(l=>l.teams.map(t=>t.name)))]:[];
  const neededTeams=teamNames.map(n=>catalog.teams[n]).filter(t=>t?.lazy);
  if(neededTeams.length){$('content').innerHTML='<div class="loading-state">Opening team profiles…</div>';try{await loadTeams(neededTeams);}catch(e){if(revision===renderRevision)$('content').innerHTML=`<div class="notice">${esc(e.message)} <button data-retry>Try again</button></div>`;return;}}
  if (["simulator","history","seasons","clubs"].includes(state.view)) {
    const needed=activeLeagues().filter(l=>l.lazy);
    if(needed.length){$("content").innerHTML='<div class="loading-state">Loading competition details…</div>';try{await Promise.all(needed.map(loadLeague));}catch(e){if(revision===renderRevision)$("content").innerHTML=`<div class="notice">${esc(e.message)} <button data-retry>Try again</button></div>`;return;}}
  }
  if(revision!==renderRevision)return;
  if (state.view === "matches") renderMatches();
  else if (state.view === "clubs")
    state.club ? renderClub(state.club) : renderClubs();
  else if (state.view === "players") renderPlayers();
  else if (state.view === "simulator") renderSimulator();
  else if (state.view === "history") renderHistory();
  else if (state.view === "seasons") renderSeasons();
  else if (state.view === "table") renderTable();
  else renderAbout();
  $("content").classList.remove('view-enter');void $("content").offsetWidth;$("content").classList.add('view-enter');
}
function allFixtures() {
  const end = Date.parse(data.as_of) + state.days * 86400000;
  return activeLeagues()
    .flatMap((l) =>
      l.fixtures
        .filter(
          (m) =>
            matchesQuery(short(m.home) + " " + short(m.away)) &&
            (state.days === 0 || Date.parse(m.date) < end),
        )
        .map((m) => ({ l, m })),
    )
    .sort(
      (a, b) =>
        ((matchTime(a.l,a.m).epoch??Date.parse(a.m.date+'T23:59Z'))-(matchTime(b.l,b.m).epoch??Date.parse(b.m.date+'T23:59Z'))) ||
        a.l.key.localeCompare(b.l.key) ||
        a.m.home.localeCompare(b.m.home),
    );
}
function matchId(l, m) {
  const k = [l.key, m.date, m.home, m.away].join("|");
  matchLookup.set(k, { l, m });
  return esc(k);
}
function fixtureMeta(l, m) {
  return catalog.fixtures?.[[l.key, m.date, m.home, m.away].join("|")] || {};
}
const dayLabel = (s) =>
  new Date(s + "T12:00:00Z").toLocaleDateString("en-GB", {
    weekday: "long",
    day: "numeric",
    month: "long",
    timeZone: "UTC",
  });
function matchRow(l, m, { inDay = false, comp = false } = {}) {
  const p = m.prediction,
    meta = fixtureMeta(l, m),
    best = Math.max(p.home_win, p.draw, p.away_win),
    label =
      p.home_win === best ? esc(short(m.home)) + " win" : p.away_win === best ? esc(short(m.away)) + " win" : "Draw",
    kickoff = matchTime(l,m);
  return `<button class="match-row" data-match="${matchId(l, m)}"><span class="match-time" title="${esc(kickoff.label||'Kickoff time not supplied')}">${inDay ? (comp ? `<span class="row-comp" title="${esc(l.name)}">${competitionLogo(l.key)}</span>` : "") : `${esc(dateLabel(kickoff.date))}<br>`}<strong>${esc(kickoff.time)}</strong></span><span class="row-teams"><span class="club-line">${crest(m.home)}<span>${esc(short(m.home))}</span></span><span class="club-line">${crest(m.away)}<span>${esc(short(m.away))}</span></span></span><span class="row-score" title="Most likely score"><span>${p.most_likely_score[0]}</span><span>${p.most_likely_score[1]}</span></span><span class="row-prob"><strong>${pct(best)}</strong>${label}</span><span class="row-bar" aria-hidden="true"><i class="home" style="flex:${Math.max(p.home_win, 0.001)}"></i><i class="draw" style="flex:${Math.max(p.draw, 0.001)}"></i><i class="away" style="flex:${Math.max(p.away_win, 0.001)}"></i></span></button>`;
}
// AI-written weekly recaps per competition, generated by tools/ai_recaps.cjs: key -> { name, text }.
let recaps = {};
function recapsHTML() {
  const items = Object.values(recaps || {}).filter(r => r && r.text);
  if (!items.length) return "";
  return `<section class="panel panel-pad recaps"><div class="mini-label">WEEKLY RECAPS</div>${items.map((r, i) => `<details class="recap"${i === 0 ? " open" : ""}><summary>${esc(r.name)}</summary><p>${esc(r.text)}</p></details>`).join("")}</section>`;
}
// AI 'why' notes per fixture (tools/ai_extras.cjs), keyed like previews.
let why = {};
// Weekly form storylines (tools/ai_extras.cjs): [{ league, team, kind, text }].
let storylines = [];
function outcomeHTML(m) {
  const p = m.prediction, hn = esc(short(m.home)), an = esc(short(m.away));
  const best = Math.max(p.home_win, p.draw, p.away_win);
  const call = p.home_win === best ? `${hn} predicted to win` : p.away_win === best ? `${an} predicted to win` : 'Draw predicted';
  const seg = (w, cls) => `<i class="${cls}" style="flex:${Math.max(w, 0.001)}"></i>`;
  return `<div class="outcome"><div class="outcome-call"><strong>${call}</strong><span>${pct(best)}</span></div><div class="outcome-bar" role="img" aria-label="${hn} ${pct(p.home_win)}, draw ${pct(p.draw)}, ${an} ${pct(p.away_win)}">${seg(p.home_win, 'home')}${seg(p.draw, 'draw')}${seg(p.away_win, 'away')}</div><div class="outcome-legend"><span>${hn} ${pct(p.home_win)}</span><span>Draw ${pct(p.draw)}</span><span>${an} ${pct(p.away_win)}</span></div></div>`;
}
function storylinesHTML() {
  const current = state.competition === "all" ? null : selectedLeague()?.name;
  const items = (storylines || []).filter((s) => !current || s.league === current).slice(0, current ? 3 : 4);
  if (!items.length) return "";
  const title = current ? "FORM WATCH · " + current : "FORM WATCH";
  return `<section class="panel panel-pad form-watch"><div class="mini-label">${esc(title)}</div>${items.map((s) => `<div class="watch-item ${s.kind}"><div class="watch-head"><strong>${esc(short(s.team))}</strong><span class="watch-tag">${s.kind === "hot" ? "Hot run" : "Cold run"}</span></div><div class="watch-meta">${esc(String(s.position))} of ${esc(String(s.size))} · ${esc(String(s.points))} pts</div><p>${esc(s.text)}</p>${current ? "" : `<small>${esc(s.league)}</small>`}</div>`).join("")}</section>`;
}
// AI-written previews, generated by tools/ai_previews.cjs. Keyed by the raw fixture id.
let previews = {};
// Facts about one team for the question box: league position and form, plus its top scorers.
function teamContext(l, name) {
  const team = (l.teams || []).find((t) => t.name === name);
  const form = (team?.form || []).slice(-5).map((f) => `${f.result} ${f.goals[0]}-${f.goals[1]} ${f.venue === "H" ? "home" : "away"} v ${f.opponent}`).join("; ");
  const catalogTeam = Object.values(catalog.teams).find((t) => t.name === name);
  const scorers = (catalogTeam?.players || [])
    .map((p) => ({ p, s: statsFor(catalogTeam, p) }))
    .filter((r) => r.s?.totalGoals > 0)
    .sort((a, b) => b.s.totalGoals - a.s.totalGoals)
    .slice(0, 3)
    .map((r) => `${r.p.name} (${r.s.totalGoals} goals)`);
  return { name, standing: catalogTeam?.standing || null, form: form || null, top_scorers: scorers.join(", ") || null };
}
// Recent meetings between the two teams from the league archive, most recent first.
function headToHead(l, m) {
  return (l.archive || [])
    .filter((x) => (x.home === m.home && x.away === m.away) || (x.home === m.away && x.away === m.home))
    .sort((a, b) => (a.date < b.date ? 1 : -1))
    .slice(0, 5)
    .map((x) => `${x.date} ${x.home} ${x.score[0]}-${x.score[1]} ${x.away}`)
    .join("; ");
}
// Club cups where the knockout simulator applies.
const CUP_KEYS = ["champions_league", "europa_league", "conference_league"];
// Team inputs for the simulator, from each team's season record in this league.
// Team inputs for the simulator. Rates come from each team's season goals, shrunk towards the
// league average: a team with two games counts for little until it has more. Seeds use strength.
function cupTeams(l) {
  const PRIOR_GAMES = 8; // games of average form added to every team before its own results count
  const teams = (l.teams || []).map((t) => ({ name: t.name, gp: t.played || 0, gf: t.goals_for || 0, ga: t.goals_against || 0 }));
  const totalGf = teams.reduce((s, t) => s + t.gf, 0), totalGp = teams.reduce((s, t) => s + t.gp, 0) || 1;
  const avg = totalGf / totalGp || 1;
  return teams
    .filter((t) => t.gp > 0)
    .map((t) => {
      const attack = (t.gf + PRIOR_GAMES * avg) / ((t.gp + PRIOR_GAMES) * avg);
      const defence = (t.ga + PRIOR_GAMES * avg) / ((t.gp + PRIOR_GAMES) * avg);
      return { name: t.name, attack, defence, strength: attack / defence, gp: t.gp };
    })
    .sort((a, b) => b.strength - a.strength);
}
function cupSimHTML() {
  const l = data.leagues.find((x) => x.key === state.competition);
  if (!l || !CUP_KEYS.includes(l.key)) return "";
  return `<section class="panel panel-pad cup-sim"><div class="mini-label">KNOCKOUT SIMULATOR</div><p class="fine-print">Shuffles the top 16 teams in this season's table into a single-leg bracket and plays it 3,000 times. It is a what-if, not the real draw.</p><button class="primary" id="cupRun" data-cup="${esc(l.key)}">Simulate the knockout</button><div id="cupResult"></div></section>`;
}
function runCupSim(key) {
  const l = data.leagues.find((x) => x.key === key);
  if (!l) return;
  const teams = cupTeams(l).slice(0, 16);
  const rows = simulateKnockout(teams, 3000, 20261019);
  const pct = (x) => Math.round(x * 100) + "%";
  const body = rows
    .slice()
    .sort((a, b) => b.reach[a.reach.length - 1] - a.reach[a.reach.length - 1] || a.seed - b.seed)
    .map((r) => `<tr><td>${r.seed}</td><td>${esc(short(r.name))}</td><td>${pct(r.reach[1])}</td><td>${pct(r.reach[2])}</td><td>${pct(r.reach[3])}</td><td>${pct(r.reach[4])}</td></tr>`)
    .join("");
  $("cupResult").innerHTML = `<div class="table-wrap"><table class="cup-table"><thead><tr><th>Seed</th><th>Team</th><th>Quarter-final</th><th>Semi-final</th><th>Final</th><th>Winner</th></tr></thead><tbody>${body}</tbody></table></div><p class="fine-print">Chance each team reaches each round. Rows are ordered by seed.</p>`;
}
document.addEventListener("click", (event) => {
  const run = event.target.closest && event.target.closest("#cupRun");
  if (run) runCupSim(run.dataset.cup);
});
// Short welcome on first visit, dismissed for good once closed.
function onboardingHTML() {
  let seen = false;
  try { seen = localStorage.getItem("touchlineWelcome") === "1"; } catch {}
  if (seen) return "";
  const steps = [
    ["Pick a competition", `${data.leagues.length} leagues, cups and international tournaments.`],
    ["Open any fixture", "Win, draw and loss chances with the likely score."],
    ["Play it out", "Simulate the match with lineups and live commentary."],
  ];
  return `<section class="welcome" aria-label="How Touchline works"><ol>${steps.map(([t, d], i) => `<li><b>${i + 1}</b><span><strong>${t}</strong>${d}</span></li>`).join("")}</ol><button class="welcome-dismiss" id="welcomeDismiss" aria-label="Dismiss introduction">×</button></section>`;
}
document.addEventListener("click", (event) => {
  if (event.target.closest && event.target.closest("#welcomeDismiss")) {
    try { localStorage.setItem("touchlineWelcome", "1"); } catch {}
    document.querySelector(".welcome")?.remove();
  }
});
function previewHTML(l, m) {
  const key = [l.key, m.date, m.home, m.away].join("|"),
    text = previews[key],
    p = m.prediction || {},
    facts = {
      competition: l.name,
      date: m.date,
      home: m.home,
      away: m.away,
      probabilities: { home_win: p.home_win, draw: p.draw, away_win: p.away_win },
      most_likely_score: (p.most_likely_score || []).join("-"),
      home_context: teamContext(l, m.home),
      away_context: teamContext(l, m.away),
      head_to_head: headToHead(l, m) || null,
      kickoff: (() => { const k = matchTime(l, m); return [k.date, k.time, k.label].filter(Boolean).join(" "); })(),
      venue: (() => { const v = Object.values(catalog.teams).find((t) => t.name === m.home)?.venue; return v ? [v.name, v.city, v.country].filter(Boolean).join(", ") : null; })(),
    };
  const whyText = why[key];
  const whyLine = whyText ? `<p class="ai-why"><span class="ai-badge">WHY</span> ${esc(whyText)}</p>` : "";
  const preview = text ? `<p class="ai-preview"><span class="ai-badge">AI preview</span> ${esc(text)}</p>` : "";
  return `${whyLine}${preview}<form class="match-ask" data-facts="${esc(JSON.stringify(facts))}"><input name="question" maxlength="200" placeholder="Ask about the teams, players or this match" aria-label="Ask about the teams, players or this match"><button type="submit">Ask</button></form><p class="match-ask-answer" aria-live="polite"></p>`;
}
// League position of a team from its standing text, e.g. "17th in English Premier League".
function leaguePosition(name) {
  const m = /^(\d+)/.exec(profile(name)?.standing || "");
  return m ? Number(m[1]) : 25;
}
// Most anticipated fixture: both sides strong in their league, and a close contest.
function anticipatedRow(rows) {
  let best = null, bestScore = -Infinity;
  for (const r of rows) {
    const p = r.m.prediction || {};
    const prestige = 1 / leaguePosition(r.m.home) + 1 / leaguePosition(r.m.away);
    const closeness = 1 - Math.abs((p.home_win || 0) - (p.away_win || 0));
    const score = prestige * 10 + closeness;
    if (score > bestScore) { bestScore = score; best = r; }
  }
  return best || rows[0];
}
function featureHTML(r) {
  if (!r) return "";
  const { l, m } = r;
  const kickoff=matchTime(l,m),[h,a]=m.prediction.most_likely_score,mass=m.prediction.most_likely_probability??m.prediction.scores?.[h]?.[a];
  const team = (n) => `<button class="featured-team" data-club="${esc(n)}">${crest(n)}<strong>${esc(short(n))}</strong></button>`;
  return `<section class="featured"><div class="featured-main"><div class="eyebrow">MATCH SPOTLIGHT · ${esc(l.name)}</div><div class="versus">${team(m.home)}<div class="featured-score"><strong>${score(m.prediction.most_likely_score)}</strong><span>Most likely score</span>${Number.isFinite(mass) ? `<small>${pct(mass)} chance</small>` : ""}</div>${team(m.away)}</div><div class="featured-date">${esc(dayLabel(kickoff.date))} · ${esc(kickoff.time)}</div></div><div class="featured-side">${outcomeHTML(m)}${previewHTML(l, m)}<button class="featured-cta" data-match="${matchId(l, m)}">Match analysis & simulation →</button></div></section>`;
}
function leadersHTML() {
  if(state.competition==='all')return `<div class="panel panel-pad"><div class="mini-label">EXPLORE FOOTBALL</div><h2>A world of matchdays.</h2><p class="fine-print">Ten domestic leagues, club tournaments and national teams. Choose a competition to explore its players, squads and fixtures.</p><button class="text-button" data-view="simulator">Build a match →</button></div>`;
  const rows = playerRows()
    .filter((r) => r.s?.appearances > 0)
    .sort((a, b) => (b.s.totalGoals || 0) - (a.s.totalGoals || 0))
    .slice(0, 5);
  if (!rows.length)
    return '<div class="panel panel-pad"><h2>Player statistics</h2><p class="fine-print">No player stats are available for this competition in the current catalog.</p></div>';
  return `<div class="panel panel-pad"><div class="mini-label">SEASON SCORING LEADERS</div>${rows.map((r, i) => playerButton(r.t, r.p, `<span class="leader-rank">${i + 1}</span>${avatar(r.p)}<span class="leader-name">${esc(r.p.name)}<small>${esc(short(r.t.name))}</small></span><span class="leader-number">${val(r.s.totalGoals)}</span>`, "leader-row")).join("")}<button class="text-button" data-view="players">All player statistics →</button><p class="fine-print" style="margin-top:9px">${state.competition === "all" ? "Primary-competition totals; see each player’s competition." : esc(selectedLeague().name) + " totals."}</p></div>`;
}
function renderMatches() {
  heading(
    "MATCH CENTRE",
    state.competition === "all" ? "Across world football" : selectedLeague().name,
    "Your matchday. Fixtures, form and the story behind every game.",
  );
  $("viewTools").innerHTML = `<div class="segmented">${[
    [7, "Next 7 days"],
    [14, "Next 14 days"],
    [0, "Full schedule"],
  ]
    .map(
      ([d, n]) =>
        `<button data-days="${d}" class="${state.days === d ? "active" : ""}">${n}</button>`,
    )
    .join("")}</div>`;
  const rows = allFixtures(),
    groups = new Map(),
    mixed = activeLeagues().length > 1;
  rows.slice(0, state.limit).forEach((r) => {
    const k = matchTime(r.l, r.m).date;
    if (!groups.has(k)) groups.set(k, []);
    groups.get(k).push(r);
  });
  const panels = [...groups]
    .map(
      ([day, g]) =>
        `<section class="panel day-panel"><div class="panel-header day-header"><strong>${esc(dayLabel(day))}</strong><span>${g.length} ${g.length === 1 ? "match" : "matches"}</span></div>${g.map((r) => matchRow(r.l, r.m, { inDay: true, comp: mixed })).join("")}</section>`,
    )
    .join("");
  const teamNames = [...new Set(activeLeagues().flatMap((l) => l.teams.map((t) => t.name)))],
    players = teamNames.reduce((n, name) => n + (catalog.teams[name]?.players_count || catalog.teams[name]?.players?.length || 0), 0);
  $("heroStats").innerHTML = [
    [rows.length, state.days ? `fixtures · next ${state.days} days` : "fixtures scheduled"],
    [teamNames.length, "teams"],
    [players, "players tracked"],
    [data.leagues.length, "competitions"],
  ]
    .filter(([n]) => n > 0)
    .map(([n, label]) => `<div><strong>${n.toLocaleString()}</strong><span>${label}</span></div>`)
    .join("");
  $("content").innerHTML =
    `${onboardingHTML()}${featureHTML(anticipatedRow(rows))}${cupSimHTML()}<div class="content-grid"><div><div class="section-head"><h2>Fixtures & predictions</h2><div class="bar-key" aria-label="Probability bar colours"><span><i class="home"></i>Home</span><span><i class="draw"></i>Draw</span><span><i class="away"></i>Away</span></div></div>${panels || '<div class="empty">No fixtures in this date range. Try the full schedule or explore a match in the simulator.</div>'}${rows.length > state.limit ? '<button class="more" data-more>Show more fixtures</button>' : ""}<p class="table-caption">Kickoffs in your time (${esc(localZone.replaceAll("_", " "))}). The chip shows the most likely score; the bar splits home, draw and away chances.</p></div><aside class="right-column">${storylinesHTML()}${leadersHTML()}${recapsHTML()}</aside></div>`;
}
function probabilities(p) {
  return `<div class="prob-bar" aria-hidden="true"><span style="width:${p.home_win * 100}%"></span><span style="width:${p.draw * 100}%"></span><span style="width:${p.away_win * 100}%"></span></div><div class="prob-labels"><span>Home <strong>${pct(p.home_win)}</strong></span><span>Draw <strong>${pct(p.draw)}</strong></span><span>Away <strong>${pct(p.away_win)}</strong></span></div>`;
}
function predictionHTML(l, m) {
  const p = m.prediction,
    meta = fixtureMeta(l, m),
    cells = p.scores
      .flatMap((r, h) => r.map((v, a) => ({ h, a, v })))
      .sort((a, b) => b.v - a.v)
      .slice(0, 5);
  return `<div class="prediction-panel"><div class="eyebrow">${esc(l.name)} · ${m.date ? esc(dateLabel(m.date)) : "HYPOTHETICAL MATCH"}</div><div class="match-title"><div><button class="match-team-profile" data-club="${esc(m.home)}">${crest(m.home)}<h3>${esc(short(m.home))}</h3><small>Team profile ↗</small></button></div><div class="big-score">${score(p.most_likely_score)}<small>Most likely score</small></div><div><button class="match-team-profile" data-club="${esc(m.away)}">${crest(m.away)}<h3>${esc(short(m.away))}</h3><small>Team profile ↗</small></button></div></div><div class="match-caption">${m.date?esc(matchTime(l,m).label||"Kickoff time not supplied")+" · ":""}${meta.venue ? esc(meta.venue) + " · " : ""}${(m.neutral || (!m.date && l.shared_international_model)) ? "Neutral venue" : "Home advantage included"} · 90 minutes</div>${probabilities(p)}<div class="forecast-stats">${statBlock(p.home_xg.toFixed(2) + " – " + p.away_xg.toFixed(2), "Model expected goals")}${statBlock(pct(p.over_2_5), "Over 2.5 goals")}${statBlock(pct(p.both_score), "Both teams to score")}</div>${simulationControlsHTML(m)}<div class="detail-columns"><div><h3>${l.shared_international_model ? "Recent national-team form" : "Recent competition form"}</h3>${[m.home, m.away].map((n) => `<div class="form-line"><span>${esc(short(n))}</span>${formHTML(modelTeam(n, l))}</div>${formImpact(modelTeam(n, l))}`).join("")}<p class="fine-print">Last five available results, oldest first. Attack and defence respond to opponent-adjusted recent performance, with small-sample shrinkage.</p>${[m.home, m.away].some((n) => (modelTeam(n, l)?.history_matches || 0) < 10) ? '<div class="notice">Limited history for one or both teams. Estimates rely more on the newcomer prior.</div>' : ""}${["champions_league", "europa_league", "conference_league"].includes(l.key) ? '<p class="fine-print">European baseline: fitted only to this competition. Qualification, extra time and penalties are not predicted.</p>' : ""}</div><div><h3>Most likely scorelines</h3>${cells.map((c) => `<div class="score-line"><span>${c.h}–${c.a}</span><div class="score-track"><div style="width:${(100 * c.v) / cells[0].v}%"></div></div><small>${pct(c.v)}</small></div>`).join("")}</div></div>${playerPicksHTML(l, m)}</div>`;
}
let matchRevision=0;
async function openMatch(l, m) {
  if (state.view === "simulator") return;
  const revision=++matchRevision;$('matchDetail').innerHTML='<div class="loading-state">Opening match analysis…</div>';if(!$('matchDialog').open)$('matchDialog').showModal();
  try{await Promise.all([loadLeague(l),loadTeams([catalog.teams[m.home],catalog.teams[m.away]].filter(Boolean))]);if(!m.prediction.scores)m=l.fixtures.find(x=>x.date===m.date&&x.home===m.home&&x.away===m.away)||m;}catch(e){if(revision===matchRevision)$('matchDetail').textContent=e.message;return;}
  if(revision!==matchRevision||!$('matchDialog').open)return;
  $("matchDetail").innerHTML = predictionHTML(l, m);
  bindSimulation(m, l);
}
function renderSimulator() {
  stopReplay();
  if (state.competition === "all") {
    state.competition = "premier_league";
    renderNavigation();
  }
  const l = selectedLeague();
  heading(
    "MATCH SIMULATOR",
    l.name,
    "Choose a matchup and explore its range of outcomes.",
  );
  const names = l.teams.map((t) => t.name);
  if (names.length < 2) {
    $("content").innerHTML =
      '<div class="empty">Not enough current-season clubs to simulate. Refresh competition data.</div>';
    return;
  }
  if (!names.includes(state.home)) state.home = names[0];
  if (!names.includes(state.away) || state.home === state.away)
    state.away = names.find((n) => n !== state.home);
  $("content").innerHTML =
    `<div class="custom-controls"><label class="field">Home team<select id="homeClub" aria-label="Home team">${names.map((n) => `<option value="${esc(n)}" ${n === state.home ? "selected" : ""}>${esc(short(n))}</option>`).join("")}</select></label><span class="vs-divider">VS</span><label class="field">Away team<select id="awayClub" aria-label="Away team">${names
      .filter((n) => n !== state.home)
      .map(
        (n) =>
          `<option value="${esc(n)}" ${n === state.away ? "selected" : ""}>${esc(short(n))}</option>`,
      )
      .join("")}</select></label></div><div id="customPrediction"></div>`;
  const f = l.forecasts.find(
    (f) => f.home === state.home && f.away === state.away,
  );
  $("customPrediction").innerHTML = predictionHTML(l, f);
  bindSimulation(f, l);
  $("homeClub").onchange = () => {
    state.home = $("homeClub").value;
    render();
  };
  $("awayClub").onchange = () => {
    state.away = $("awayClub").value;
    render();
  };
}
function clubNames() {
  return [
    ...new Set(activeLeagues().flatMap((l) => l.teams.map((t) => t.name))),
  ]
    .filter((n) => matchesQuery(short(n) + " " + n))
    .sort((a, b) => short(a).localeCompare(short(b)));
}
function renderClubs() {
  heading(
    "TEAM DIRECTORY",
    state.competition === "all" ? "Clubs & national teams" : selectedLeague().name,
    "Explore team profiles, squads, tactics and history.",
  );
  const names = clubNames();
  $("viewTools").innerHTML =
    `<span class="fine-print">${names.length} teams</span>`;
  $("content").innerHTML = `<div class="club-grid">${names
    .map((n) => {
      const t = profile(n),
        l = activeLeagues().find((l) => l.teams.some((t) => t.name === n)),
        mt = modelTeam(n, l);
      return `<button class="club-card" data-club="${esc(n)}" style="--club-color:${color(t)}"><div class="club-card-top"></div><div class="club-card-main">${crest(n)}<div><h3>${esc(short(n))}</h3><small>${esc(t.country || l.name)}</small><span class="club-profile-link">View team profile →</span></div></div><div class="club-card-bottom"><span>${mt?.played || 0} played · ${mt?.goals_for || 0} goals</span>${formHTML(mt)}</div></button>`;
    })
    .join("")}</div>`;
  if (!names.length)
    $("content").innerHTML =
      '<div class="empty">No clubs match your search.</div>';
}
function renderClub(name) {
 const t=profile(name),l=activeLeagues().find(l=>l.teams.some(t=>t.name===name))||data.leagues.find(l=>l.teams.some(t=>t.name===name));
 heading('TEAM PROFILE',short(name),l?.name||'Team details');$('viewTools').innerHTML='';
 const fixtures=(l?.fixtures||[]).filter(m=>m.home===name||m.away===name).slice(0,8);
 $('content').innerHTML=teamProfileHTML(t,l,modelTeam(name,l),playerRows(name),fixtures);
}
function renderPlayers() {
  heading(
    "PLAYER DATABASE",
    state.competition === "all"
      ? "Players across Europe"
      : selectedLeague().name,
    "Season statistics, squad roles and individual player profiles.",
  );
  const rows = playerRows();
  $("viewTools").innerHTML =
    `<label class="field">Sort by<select id="sortPlayers">${[
      ["totalGoals", "Goals"],
      ["goalAssists", "Assists"],
      ["appearances", "Appearances"],
      ["shotsOnTarget", "Shots on target"],
      ["saves", "Saves"],
    ]
      .map(
        ([v, n]) =>
          `<option value="${v}" ${v === state.sort ? "selected" : ""}>${n}</option>`,
      )
      .join("")}</select></label>`;
  $("sortPlayers").onchange = () => {
    state.sort = $("sortPlayers").value;
    state.limit = 30;
    renderPlayers();
  };
  const featured = rows
    .filter((r) => r.p.portrait && r.s?.appearances > 0)
    .slice(0, 3);
  $("content").innerHTML =
    `${featured.length ? '<div class="player-highlights">' + featured.map(playerCard).join("") + "</div>" : ""}<div class="section-head"><h2>Player statistics</h2><small>${rows.length.toLocaleString()} players</small></div>${rows.length ? playerTable(rows.slice(0, state.limit)) : '<div class="empty">No players match this search.</div>'}${rows.length > state.limit ? '<button class="more" data-more>Show more players</button>' : ""}<p class="table-caption">${state.competition === "all" ? "Stats from each club’s primary competition, shown per player." : "Stats for " + esc(selectedLeague().name) + "."} Season totals as supplied by ESPN; — means unavailable. Profiles retrieved ${esc(catalog.updated_at?.slice(0, 10) || "not yet")}.</p>`;
}
function openPlayer(key) {
 const r=playerLookup.get(key);if(!r)return;
 $('playerDetail').innerHTML=playerProfileHTML(r.t,r.p);
 if(!$('playerDialog').open)$('playerDialog').showModal();
 $('playerDialog').scrollTop=0;
}
function renderHistory() {
  heading(
    "MODEL PERFORMANCE",
    "Predictions against results",
    "Reconstructed pre-match estimates, compared with the final score.",
  );
  const rows = activeLeagues()
    .flatMap((l) =>
      l.historical
        .filter((m) => matchesQuery(short(m.home) + " " + short(m.away)))
        .map((m) => ({ l, m })),
    )
    .sort(
      (a, b) =>
        b.m.date.localeCompare(a.m.date) || a.m.home.localeCompare(b.m.home),
    );
  let hits = 0,
    exact = 0,
    ll = 0,
    bs = 0;
  rows.forEach(({ m }) => {
    const p = m.prediction,
      probs = [p.home_win, p.draw, p.away_win],
      o = m.actual[0] > m.actual[1] ? 0 : m.actual[0] === m.actual[1] ? 1 : 2;
    const scoreline = p.most_likely_score ? (p.most_likely_score[0] > p.most_likely_score[1] ? 0 : p.most_likely_score[0] === p.most_likely_score[1] ? 1 : 2) : -1;
    hits += Number(probs.indexOf(Math.max(...probs)) === o || scoreline === o);
    exact += Number(score(m.actual) === score(p.most_likely_score));
    ll -= Math.log(Math.max(probs[o], 1e-9));
    bs += probs.reduce((s, p, i) => s + (p - Number(i === o)) ** 2, 0);
  });
  const n = rows.length;
  $("viewTools").innerHTML =
    `<span class="fine-print">${esc(dateLabel(data.history_from))} – ${esc(dateLabel(data.as_of))} · ${n} matches</span>`;
  $("content").innerHTML =
    `<div class="model-metrics">${statBlock(n ? pct(hits / n) : "—", "Outcome hit rate")}${statBlock(n ? pct(exact / n) : "—", "Exact score hit rate")}${statBlock(n ? (ll / n).toFixed(3) : "—", "Log loss · lower is better")}${statBlock(n ? (bs / n).toFixed(3) : "—", "Brier score · lower is better")}</div><div class="table-wrap"><table><thead><tr><th>Date / competition</th><th>Match</th><th>Most likely score</th><th>Actual score</th><th>Outcome check</th><th>Home / Draw / Away</th><th>Training cutoff</th></tr></thead><tbody>${rows
      .slice(0, state.limit)
      .map(
        ({ l, m }) =>
          `<tr><td>${esc(dateLabel(m.date))}<br><small class="muted">${esc(l.name)}</small></td><td><div class="club-line">${crest(m.home)}${esc(short(m.home))}</div><div class="club-line" style="margin-top:6px">${crest(m.away)}${esc(short(m.away))}</div></td><td class="score-cell">${score(m.prediction.most_likely_score)}</td><td><strong>${score(m.actual)}</strong>${score(m.actual) === score(m.prediction.most_likely_score) ? '<br><span class="exact-badge">★ Exact score</span>' : ""}</td><td>${outcomeBadge(m)}</td><td>${[m.prediction.home_win, m.prediction.draw, m.prediction.away_win].map(pct).join(" / ")}</td><td>${esc(m.trained_before)}</td></tr>`,
      )
      .join(
        "",
      )}</tbody></table></div>${!n ? '<div class="empty">No evaluated matches for these filters. At least 100 earlier competition results are required.</div>' : ""}${n > state.limit ? '<button class="more" data-more>Show more results</button>' : ""}<p class="table-caption">Outcome checks use the highest home/draw/away probability, independently of the most likely exact score. Gold marks an exact-score match. Reconstructed backtests, not archived forecasts. Weekly fits exclude results on or after the cutoff. Fixed settings may have been tuned using overlapping history.</p>`;
}
function renderAbout() {
  heading(
    "SOURCES & METHODOLOGY",
    "About the data",
    "Coverage, assumptions and the people behind the imagery.",
  );
  $("content").innerHTML =
    `<div class="build-grid">${[["Model","Dixon–Coles in Rust","Attack, defence and home advantage fitted per competition, with recent results weighted more."],["Data","Refreshed daily","A scheduled GitHub Actions job pulls open results, rebuilds and republishes the site."],["Simulation","Runs in your browser","Score distributions are sampled locally into replays, lineups and commentary."],["Notes","AI match context","Short previews and form notes are generated at build time and cached."]].map(([k,t,d])=>`<div class="build-card"><span>${k}</span><strong>${t}</strong><p>${d}</p></div>`).join("")}</div><article class="about panel panel-pad"><h2>${data.leagues.length} competitions. One match centre.</h2><p>${data.leagues.map(l=>esc(l.name)).join(", ")}. Touchline is an independent project and is not affiliated with these competitions or clubs.</p><h2>Coverage in this snapshot</h2><div class="table-wrap"><table><thead><tr><th>Competition</th><th>Latest result</th><th>Training results</th><th>Future fixtures</th></tr></thead><tbody>${data.leagues.map((l) => `<tr><td>${esc(l.name)}</td><td>${esc(l.latest_result || "Unavailable")}</td><td>${l.history_matches}</td><td>${l.fixtures.length}</td></tr>`).join("")}</tbody></table></div><h2>The prediction model</h2><p>The Rust Dixon–Coles model fits club attack, defence and home advantage by competition. National-team tournaments share a deduplicated national-team result pool to retain meaningful history between tournament editions; club results never enter that pool. Hypothetical international simulations use a neutral venue. Recent matches receive more weight with a 365-day half-life. Where available, shots on target contribute a 30% blend to goal targets. Form badges summarize the last five available competition results for clubs and the shared international result history for national teams. Attack and defence are adjusted by recent goals relative to opponent-adjusted expectations, with six pseudo-goals and bounded strength multipliers (0.85–1.15). The last 90 days also inform a competition scoring-level adjustment, shrunk by 40 pseudo-matches and capped at 0.90–1.15. The same procedure is applied using earlier data in each backtest. Model expected goals are scoring-rate estimates, not event-level shot xG.</p><p>European competitions are an experimental baseline fitted to their own available history, including qualifying rounds. Domestic squad quality, player statistics and injuries are not model inputs. Clubs with limited European history rely more on priors. Neutral venues remove league and club home advantage. Predictions cover 90 minutes, not aggregate ties, qualification, extra time or penalties. European results ending after extra time or penalties are omitted when a separate regulation score is unavailable.</p><h2>Simulations and backtests</h2><p>Simulations sample the normalized score distribution, with 0–10 goals per side. Each run gets a fresh random seed by default; an optional fixed seed makes the score and events repeatable. Player goals and assists use an experimental share heuristic from competition season totals and position priors, with a 70% assisted-goal assumption. The simulator can restrict contributors to an illustrative XI chosen from the current squad and selected shape. These are not confirmed upcoming lineups; formation controls do not establish a change in team winning probability. There are no injury or player-minute adjustments, and these are not calibrated player predictions. Synthetic possession, shots, cards and commentary are entertainment, separate from historical data. Event times are illustrative. Historical comparisons refit weekly using only results before the displayed cutoff. Current player profiles, news and season totals never enter historical fits. Settings may have been tuned on overlapping data; these are not untouched holdout evaluations.</p><h2>Historical enrichment</h2><p>Season tables, club honours, player career totals, transfers and market-value history come through <a href="https://github.com/felipeall/transfermarkt-api" target="_blank" rel="noopener">felipeall/transfermarkt-api</a>. Club identities are mapped explicitly or by unique normalized names within their competition. Player identities require a name and birth-date match. ${data.saved_only ? "This public edition includes saved history; profiles without bundled enrichment are clearly marked. More histories can be included in future editions." : "Saved history works offline; the local Node server loads additional history on demand, with provider throttling and disk caching."} Source timestamps remain visible. Player histories do not enter historical team forecasts.</p><h2>Sources and refresh</h2><p>Domestic results: <a href="https://github.com/openfootball/football.json" target="_blank" rel="noopener">openfootball</a>. Historical match statistics: <a href="https://www.football-data.co.uk/" target="_blank" rel="noopener">football-data.co.uk</a>. Club profiles, rosters, competition-specific player totals, logos and European fixtures: <a href="https://www.espn.com/soccer/" target="_blank" rel="noopener">ESPN public football feeds</a>, retrieved ${esc(catalog.updated_at?.slice(0, 10) || "not yet")}. These public endpoints are not a supported API contract. This is a generated snapshot, not a live scores service.</p><p>Profiles can be newer than the forecast snapshot and are labeled separately. Missing player values stay —. Domestic results and provider records can differ because they are refreshed separately. Kickoff times use your browser’s timezone in 12-hour AM/PM format, including daylight saving and local date changes. Missing kickoff times are marked Time TBC.</p>${data.saved_only ? "<p>Data updates are published as new editions. Simulations run in your browser and do not require an account.</p>" : `<pre>node tools/sync_catalog.cjs --year=2026 --refresh<br>python tools/fetch_results.py 2026-27<br>cargo run --release -p predictor --bin dashboard -- ${esc(data.as_of)} dashboard.html</pre>`}<h2>Imagery & typography</h2><p>Club and competition marks are remotely served by ESPN for identification; ownership remains with their rights holders. Player portraits use ESPN-provided images, verified Transfermarkt squad photos and credited Wikimedia Commons images. Transfermarkt identities require a name and birth-date match; source ownership and attribution remain visible. Each player profile links the original, creator and license. Photos are displayed with a layout crop. Internet access loads saved website data, remote imagery and the Inter and Archivo fonts. Simulations use downloaded forecasts without a live football API.</p><p>Inter by Rasmus Andersson and Archivo by Omnibus-Type, served through Google Fonts, are licensed under the SIL Open Font License. Missing artwork uses a simple initials placeholder.</p>${catalog.errors?.length ? `<h2>Refresh issues</h2><p>${catalog.errors.length} source requests failed. Previous result files were retained. Refresh again to fill missing coverage.</p>` : ""}</article>`;
}
document.addEventListener("click", (event) => {
  const b = event.target.closest("button");
  if (!b) return;
  if(b.dataset.profileTab){selectProfileTab(b.closest('.profile-sections'),b.dataset.profileTab);}
  else if(b.dataset.profileSection){selectProfileTab(b.closest('.profile-sections'),b.dataset.profileSection,true);}
  else if(b.dataset.simulateTeam){state.view='simulator';state.home=b.dataset.simulateTeam;const containing=data.leagues.find(l=>l.key===state.competition&&l.teams.some(t=>t.name===state.home))||data.leagues.find(l=>l.teams.some(t=>t.name===state.home));if(containing)state.competition=containing.key;state.club=null;render();window.scrollTo({top:0,behavior:'smooth'});}
  else if(b.hasAttribute("data-retry")){render();}
  else if (b.dataset.tmClub) {
    loadClubHistory(b.dataset.tmClub);
  } else if (b.dataset.tmPlayer) {
    const r = playerLookup.get(b.dataset.tmPlayer);
    if (r) loadPlayerHistory(r.t, r.p);
  } else if (b.dataset.comp) {
    window.scrollTo({top:0,behavior:'instant'});
    state.competition = b.dataset.comp;
    state.club = null;
    state.limit = 30;
    state.query = "";
    $("search").value = "";
    render();
  } else if (b.dataset.view) {
    window.scrollTo({top:0,behavior:'instant'});
    $("navigationDialog").close();
    $("matchDialog").close();
    $("playerDialog").close();
    $("matchDetail").innerHTML = "";
    state.view = b.dataset.view;
    state.query = "";
    $("search").value = "";
    state.club = null;
    state.limit = 30;
    render();
  } else if (b.dataset.club) {
    $("playerDialog").close();$("matchDialog").close();
    state.view = "clubs";
    state.club = b.dataset.club;
    state.query = "";
    $("search").value = "";
    render();
    window.scrollTo({ top: 0, behavior: "smooth" });
  } else if (b.hasAttribute("data-retry")) {
    render();
  } else if (b.hasAttribute("data-back-clubs")) {
    state.club = null;
    window.scrollTo({top:0,behavior:"smooth"});
    render();
  } else if (b.hasAttribute("data-days")) {
    state.days = Number(b.dataset.days);
    state.limit = 30;
    render();
  } else if (b.hasAttribute("data-more")) {
    state.limit += 30;
    render();
  } else if (b.dataset.match) {
    const r = matchLookup.get(b.dataset.match);
    if (r) openMatch(r.l, r.m);
  } else if (b.dataset.player) openPlayer(b.dataset.player);
  else if (b.classList.contains("dialog-close")) b.closest("dialog").close();
});
$('moreNavigation').onclick=()=>$('navigationDialog').showModal();
$('mobileCompetition').onchange=()=>{state.competition=$('mobileCompetition').value;state.club=null;state.query='';state.limit=30;$('search').value='';window.scrollTo({top:0,behavior:'instant'});render();};
document.addEventListener('keydown',e=>{
 const tab=e.target.closest('[role="tab"]');if(!tab||!['ArrowLeft','ArrowRight','Home','End'].includes(e.key))return;
 const root=tab.closest('.profile-sections'),tabs=[...root.querySelectorAll('[role="tab"]')],i=tabs.indexOf(tab);
 const next=e.key==='Home'?0:e.key==='End'?tabs.length-1:(i+(e.key==='ArrowRight'?1:-1)+tabs.length)%tabs.length;
 e.preventDefault();selectProfileTab(root,tabs[next].dataset.profileTab,true);
});
$("matchDialog").addEventListener("close", stopReplay);
document.querySelector(".wordmark").onclick = (e) => {
  e.preventDefault();
  window.scrollTo({top:0,behavior:'instant'});
  state.view = "matches";
  state.club = null;
  render();
};
$("search").oninput = () => {
  state.query = $("search").value;
  state.limit = 30;
  render();
};
document.addEventListener(
  "error",
  (e) => {
    if (e.target.tagName === "IMG") {
      if(e.target.dataset.fallbackSrc){const fallback=e.target.dataset.fallbackSrc;delete e.target.dataset.fallbackSrc;e.target.src=fallback;return;}
      const span = document.createElement("span");
      span.className = e.target.classList.contains("avatar") || !!e.target.closest(".profile-player-identity")
        ? "avatar portrait-fallback"
        : "crest-placeholder";
      span.textContent = initials(e.target.alt || "");
      span.title = e.target.alt + " · image unavailable";
      e.target.replaceWith(span);
    }
  },
  true,
);
for (const d of document.querySelectorAll("dialog"))
  d.addEventListener("click", (e) => {
    if (e.target === d) {
      const r = d.getBoundingClientRect();
      if (
        e.clientX < r.left ||
        e.clientX > r.right ||
        e.clientY < r.top ||
        e.clientY > r.bottom
      )
        d.close();
    }
  });
render();
fetch(new URL("assets/previews.json", document.baseURI)).then(r => (r.ok ? r.json() : {})).then(p => { previews = p; render(); }).catch(() => {});
fetch(new URL("assets/why.json", document.baseURI)).then(r => (r.ok ? r.json() : {})).then(p => { why = p; render(); }).catch(() => {});
fetch(new URL("assets/storylines.json", document.baseURI)).then(r => (r.ok ? r.json() : [])).then(p => { storylines = p; render(); }).catch(() => {});
fetch(new URL("assets/history.json", document.baseURI)).then(r => (r.ok ? r.json() : {})).then(p => { seasonHistory = p; render(); }).catch(() => {});

// Ask about a match: the page sends only the facts it already shows; the server answers from those.
document.addEventListener("submit", async (event) => {
  const form = event.target.closest && event.target.closest(".match-ask");
  if (!form) return;
  event.preventDefault();
  const out = form.nextElementSibling;
  const question = form.elements.question.value.trim();
  if (question.length < 3) {
    out.textContent = "Ask a question of at least three characters.";
    return;
  }
  out.textContent = "Thinking…";
  try {
    const res = await fetch(new URL("api/match-question", document.baseURI), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ question, fixture: JSON.parse(form.dataset.facts) }),
    });
    const body = await res.json().catch(() => ({}));
    out.textContent = res.ok ? body.answer : body.error || "Could not get an answer.";
  } catch {
    out.textContent = "Could not reach the answer service.";
  }
});
fetch(new URL("assets/recaps.json", document.baseURI)).then(r => (r.ok ? r.json() : {})).then(p => { recaps = p; render(); }).catch(() => {});

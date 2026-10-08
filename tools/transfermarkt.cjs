// Adapter for https://github.com/felipeall/transfermarkt-api (v4 response shapes).
// All endpoint paths are constructed from verified identities. No generic proxy.
const fs = require("node:fs"),
  path = require("node:path"),
  crypto = require("node:crypto");
const ROOT = path.resolve(__dirname, ".."),
  DIR = path.join(ROOT, "data/catalog"),
  CACHE = path.join(ROOT, "target/transfermarkt-cache");
const IDS = {
  premier_league: "GB1",
  la_liga: "ES1",
  serie_a: "IT1",
  bundesliga: "L1",
  ligue_1: "FR1",
  primeira_liga: "PO1",
  eredivisie: "NL1",
  belgian_pro_league: "BE1",
  super_lig: "TR1",
  chance_liga: "TS1",
  champions_league: "CL",
  europa_league: "EL",
  conference_league: "UCOL",
};
const normalize = (s) =>
  String(s || "")
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(
      /\b(fc|afc|cf|sc|ac|as|ssc|rc|rcd|cd|ud|ca|sv|tsg|vfb|vfl|fsv|fk|calcio|club|de|del|football|1899|1907|1913|1909|04|05|07|29|1)\b/g,
      "",
    )
    .replace(/[^a-z0-9]/g, "");
const file = path.join(DIR, "transfermarkt.json");
const snapshot = fs.existsSync(file)
  ? JSON.parse(fs.readFileSync(file))
  : {
      source: "felipeall/transfermarkt-api",
      competitions: {},
      club_ids: {},
      clubs: {},
      players: {},
      tables: {},
      unmapped: [],
    };
const REFRESH = process.argv.includes("--refresh");
let queue = Promise.resolve(),
  lastRequest = 0;
function save() {
  fs.mkdirSync(DIR, { recursive: true });
  fs.writeFileSync(file, JSON.stringify(snapshot, null, 2) + "\n");
}
function request(endpoint) {
  const job = queue.then(async () => {
    const base = (
      process.env.TRANSFERMARKT_API_URL || "https://transfermarkt-api.fly.dev"
    ).replace(/\/$/, "");
    const cache = path.join(
      CACHE,
      crypto
        .createHash("sha256")
        .update(base + endpoint)
        .digest("hex") + ".json",
    );
    if (
      !REFRESH &&
      fs.existsSync(cache) &&
      Date.now() - fs.statSync(cache).mtimeMs < 24 * 3600000
    )
      return JSON.parse(fs.readFileSync(cache));
    const delay = Math.max(0, 1650 - (Date.now() - lastRequest));
    if (delay) await new Promise((r) => setTimeout(r, delay));
    lastRequest = Date.now();
    const r = await fetch(base + endpoint, {
      signal: AbortSignal.timeout(25000),
      headers: { "User-Agent": "TouchlineFootball/0.3" },
    });
    if (!r.ok)
      throw new Error(
        "Transfermarkt provider returned HTTP " +
          r.status +
          ". Try again later.",
      );
    const d = await r.json();
    fs.mkdirSync(CACHE, { recursive: true });
    fs.writeFileSync(cache, JSON.stringify(d));
    return d;
  });
  queue = job.catch(() => {});
  return job;
}
function catalog() {
  return JSON.parse(fs.readFileSync(path.join(DIR, "catalog.json")));
}
async function sync(year) {
  const c = catalog();
  snapshot.unmapped = [];
  const overrides = JSON.parse(
    fs.readFileSync(path.join(DIR, "transfermarkt-map.json"), "utf8"),
  );
  for (const [key, id] of Object.entries(IDS)) {
    console.log("Transfermarkt competition", key);
    const clubs = await request(`/competitions/${id}/clubs?season_id=${year}`),
      seasons = await request(`/competitions/${id}/seasons`);
    snapshot.competitions[key] = {
      id,
      name: clubs.name,
      seasons: seasons.seasons,
      updatedAt: seasons.updatedAt,
    };
    for (const t of Object.values(c.teams).filter((t) =>
      t.competitions.includes(key),
    )) {
      if (snapshot.club_ids[t.name]) continue;
      if (
        overrides[t.name] &&
        clubs.clubs.some((x) => x.id === overrides[t.name])
      ) {
        snapshot.club_ids[t.name] = overrides[t.name];
        continue;
      }
      const found = clubs.clubs.filter((x) =>
        [t.name, t.display_name, t.short_name].some(
          (n) => normalize(x.name) === normalize(n),
        ),
      );
      if (found.length === 1) snapshot.club_ids[t.name] = found[0].id;
    }
    save();
  }
  snapshot.unmapped = Object.keys(c.teams).filter((n) => !snapshot.club_ids[n]);
  save();
}
async function club(name) {
  if (!catalog().teams[name]) throw new Error("Unknown club");
  const id = snapshot.club_ids[name];
  if (!id)
    throw new Error("This club has no verified Transfermarkt identity yet.");
  if (!REFRESH && snapshot.clubs[name]) return snapshot.clubs[name];
  const profile = await request(`/clubs/${id}/profile`),
    achievements = await request(`/clubs/${id}/achievements`);
  const d = { profile, achievements };
  snapshot.clubs[name] = d;
  save();
  return d;
}
async function player(name, playerId) {
  const t = catalog().teams[name],
    p = t?.players.find((p) => p.id === playerId),
    id = snapshot.club_ids[name];
  if (!p || !id)
    throw new Error("No verified club/player mapping is available.");
  const key = t.id + ":" + p.id;
  if (!REFRESH && snapshot.players[key]) return snapshot.players[key];
  const roster = await request(`/clubs/${id}/players`);
  const matches = roster.players.filter(
    (x) => normalize(x.name) === normalize(p.name),
  );
  if (matches.length !== 1)
    throw new Error("Player identity could not be matched uniquely.");
  const tmId = matches[0].id,
    profile = await request(`/players/${tmId}/profile`);
  if (!p.birth_date || profile.dateOfBirth !== p.birth_date)
    throw new Error("Player birth date did not match; history was not linked.");
  const stats = await request(`/players/${tmId}/stats`),
    transfers = await request(`/players/${tmId}/transfers`),
    achievements = await request(`/players/${tmId}/achievements`),
    market = await request(`/players/${tmId}/market_value`);
  const d = { profile, stats, transfers, achievements, market };
  snapshot.players[key] = d;
  save();
  return d;
}
async function table(key, season) {
  const id = IDS[key];
  if (
    !id ||
    !/^\d{4}$/.test(season) ||
    Number(season) < 1900 ||
    Number(season) > 2200
  )
    throw new Error("Invalid competition or season");
  const cacheKey = key + ":" + season;
  if (!REFRESH && snapshot.tables[cacheKey]) return snapshot.tables[cacheKey];
  const d = await request(`/competitions/${id}/table?season_id=${season}`);
  snapshot.tables[cacheKey] = d;
  save();
  return d;
}
module.exports = {
  IDS,
  normalize,
  sync,
  club,
  player,
  table,
  snapshot,
  request,
};
if (require.main === module)
  sync(Number(process.argv[2] || new Date().getUTCFullYear()))
    .then(async () => {
      if (REFRESH) {
        for (const name of Object.keys(snapshot.clubs)) await club(name);
        const c = catalog();
        for (const key of Object.keys(snapshot.players)) {
          const [clubId, playerId] = key.split(":");
          const t = Object.values(c.teams).find((t) => t.id === clubId);
          if (t?.players.some((p) => p.id === playerId))
            await player(t.name, playerId);
        }
        for (const key of Object.keys(snapshot.tables)) {
          const [competition, season] = key.split(":");
          await table(competition, season);
        }
      }
      console.log(
        "Mapped",
        Object.keys(snapshot.club_ids).length,
        "clubs; unmatched:",
        snapshot.unmapped,
      );
    })
    .catch((e) => {
      console.error(e.message);
      process.exitCode = 1;
    });

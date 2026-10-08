#!/usr/bin/env node
// Public football profiles and UEFA fixtures. No API key or npm packages required.
// ESPN endpoints are public but unofficial and may change. Failed refreshes keep prior files.
const fs = require("node:fs"),
  path = require("node:path");
const ROOT = path.resolve(__dirname, "..");
const OUT = path.join(ROOT, "data/catalog");
const CACHE = path.join(ROOT, "target/feed-cache");
const COMPETITIONS = [
  ["premier_league", "eng.1", "Premier League", "England"],
  ["la_liga", "esp.1", "La Liga", "Spain"],
  ["serie_a", "ita.1", "Serie A", "Italy"],
  ["bundesliga", "ger.1", "Bundesliga", "Germany"],
  ["ligue_1", "fra.1", "Ligue 1", "France"],
  ["champions_league", "uefa.champions", "Champions League", "Europe"],
  ["europa_league", "uefa.europa", "Europa League", "Europe"],
  ["conference_league", "uefa.europa.conf", "Conference League", "Europe"],
];
const opts = Object.fromEntries(
  process.argv.slice(2).map((s) => s.replace(/^--/, "").split("=")),
);
const YEAR = Number(opts.year || new Date().getUTCFullYear());
if (!Number.isInteger(YEAR) || YEAR < 2022 || YEAR > 2200)
  throw new Error("Use --year=YYYY (2022 or later)");
const REFRESH = "refresh" in opts;
fs.mkdirSync(OUT, { recursive: true });
fs.mkdirSync(CACHE, { recursive: true });
const errors = [];
const throttledHosts = new Set();
async function json(url) {
  const cache = path.join(
    CACHE,
    require("node:crypto").createHash("sha256").update(url).digest("hex") +
      ".json",
  );
  if (!REFRESH && fs.existsSync(cache))
    return JSON.parse(fs.readFileSync(cache));
  const host = new URL(url).hostname;
  if (throttledHosts.has(host))
    throw new Error("Provider throttled; try a later refresh: " + host);
  let last;
  for (let i = 0; i < 3; i++) {
    try {
      const r = await fetch(url, {
        headers: {
          "User-Agent":
            "TouchlineFootball/0.2 (personal football research dashboard)",
        },
        signal: AbortSignal.timeout(15000),
      });
      if (r.status === 429) {
        throttledHosts.add(host);
        throw new Error("HTTP 429: provider throttled");
      }
      if (r.status === 404 && /wikipedia|wikimedia|wikidata/.test(host)) {
        const missing = { missing: true };
        fs.writeFileSync(cache, JSON.stringify(missing));
        return missing;
      }
      if (!r.ok) throw new Error("HTTP " + r.status);
      const d = await r.json();
      fs.writeFileSync(cache, JSON.stringify(d));
      return d;
    } catch (e) {
      last = e;
      if (throttledHosts.has(host)) break;
      if (i < 2) await new Promise((r) => setTimeout(r, 500 * (i + 1)));
    }
  }
  throw new Error(url + ": " + last.message);
}
async function pool(items, fn, n = 4) {
  let i = 0;
  await Promise.all(
    Array.from({ length: n }, async () => {
      while (i < items.length) {
        const item = items[i++];
        await fn(item);
      }
    }),
  );
}
const normalize = (s) =>
  s
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(
      /\b(fc|afc|cf|sc|ac|as|ssc|cfc|rc|rcd|cd|ud|ca|sv|tsg|vfb|vfl|fsv|fk|calcio|club|de|del|football|1899|1907|1913|1909|04|05|07|29|1)\b/g,
      "",
    )
    .replace(/[^a-z0-9]/g, "");
const domesticNames = new Map();
for (const [key] of COMPETITIONS.slice(0, 5)) {
  const file = path.join(
    ROOT,
    `data/results/${YEAR}-${String((YEAR + 1) % 100).padStart(2, "0")}/${key}.json`,
  );
  if (fs.existsSync(file)) {
    const alias = JSON.parse(
      fs.readFileSync(path.join(ROOT, "data/results/aliases.json")),
    );
    domesticNames.set(key, [
      ...new Set(
        JSON.parse(fs.readFileSync(file)).matches.flatMap((m) => [
          alias[m.team1] || m.team1,
          alias[m.team2] || m.team2,
        ]),
      ),
    ]);
  }
}
const overrides = fs.existsSync(path.join(OUT, "team-map.json"))
  ? JSON.parse(fs.readFileSync(path.join(OUT, "team-map.json")))
  : {};
function canonical(t, key) {
  const names = domesticNames.get(key) || [];
  const mapped = overrides[t.id];
  if (mapped && names.includes(mapped)) return mapped;
  const candidates = names.filter((n) =>
    [t.displayName, t.name, t.shortDisplayName, t.nickname]
      .filter(Boolean)
      .some((a) => normalize(n) === normalize(a)),
  );
  return candidates.length === 1 ? candidates[0] : null;
}
const catalog = {
  updated_at: new Date().toISOString(),
  source: "ESPN public football feeds",
  competitions: [],
  teams: {},
  fixtures: {},
  unmapped: [],
  errors,
};
const teamsById = new Map();
async function main() {
  for (const [key, slug, name, country] of COMPETITIONS) {
    console.log("Competition:", name);
    try {
      const list = await json(
        `https://site.api.espn.com/apis/site/v2/sports/soccer/${slug}/teams?limit=200`,
      );
      const league = list.sports?.[0]?.leagues?.[0];
      if (!league?.teams?.length) throw new Error("No teams returned");
      const board = await json(
        `https://site.api.espn.com/apis/site/v2/sports/soccer/${slug}/scoreboard`,
      );
      catalog.competitions.push({
        key,
        slug,
        name,
        country,
        logo: board.leagues?.[0]?.logos?.[0]?.href || null,
        team_ids: league.teams.map((x) => x.team.id),
      });
      for (const { team: t } of league.teams) {
        const existing = teamsById.get(t.id);
        if (existing) {
          existing.competitions.push(key);
          continue;
        }
        const matched = canonical(t, key);
        if (country !== "Europe" && !matched)
          catalog.unmapped.push({
            id: t.id,
            feed: t.displayName,
            competition: key,
          });
        teamsById.set(t.id, {
          id: t.id,
          name: matched || t.displayName,
          display_name: t.displayName,
          short_name: t.shortDisplayName || t.displayName,
          abbreviation: t.abbreviation,
          color: t.color,
          logo: t.logos?.[0]?.href || null,
          slug,
          competitions: [key],
          country,
          source_url:
            t.links?.find((l) => l.rel.includes("clubhouse"))?.href || null,
        });
      }
    } catch (e) {
      errors.push({ competition: key, error: e.message });
    }
  }
  // Resolve names once the full domestic registry is available; never fuzzy-match clubs.
  for (const t of teamsById.values()) catalog.teams[t.name] = t;
  console.log(
    "Club profiles:",
    teamsById.size,
    "unmapped domestic:",
    catalog.unmapped.length,
  );
  await pool([...teamsById.values()], async (t) => {
    try {
      const detail = await json(
        `https://site.api.espn.com/apis/site/v2/sports/soccer/${t.slug}/teams/${t.id}`,
      );
      const roster = await json(
        `https://site.api.espn.com/apis/site/v2/sports/soccer/${t.slug}/teams/${t.id}/roster`,
      );
      // Coach fields in this feed can be undated historical records, so omit them.
      const d = detail.team || {};
      t.nickname = d.nickname;
      t.standing = d.standingSummary;
      t.season = roster.season?.displayName || null;
      t.profile_updated_at = roster.timestamp || catalog.updated_at;
      const venue = d.nextEvent?.[0]?.competitions?.find((c) =>
        c.competitors?.find((x) => x.homeAway === "home" && x.id === t.id),
      )?.venue;
      if (venue)
        t.venue = {
          name: venue.fullName,
          city: venue.address?.city,
          country: venue.address?.country,
        };
      t.record = Object.fromEntries(
        (d.record?.items?.find((x) => x.type === "total")?.stats || []).map(
          (s) => [s.name, s.value],
        ),
      );
      t.players = (roster.athletes || []).map((a) => ({
        id: a.id,
        name: a.displayName,
        number: a.jersey || null,
        age: a.age || null,
        position: a.position?.displayName || a.position?.name || null,
        position_code: a.position?.abbreviation || null,
        nationality: a.citizenship || null,
        height_cm: a.height ? Math.round(a.height * 2.54) : null,
        weight_kg: a.weight ? Math.round(a.weight * 0.453592) : null,
        birth_date: a.dateOfBirth?.slice(0, 10) || null,
        source_url:
          a.links?.find((l) => l.rel.includes("playercard"))?.href || null,
        stats: a.statistics?.splits
          ? Object.fromEntries(
              a.statistics.splits.categories
                .flatMap((c) => c.stats)
                .map((s) => [s.name, s.value]),
            )
          : null,
      }));
      t.player_stats = {};
      for (const key of t.competitions) {
        const comp = catalog.competitions.find((c) => c.key === key);
        const scoped =
          comp.slug === t.slug
            ? roster
            : await json(
                `https://site.api.espn.com/apis/site/v2/sports/soccer/${comp.slug}/teams/${t.id}/roster`,
              );
        t.player_stats[key] = {
          season: scoped.season?.displayName || null,
          players: Object.fromEntries(
            (scoped.athletes || []).map((a) => [
              a.id,
              a.statistics?.splits
                ? Object.fromEntries(
                    a.statistics.splits.categories
                      .flatMap((c) => c.stats)
                      .map((s) => [s.name, s.value]),
                  )
                : null,
            ]),
          ),
        };
      }
    } catch (e) {
      t.players = t.players || [];
      errors.push({ team: t.name, error: e.message });
    }
  });
  // Domestic kickoff/venue metadata supplements, but never overwrites, historical results.
  for (const c of catalog.competitions.filter((c) => c.country !== "Europe")) {
    await pool(
      [YEAR, YEAR + 1],
      async (year) => {
        try {
          const d = await json(
            `https://site.api.espn.com/apis/site/v2/sports/soccer/${c.slug}/scoreboard?dates=${year}&limit=1000`,
          );
          if (!Array.isArray(d.events) || d.events.length >= 1000)
            throw new Error("Missing or truncated domestic scoreboard");
          for (const e of d.events) {
            const g = e.competitions?.[0],
              h = g?.competitors?.find((x) => x.homeAway === "home"),
              a = g?.competitors?.find((x) => x.homeAway === "away");
            if (!h || !a) continue;
            const ht = teamsById.get(h.id),
              at = teamsById.get(a.id);
            if (!ht || !at) continue;
            const date = (g.date || e.date).slice(0, 10);
            catalog.fixtures[`${c.key}|${date}|${ht.name}|${at.name}`] = {
              kickoff: g.date || e.date,
              venue: g.venue?.fullName || null,
              status:
                g.status?.type?.description || e.status?.type?.description,
              neutral: !!g.neutralSite,
              source_url:
                e.links?.find((l) => l.rel.includes("summary"))?.href || null,
            };
            if (!ht.venue && g.venue && !g.neutralSite)
              ht.venue = {
                name: g.venue.fullName,
                city: g.venue.address?.city,
                country: g.venue.address?.country,
              };
          }
        } catch (e) {
          errors.push({ competition: c.key, error: e.message });
        }
      },
      2,
    );
  }
  // UEFA fixtures/results: yearly responses can exceed provider limits, so use months.
  for (const c of catalog.competitions.filter((c) => c.country === "Europe")) {
    const seasonFiles = new Map();
    const seen = new Set();
    let skipped = 0;
    const months = [];
    for (let y = Math.max(2022, YEAR - 3); y <= YEAR + 1; y++)
      for (let m = 1; m <= 12; m++)
        if (y < YEAR + 1 || m <= 6)
          months.push(`${y}${String(m).padStart(2, "0")}`);
    console.log("European fixtures:", c.name, months.length, "months");
    let failed = false;
    await pool(
      months,
      async (month) => {
        try {
          const d = await json(
            `https://site.api.espn.com/apis/site/v2/sports/soccer/${c.slug}/scoreboard?dates=${month}&limit=1000`,
          );
          if (!Array.isArray(d.events)) throw new Error("No events array");
          if (d.events.length >= 1000)
            throw new Error("Potentially truncated events response");
          for (const e of d.events) {
            if (seen.has(e.id)) continue;
            seen.add(e.id);
            const g = e.competitions?.[0];
            if (!g) continue;
            const h = g.competitors?.find((x) => x.homeAway === "home"),
              a = g.competitors?.find((x) => x.homeAway === "away");
            if (!h || !a) continue;
            const status = g.status?.type || e.status?.type;
            const completed = status?.completed;
            // Full-time only. Final scores after extra time or penalties are not 90-minute scores.
            if (completed && status.name !== "STATUS_FULL_TIME") {
              skipped++;
              continue;
            }
            if (
              !completed &&
              !["STATUS_SCHEDULED", "STATUS_POSTPONED"].includes(status?.name)
            )
              continue;
            const home =
                teamsById.get(h.id)?.name ||
                `ESPN:${h.id}:${h.team.displayName}`,
              away =
                teamsById.get(a.id)?.name ||
                `ESPN:${a.id}:${a.team.displayName}`;
            const date = (g.date || e.date).slice(0, 10);
            const sy =
              Number(date.slice(0, 4)) - (date.slice(5, 7) < "07" ? 1 : 0);
            const season = `${sy}-${String((sy + 1) % 100).padStart(2, "0")}`;
            const gh = Number(h.score),
              ga = Number(a.score);
            if (
              completed &&
              (!Number.isInteger(gh) ||
                !Number.isInteger(ga) ||
                gh < 0 ||
                ga < 0 ||
                gh > 30 ||
                ga > 30)
            ) {
              skipped++;
              continue;
            }
            const row = {
              date,
              team1: home,
              team2: away,
              neutral: !!g.neutralSite,
              ...(completed ? { score: { ft: [gh, ga] } } : {}),
            };
            if (!seasonFiles.has(season)) seasonFiles.set(season, []);
            seasonFiles.get(season).push(row);
            catalog.fixtures[`${c.key}|${date}|${home}|${away}`] = {
              kickoff: g.date || e.date,
              venue: g.venue?.fullName || null,
              stage: g.altGameNote || null,
              status: status.description,
              neutral: !!g.neutralSite,
              source_url:
                e.links?.find((l) => l.rel.includes("summary"))?.href || null,
            };
          }
        } catch (e) {
          failed = true;
          errors.push({ competition: c.key, month, error: e.message });
        }
      },
      3,
    );
    c.skipped_non_regulation = skipped;
    if (!failed) {
      for (const [season, matches] of seasonFiles) {
        const dir = path.join(ROOT, "data/results", season);
        fs.mkdirSync(dir, { recursive: true });
        matches.sort(
          (a, b) =>
            a.date.localeCompare(b.date) || a.team1.localeCompare(b.team1),
        );
        fs.writeFileSync(
          path.join(dir, c.key + ".json"),
          JSON.stringify(
            {
              name: c.name,
              source: "ESPN",
              updated_at: catalog.updated_at,
              matches,
            },
            null,
            2,
          ) + "\n",
        );
      }
    } else
      console.log(
        "Kept previous result files after incomplete refresh:",
        c.name,
      );
  }
  // Real portraits for leading players, with links to the original Commons attribution.
  const featured = [...teamsById.values()].flatMap((t) =>
    (t.players || [])
      .filter((p) => p.stats)
      .sort(
        (a, b) =>
          (b.stats.totalGoals || 0) * 3 +
          (b.stats.goalAssists || 0) -
          (a.stats.totalGoals || 0) * 3 -
          (a.stats.goalAssists || 0),
      )
      .slice(0, 3),
  );
  console.log("Player portraits:", featured.length);
  await pool(
    featured,
    async (p) => {
      try {
        const d = await json(
          "https://en.wikipedia.org/api/rest_v1/page/summary/" +
            encodeURIComponent(p.name.replaceAll(" ", "_")),
        );
        // Require a football biography; ambiguous names and non-person images remain absent.
        if (
          !/footballer|soccer player/i.test(d.description || "") ||
          !d.thumbnail?.source ||
          !d.originalimage?.source ||
          !p.birth_date
        )
          return;
        const born = d.extract?.match(
          /born\s+(\d{1,2}\s+[A-Za-z]+\s+\d{4}|[A-Za-z]+\s+\d{1,2},\s+\d{4})/i,
        )?.[1];
        let birth =
          born && Number.isFinite(Date.parse(born + " UTC"))
            ? new Date(born + " UTC").toISOString().slice(0, 10)
            : null;
        if (!birth && d.wikibase_item) {
          const wd = await json(
            "https://www.wikidata.org/wiki/Special:EntityData/" +
              d.wikibase_item +
              ".json",
          );
          birth = wd.entities?.[
            d.wikibase_item
          ]?.claims?.P569?.[0]?.mainsnak?.datavalue?.value?.time?.slice(1, 11);
        }
        if (birth !== p.birth_date) return;
        const filename = decodeURIComponent(
          new URL(d.originalimage.source).pathname.split("/").pop(),
        );
        const metadata = await json(
          "https://commons.wikimedia.org/w/api.php?action=query&format=json&prop=imageinfo&iiprop=extmetadata&titles=" +
            encodeURIComponent("File:" + filename),
        );
        const info = Object.values(metadata.query?.pages || {})[0]
          ?.imageinfo?.[0]?.extmetadata;
        const plain = (s) =>
          String(s || "")
            .replace(/<[^>]*>/g, "")
            .replace(/&quot;/g, '"')
            .replace(/&amp;/g, "&")
            .replace(/&#39;/g, "'");
        if (!info?.LicenseShortName?.value || !info?.Artist?.value) return;
        p.portrait = {
          url: d.thumbnail.source,
          page: d.content_urls?.desktop?.page,
          credit: plain(info.Artist.value),
          license: plain(info.LicenseShortName.value),
          license_url: info.LicenseUrl?.value || null,
          source_url:
            "https://commons.wikimedia.org/wiki/File:" +
            encodeURIComponent(filename),
        };
      } catch {
        /* Missing portraits are intentional; never invent a likeness. */
      }
    },
    3,
  );
  fs.writeFileSync(
    path.join(OUT, "catalog.json"),
    JSON.stringify(catalog, null, 2) + "\n",
  );
  console.log(
    JSON.stringify(
      {
        teams: teamsById.size,
        players: [...teamsById.values()].reduce(
          (s, t) => s + (t.players?.length || 0),
          0,
        ),
        portraits: featured.filter((p) => p.portrait).length,
        unmapped: catalog.unmapped,
        errors,
      },
      null,
      2,
    ),
  );
  if (errors.length) process.exitCode = 1;
}
if (require.main === module)
  main().catch((e) => {
    console.error(e);
    process.exitCode = 1;
  });
module.exports = { normalize };

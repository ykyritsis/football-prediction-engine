# Football catalog

`catalog.json` is a dated public-data snapshot: club and competition identities,
logos, full squads, professional player profiles, competition-scoped season
statistics, fixture metadata and selected player portraits. No API keys are used.

Run from the repository root with Node 20 or newer:

```sh
node tools/sync_catalog.cjs --year=2026 --refresh
cargo run --release -p predictor --bin dashboard -- 2026-10-08 dashboard.html
```

The year selects the season starting in that calendar year. Existing domestic
result files define canonical club names. `team-map.json` maps ESPN IDs to names
that differ across providers; remaining mappings require a unique normalized
exact match. Inspect the printed `unmapped` and `errors` arrays after refreshing.
Never accept a fuzzy club-name match silently.

Without `--refresh`, requests reuse `target/feed-cache/` for restartable builds.
With `--refresh`, source requests are repeated. HTTP 429 pauses requests to that
host for the rest of the run; a later invocation can retry. European results are
saved only when every monthly request for that competition succeeds. Other
request failures are recorded and cause a nonzero exit. An unavailable portrait
is optional and is omitted. A fetched timestamp says when data was requested,
not when the source last updated every player or fixture.

## Sources

* Club profiles, rosters, player statistics, European results and fixtures:
  ESPN public `site.api.espn.com` football feeds. These are unofficial public
  endpoints with no stability guarantee. Logos are URLs returned by the feed
  and remain the property of their respective rights holders.
* Player portraits: Wikipedia REST summaries and Wikimedia Commons image
  metadata. Only football biographies whose date of birth matches the roster
  are accepted. Name collisions and missing metadata produce no portrait.
  Each accepted image stores the creator, license and original Commons file
  URL, displayed in the player profile. Photos are remotely loaded, not copied
  into this repository; the dashboard uses a layout crop.
* Inter: Google Fonts, SIL Open Font License, with system-font fallbacks.
* Expanded portraits: ESPN headshots and Transfermarkt squad images. The latter
  require a unique normalized player name plus matching birth date. Alternate
  portraits are tried if the primary remote image fails. These are source-owned
  remote URLs, not a claim that every image is openly licensed.
* Czech league results: Openfootball's `europe/czech-republic` archives, with
  explicit team-ID mappings. Timed fixtures retain Europe/Prague civil time.

The dashboard is independent and does not claim official league/club affiliation.
The current snapshot includes 392 teams, 11,411 roster entries and more than
6,400 portrait links; counts can change with the next refresh. Not every player
has a photo, and remote image availability can change. A player represented in
both club and national squads is counted as two roster entries.

`competitions.json` is the 22-competition registry. `sync_match_centre.cjs`
imports extra ESPN leagues and national tournaments, player season totals,
headshots and dated match sheets. `enrich_match_centre.cjs` adds Czech coverage
and verified Transfermarkt squad/club details. `enrich_national_teams.cjs`
requires a unique senior-team match and provider-confirmed confederation before
attaching national managers and honours. Import issues and source records remain
saved alongside the snapshot.

## Model boundaries

UEFA results use only `STATUS_FULL_TIME` scores. AET/penalty results are skipped
because the score feed does not guarantee a separate 90-minute result. Scheduled
or postponed events have no score. Finalized future scores still cannot enter a
model fit before their date. Venue neutrality is preserved where supplied.

European histories cover calendar 2023 onward in the bundled snapshot. Each cup
has an independent fit; domestic strengths are not transferred across leagues.
Current main-competition participants populate the UI; earlier and qualifying
teams can still contribute training history. Unknown older club names include
an ESPN ID prefix to avoid collisions, which the UI removes for display.

Season player totals are separated by competition and never fed into historical
forecasts. Current profiles may be newer than the chosen model cutoff. Feed coach
records were found to contain undated historical names, so they are omitted.
Stadium details are taken from available home fixtures and are absent when not
verified. Domestic match results and player totals are different sources and can
have different coverage; source dates are visible in the interface.

## Transfermarkt history adapter

`transfermarkt.json` contains verified club IDs, available seasons, saved tables
and explicitly loaded club/player histories from
[felipeall/transfermarkt-api](https://github.com/felipeall/transfermarkt-api).
`transfermarkt-map.json` resolves provider naming differences with observed IDs.
Every mapped club must occur in its provider competition roster; player histories
require a unique squad-name match and the same date of birth. Failed identity
checks never attach someone else's history.

```sh
node tools/transfermarkt.cjs 2026
node tools/serve_dashboard.cjs dashboard.html
```

The local server can load additional histories from club/player pages and the
season selector. Requests are serialized with at least 1.65 seconds between
upstream calls. Raw successful responses are cached for 24 hours, while normalized
saved histories persist until explicitly refreshed. `node tools/transfermarkt.cjs
2026 --refresh` refreshes the season catalog and all already-saved histories;
rebuild the HTML afterwards. Failed requests leave successful older entries in
place. `updatedAt` remains the upstream fetch time, not the HTML build time.
Use `TRANSFERMARKT_API_URL` for a self-hosted deployment of the same service.

These historical profiles and current roster-based player probability estimates
never enter the historical team-model fits. Player estimates are an uncalibrated
squad-share heuristic, not a lineup-aware player model. The local match replay's
event times and contributions are synthetic; its final score is sampled from the
team forecast. The UEFA tables are group/league-phase standings, not a list of
knockout champions.

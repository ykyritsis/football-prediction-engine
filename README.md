# Football Prediction Engine

**Touchline** is a browser match centre for European football: predictions,
animated match simulations, club and player profiles, and historical evaluation.

[Open the public website](https://ykyritsis.github.io/football-prediction-engine/)

## Public website

The public edition runs on GitHub Pages from saved data. It needs no local
installation or backend, and all match simulations run in the browser.
Only bundled Transfermarkt histories are available on the public site;
additional histories can be collected locally and included in a later edition.
The forecast date and latest result dates are visible in the app.

Build the same website locally with Node 20+ and stable Rust:

```sh
node tools/build_website.cjs
```

The output is `dist/index.html`, with no server-side dependencies. See
[website publishing and data updates](docs/WEBSITE.md).

## Local dashboard

Generate a browser match centre for the Premier League, La Liga, Serie A,
Bundesliga, Ligue 1, Champions League, Europa League and Conference League:

```sh
cargo run --release -p predictor --bin dashboard -- 2026-10-08 dashboard.html
```

Open `dashboard.html` in any modern browser. Predictions and tables work offline,
with no server, API key or JavaScript build. Club/competition logos, credited
player photos and the Inter font load remotely. The date is an explicit **as-of
cutoff**, not an automatic assertion that the data is current. Update it when
generating a new snapshot. The optional third argument chooses the historical
evaluation start date (default: 90 days before the cutoff):

```sh
python tools/fetch_results.py 2026-27
node tools/sync_catalog.cjs --year=2026 --refresh
cargo run --release -p predictor --bin dashboard -- 2026-10-08 dashboard.html 2025-08-01
```

The fetch script prints failures and retains older files if a source is
unavailable. Check its output and the dashboard's latest-result dates before
treating forecasts as current. `PREDICTOR_DATA` overrides `data/results`; match
statistics are loaded from the adjacent `stats` directory. The result refresh
script does not refresh the separate statistics files.
`sync_catalog.cjs` refreshes public ESPN club profiles, competition-scoped player
statistics, kickoff/venue metadata and European fixtures/results. It also fetches
credited Wikimedia portraits for selected leading players, verifying identity
against the provider's date of birth. See `data/catalog/README.md` for coverage,
cache behavior and source limitations.

- **Upcoming fixtures:** league and club filters, score probabilities, home /
  draw / away chances, model expected goals, over 2.5 goals and both teams scoring.
- **Match simulator:** any two current-season clubs within one competition, with
  an animated clock, progressively revealed goals/scorers/assists, pause, skip
  and reduced-motion support. Each run uses a new random seed; optional fixed
  seeds reproduce both score and events. A collapsible 10,000-run comparison
  distinguishes a sampled match from the forecast distribution.
- **Club profiles:** crests, source-supplied stadium details, current-season result
  summaries, upcoming matches, full squads and recent results.
- **Player profiles:** nationality, age, shirt number, position, available goals,
  assists, appearances, shots, cards and goalkeeper stats. Values are scoped to
  the selected competition; missing stats remain missing. Verified portraits
  link the original creator, file and license. Profiles are descriptive, not
  additional inputs to the team prediction model. Upcoming matches include
  experimental anytime-goal/assist estimates from competition-specific squad
  shares, with position smoothing and a 70% assisted-goal prior. They have no
  confirmed lineup/minutes/injury inputs and are not calibrated player forecasts.
- **Recent form:** each club's last five available competition results, including
  opponent, venue and score on hover. Recency affects the underlying model
  through the base one-year half-life. The dashboard additionally applies
  opponent-adjusted last-five attack/defence residuals with six pseudo-goals,
  exponent 0.35 and strength caps of 0.85–1.15 (the base model spread still
  applies). Good/mixed/poor badges describe points per game, while the displayed
  numerical adjustment describes performance against expectations.
- **Past predictions:** green checks for correct 1X2 picks, red crosses for
  incorrect picks, gold exact-score badges, predicted versus actual scores, training
  cutoffs, outcome and exact-score hit rates, log loss and Brier score. Fits use
  only earlier matches and are reused for seven-day windows. Evaluation skips
  windows with fewer than 100 earlier competition results.

Historical rows are reconstructed backtests, not saved pre-match forecasts.
Model settings may have been tuned over overlapping history, so these are not
untouched holdout scores. Simulation samples the statistical score distribution.
This dashboard does not apply current
manual team-news adjustments, injuries or live lineups. European forecasts are
an experimental baseline fitted to each cup's history, including qualifying
rounds. They cover 90 minutes, not aggregate ties, qualification, extra time or
penalties. Neutral venues remove both league and club home advantage. The import
omits games ending after extra time or penalties when no regulation score is
provided; it does not mistake those final scores for 90-minute results.
It exposes stale data and unresolved past fixtures instead of silently treating
them as future matches. All future fixtures use strengths at the snapshot cutoff.

### Historical data through Transfermarkt

The adapter uses [felipeall/transfermarkt-api](https://github.com/felipeall/transfermarkt-api).
Run Node 20+ from the repository root:

```sh
node tools/transfermarkt.cjs 2026
cargo run --release -p predictor --bin dashboard -- 2026-10-08 dashboard.html
node tools/serve_dashboard.cjs dashboard.html
```

Open `http://127.0.0.1:8765/`. `Seasons & history` offers historical tables
(including cup group/league phases) and the bundled match archive. Club profiles
show season summaries and load honours/heritage. Player profiles load career
statistics, transfers, honours and a market-value timeline after a verified name
and birth-date match. The local server retrieves extra histories on demand;
saved enrichments are included in the next generated standalone HTML.

All 166 current clubs have checked Transfermarkt IDs. The bundled snapshot has
2025/26 tables for all eight competitions plus initial Arsenal/Tottenham and
Saka/Richarlison histories. It is not a downloaded history of every player.
Requests are serialized at least 1.65 seconds apart, and raw responses are cached
for 24 hours. Successful normalized enrichments persist in
`data/catalog/transfermarkt.json`; their upstream timestamps remain visible.
`TRANSFERMARKT_API_URL` can point at a self-hosted instance of the same API.
The default hosted API is intended for testing. Its code license does not grant
rights to Transfermarkt data; see the upstream project for usage terms. The
server binds only to loopback and is a local preview, not a production service.

The dashboard goal level is reconciled to the previous 90 days, using 40
pseudo-matches, a minimum of 20 real matches and a 0.90–1.15 multiplier cap.
It runs after the recent-form adjustment. Both use only earlier results in
backtests. The baseline CLI remains available without these dashboard additions.
A low most-likely exact score is not a low expected match total. These heuristic
additions need independent future validation; they do not establish better
predictive accuracy.

Validation:

```sh
cargo test -p predictor --locked
node --test crates/predictor/web/simulation.test.cjs
```

The exporter is `crates/predictor/src/bin/dashboard.rs`; its typed data assembly
is in `src/dashboard.rs` and the dependency-free browser UI is in `web/`.
Generated HTML is a build output and is not committed.

For a future live-data release, add a maintained fixtures/results ingestion service and
an immutable archive of forecasts with model versions. Lineup and injury feeds
and additional short-term form features should be evaluated on a later holdout
period before changing the default model.

## Command-line predictions

The Rust model also exposes match forecasts, fixture forecasts, season projections
and evaluation through the CLI:

```sh
cargo run --release -p predictor --bin predict -- match premier_league liverpool "man city" "M Oliver"
cargo run --release -p predictor --bin predict -- fixtures all 4
cargo run --release -p predictor --bin predict -- table premier_league
cargo run --release -p predictor --bin predict -- backtest all 2024-08-01
```

## Project structure

| Directory                                       | Purpose                                                             |
| ----------------------------------------------- | ------------------------------------------------------------------- |
| `crates/predictor`                              | Statistical forecasting model, dashboard exporter and browser UI    |
| `data/results`, `data/stats`                    | Saved match results and supporting statistics                       |
| `data/catalog`                                  | Clubs, squads, competition metadata and saved historical enrichment |
| `tools`                                         | Data refresh, historical adapters and website build                 |
| `crates/football_ai`, `crates/football_sandbox` | Optional tactical simulation library and visual development sandbox |

The tactical simulation is a separate development tool. Its ratings and
illustrative squads are not inputs to the statistical match forecasts.
See [tactical simulation documentation](docs/SIMULATION.md) for its setup and API.

Code is licensed under MIT OR Apache-2.0. Upstream football data, photography
and marks retain their separate source terms and attribution requirements.

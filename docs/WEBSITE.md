# Website publishing

The public Touchline website is a static build hosted on GitHub Pages:
https://football-prediction-engine-cyan.vercel.app/

The source repository stays private. The generated browser interface and its
saved football data are public. No API keys or server processes are included.

## Build and preview

Install stable Rust and Node 20+, then run from the repository root:

```sh
node tools/build_website.cjs
node tools/serve_dashboard.cjs dist/index.html
```

Open http://127.0.0.1:8765/. To choose another output directory:

```sh
node tools/build_website.cjs ./public-site
```

Deploy the whole output folder, including `assets/`, to any static web host.
The public build loads competition archives and team profiles on demand; preview
it over HTTP rather than opening `dist/index.html` as a file. The standalone
Rust dashboard export can still be opened directly.
The `--static` dashboard flag bundles saved historical
records and hides controls that would need the local historical API.
The season picker only offers years represented by saved tables or results.
Simulation remains interactive and uses a fresh random seed by default.

## Publish

The `Publish Touchline website` GitHub Actions workflow tests the predictor,
builds from committed data and deploys only `dist/` to GitHub Pages. It runs on
changes to the default branch `claude/great-cerf-lgzlzb`, or can be started
manually from Actions. In Settings → Pages, the publishing source is
**GitHub Actions**. The public project URL uses the new repository name.

The workflow uses the built-in short-lived GitHub token, with repository read
access for the build and Pages/OIDC write permissions only in the deploy job.
It has no third-party credentials. A separate scheduled workflow, `Refresh Touchline data`, runs daily. It fetches open results from openfootball, moves the forecast cutoff (`as_of`) to today, rebuilds, commits any changed data and starts this publish workflow. GitHub Actions usage is subject to the account's included
minutes; this does not subscribe to a paid hosting service.

## AI previews

Fixtures in the next 10 days get a short AI-written preview from the DeepSeek API, generated during the build by `tools/ai_previews.cjs`. Previews are cached in `data/ai/previews.json`, so each fixture is only generated once. The job needs a repository secret named `DEEPSEEK_API_KEY`. Without it, the site still builds and simply shows no previews.

## AI previews

Fixtures in the next 10 days get a short AI-written preview from the DeepSeek API, generated during the build by `tools/ai_previews.cjs`. Previews are cached in `data/ai/previews.json`, so each fixture is only generated once. The job needs a repository secret named `DEEPSEEK_API_KEY`. Without it, the site still builds and simply shows no previews.

## AI previews

Fixtures in the next 10 days get a short AI-written preview from the DeepSeek API, generated during the build by `tools/ai_previews.cjs`. Previews are cached in `data/ai/previews.json`, so each fixture is only generated once. The job needs a repository secret named `DEEPSEEK_API_KEY`. Without it, the site still builds and simply shows no previews.

## Update an edition

1. Refresh results and catalog data. Check failures and source timestamps.
   Refresh historical enrichment locally when adding tables or player careers.
2. Set `as_of` and `history_from` in `data/catalog/site.json`. The date is an
   explicit prediction cutoff; changing it alone does not refresh results.
3. Run `node tools/build_website.cjs` and inspect the latest-result notices,
   fixture coverage, simulations and historical comparisons.
4. Commit the saved data, edition metadata and any UI changes to the default
   branch. The workflow republishes the website after its checks pass.

Saved results and profiles have different timestamps. The current edition
clearly shows incomplete or older competition results. The site is not a
live-score feed and does not automatically move forecasts forward each day.
Recorded match sheets show dated historical starting XIs. Upcoming selections,
injuries and real-time updates need a later backend/data pipeline.

## Expanded match centre

The registry in `data/catalog/competitions.json` covers ten domestic leagues,
four club tournaments and eight national-team competitions. The added leagues
are Portugal, Netherlands, Belgium, Türkiye and Czechia. National coverage
includes the World Cup, Euros, UEFA Nations League, European World Cup qualifying,
Copa América, AFCON, Gold Cup and CONCACAF Nations League.

Refresh the expansion separately from the original results pipeline:

```sh
node tools/sync_match_centre.cjs
node tools/enrich_match_centre.cjs
node tools/enrich_national_teams.cjs
```

Check each command's issues and saved source records. Czech results use
Openfootball's European text archives; current squads come through Transfermarkt.
Portrait enrichment requires a unique name and birth-date match. The national
profile import also requires a confirmed national confederation. These scripts
preserve available older data when a provider fails.

Kickoffs use the browser's timezone, AM/PM and the corresponding local date,
including daylight saving. Civil Czech kickoff times carry an explicit
Europe/Prague timezone. Missing kickoff times remain `Time TBC`.

The replay defaults to five seconds. Formation controls choose an illustrative
XI; animated passing, shot events, possession and commentary are synthetic.
The final score is sampled from the team forecast, and contributor allocation
uses available players in that XI. Formations do not change team winning
probabilities. Reduced-motion users get the result without an animated replay.

## Source and artwork terms

Only the compiled app and football snapshots are deployed. Team/competition
marks and credited player portraits remain remotely served and retain their
source ownership and licensing. The app displays source links, photo credits
and data limitations. `felipeall/transfermarkt-api` is an adapter to historical
records; its code license does not grant rights to the upstream data.

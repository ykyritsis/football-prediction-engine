# Website publishing

The public Touchline website is a static build hosted on GitHub Pages:
https://ykyritsis.github.io/football-prediction-engine/

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

You can also open the generated HTML directly or deploy the output folder to
any static web host. The `--static` dashboard flag bundles saved historical
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
It has no third-party credentials. It is not scheduled and does not refresh
sources implicitly. GitHub Actions usage is subject to the account's included
minutes; this does not subscribe to a paid hosting service.

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
Lineups, injuries and real-time updates need a later backend/data pipeline.

## Source and artwork terms

Only the compiled app and football snapshots are deployed. Team/competition
marks and credited player portraits remain remotely served and retain their
source ownership and licensing. The app displays source links, photo credits
and data limitations. `felipeall/transfermarkt-api` is an adapter to historical
records; its code license does not grant rights to the upstream data.

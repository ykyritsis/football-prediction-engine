#!/usr/bin/env node
// Moves the prediction cutoff (site.json as_of) forward to today (UTC), so forecasts
// reflect the latest saved results. Run after tools/fetch_results.py.
const fs = require("node:fs");
const path = require("node:path");

const file = path.resolve(__dirname, "..", "data", "catalog", "site.json");
const site = JSON.parse(fs.readFileSync(file, "utf8"));
const today = new Date().toISOString().slice(0, 10);

if (site.as_of >= today) {
  console.log(`as_of ${site.as_of} is already current`);
} else {
  site.as_of = today;
  fs.writeFileSync(file, JSON.stringify(site, null, 2) + "\n");
  console.log(`as_of moved to ${today}`);
}

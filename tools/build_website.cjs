#!/usr/bin/env node
// Rebuild a public edition entirely from checked-in data. No live API or keys.
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const root = path.resolve(__dirname, "..");
const edition = require("../data/catalog/site.json");
const output = path.resolve(root, process.argv[2] || "dist");
fs.mkdirSync(output, { recursive: true });
const built = spawnSync(
  "cargo",
  [
    "run",
    "--locked",
    "--release",
    "-p",
    "predictor",
    "--bin",
    "dashboard",
    "--",
    edition.as_of,
    path.join(output, "index.html"),
    edition.history_from,
    "--static",
  ],
  { cwd: root, stdio: "inherit" },
);
if (built.error) throw built.error;
if (built.status !== 0) process.exit(built.status || 1);
console.log(require("./package_website.cjs").packageWebsite(output));
// AI previews are optional: a failure here must not stop the site from publishing.
require("./ai_previews.cjs").run(output).catch((err) => console.error("previews skipped:", err.message));
require("./ai_recaps.cjs").run(output).catch((err) => console.error("recaps skipped:", err.message));
require("./ai_extras.cjs").run(output).catch((err) => console.error("extras skipped:", err.message));
fs.mkdirSync(path.join(output, "assets"), { recursive: true });
const historySource = path.join(root, "data", "history", "standings.json");
if (fs.existsSync(historySource)) fs.copyFileSync(historySource, path.join(output, "assets", "history.json"));
fs.writeFileSync(path.join(output, ".nojekyll"), "");
fs.writeFileSync(
  path.join(output, "edition.json"),
  JSON.stringify(edition, null, 2) + "\n",
);
console.log(`Public website ready in ${output}`);

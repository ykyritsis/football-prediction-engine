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
fs.writeFileSync(path.join(output, ".nojekyll"), "");
fs.writeFileSync(
  path.join(output, "edition.json"),
  JSON.stringify(edition, null, 2) + "\n",
);
console.log(`Public website ready in ${output}`);

#!/usr/bin/env node
// Local-only snapshot server with a rate-limited, cached Transfermarkt adapter.
const http = require("node:http"),
  fs = require("node:fs"),
  path = require("node:path"),
  tm = require("./transfermarkt.cjs");
const file = path.resolve(process.argv[2] || "dashboard.html"),
  port = Number(process.env.PORT || 8765);
if (!fs.existsSync(file)) throw new Error("Generate the dashboard HTML first.");
http
  .createServer(async (req, res) => {
    const u = new URL(req.url, "http://localhost");
    res.setHeader("X-Content-Type-Options", "nosniff");
    if (req.method !== "GET") {
      res.writeHead(405);
      res.end();
      return;
    }
    if (u.pathname === "/") {
      res.writeHead(200, {
        "Content-Type": "text/html; charset=utf-8",
        "Cache-Control": "no-store",
      });
      fs.createReadStream(file).pipe(res);
      return;
    }
    if (!u.pathname.startsWith("/api/transfermarkt/")) {
      res.writeHead(404);
      res.end();
      return;
    }
    // Browser callers must be same-origin; do not expose this server on a public interface.
    if (
      req.headers.origin &&
      new URL(req.headers.origin).host !== req.headers.host
    ) {
      res.writeHead(403);
      res.end();
      return;
    }
    try {
      let result;
      if (u.pathname === "/api/transfermarkt/club")
        result = await tm.club(u.searchParams.get("name"));
      else if (u.pathname === "/api/transfermarkt/player")
        result = await tm.player(
          u.searchParams.get("club"),
          u.searchParams.get("id"),
        );
      else if (u.pathname === "/api/transfermarkt/table")
        result = await tm.table(
          u.searchParams.get("competition"),
          u.searchParams.get("season"),
        );
      else {
        res.writeHead(404);
        res.end();
        return;
      }
      res.writeHead(200, {
        "Content-Type": "application/json",
        "Cache-Control": "no-store",
      });
      res.end(JSON.stringify(result));
    } catch (e) {
      res.writeHead(502, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ error: e.message }));
    }
  })
  .listen(port, "127.0.0.1", () =>
    console.log(`Touchline: http://127.0.0.1:${port}/`),
  );

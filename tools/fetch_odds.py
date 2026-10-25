#!/usr/bin/env python3
"""Save bookmaker odds for the big-five leagues as match probabilities in data/odds/odds.json.

Source: football-data.co.uk (season file for played matches, fixtures.csv for the coming days).
Odds are the market average, falling back to Bet365 then Pinnacle, turned into probabilities
with the bookmaker margin removed. Clubs are matched to the names in data/results, and each
match is keyed "SEASON|Home|Away" (a pairing happens once a season in a league), so a shifted
kickoff date does not matter. Earlier entries are kept, because the odds published days
before a match are what the forecast should use.

usage: fetch_odds.py [SEASON]   (default: the current season, e.g. 2026-27)
"""
import csv
import datetime as dt
import difflib
import io
import json
import os
import re
import sys
import unicodedata
import urllib.request

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
LEAGUES = {"E0": "premier_league", "SP1": "la_liga", "I1": "serie_a", "D1": "bundesliga", "F1": "ligue_1"}
BASE = "https://www.football-data.co.uk"
# Names that football-data abbreviates beyond what fuzzy matching can resolve.
ALIASES = {
    "man united": "manchester united", "man city": "manchester city", "nott'm forest": "nottingham forest",
    "spurs": "tottenham hotspur", "wolves": "wolverhampton wanderers", "leeds": "leeds united",
    "newcastle": "newcastle united", "brighton": "brighton hove albion", "west ham": "west ham united",
    "ath madrid": "atletico madrid", "ath bilbao": "athletic club", "betis": "real betis",
    "sociedad": "real sociedad", "celta": "celta vigo", "vallecano": "rayo vallecano", "espanol": "espanyol",
    "inter": "internazionale", "milan": "ac milan", "bayern munich": "bayern munchen",
    "dortmund": "borussia dortmund", "m'gladbach": "borussia monchengladbach", "leverkusen": "bayer leverkusen",
    "ein frankfurt": "eintracht frankfurt", "hamburg": "hamburger", "st pauli": "st pauli",
    "paris sg": "paris saint germain", "marseille": "olympique marseille", "lyon": "olympique lyonnais",
    "stuttgart": "vfb stuttgart", "mainz": "mainz 05", "fc koln": "koln", "union berlin": "union berlin",
    "freiburg": "sc freiburg", "hoffenheim": "hoffenheim", "wolfsburg": "vfl wolfsburg", "werder bremen": "werder bremen",
    "heidenheim": "heidenheim", "augsburg": "augsburg", "lens": "racing club de lens", "rennes": "stade rennais", "psg": "paris saint germain",
}
DROP = {"fc", "afc", "cf", "ac", "as", "ss", "ssc", "sc", "rc", "rcd", "ud", "cd", "sd", "ca", "us", "ogc", "sv", "vfl", "vfb", "tsg", "fsv", "club", "de", "futbol", "calcio", "1907", "1899", "1846", "1900", "1909", "04", "05", "96", "98", "balompie", "1913", "1919"}


def norm(name):
    s = unicodedata.normalize("NFKD", name).encode("ascii", "ignore").decode().lower()
    s = ALIASES.get(s.replace(".", "").strip(), s)
    s = re.sub(r"[^a-z0-9 ]", " ", s.replace("&", " "))
    return " ".join(w for w in s.split() if w not in DROP)


def fetch(url):
    with urllib.request.urlopen(url, timeout=60) as r:
        return r.read().decode("utf-8-sig", errors="replace")


def probs(row):
    for cols in (("AvgH", "AvgD", "AvgA"), ("B365H", "B365D", "B365A"), ("PSH", "PSD", "PSA")):
        try:
            odds = [float(row[c]) for c in cols]
        except (KeyError, ValueError, TypeError):
            continue
        if all(o > 1 for o in odds):
            inv = [1 / o for o in odds]
            total = sum(inv)
            return [round(x / total, 4) for x in inv]
    return None


def team_map(league, season, names):
    """football-data name -> openfootball name, one to one, by normalised name similarity."""
    path = os.path.join(ROOT, "data", "results", season, f"{league}.json")
    with open(path, encoding="utf-8") as f:
        ours = sorted({t for m in json.load(f)["matches"] for t in (m["team1"], m["team2"])})
    pairs = sorted(
        ((difflib.SequenceMatcher(None, norm(n), norm(o)).ratio() + (0.5 if norm(n) in norm(o) or norm(o) in norm(n) else 0), n, o)
         for n in names for o in ours), reverse=True)
    mapping, used = {}, set()
    for score, n, o in pairs:
        if n not in mapping and o not in used and score >= 0.6:
            mapping[n] = o
            used.add(o)
    return mapping


def main():
    today = dt.date.today()
    year = today.year if today.month >= 7 else today.year - 1
    season = sys.argv[1] if len(sys.argv) > 1 else f"{year}-{(year + 1) % 100:02d}"
    code = season[2:4] + season[5:7]
    out_path = os.path.join(ROOT, "data", "odds", "odds.json")
    book = {}
    if os.path.exists(out_path):
        with open(out_path, encoding="utf-8") as f:
            book = json.load(f)
    try:
        fixtures = list(csv.DictReader(io.StringIO(fetch(f"{BASE}/fixtures.csv"))))
    except Exception as e:
        print(f"skip fixtures.csv: {e}")
        fixtures = []
    added = 0
    for div, league in LEAGUES.items():
        rows = []
        try:
            rows += list(csv.DictReader(io.StringIO(fetch(f"{BASE}/mmz4281/{code}/{div}.csv"))))
        except Exception as e:
            print(f"skip {league}: {e}")
        rows += [r for r in fixtures if r.get("Div") == div]
        rows = [r for r in rows if r.get("HomeTeam") and probs(r)]
        if not rows:
            continue
        names = {r[k] for r in rows for k in ("HomeTeam", "AwayTeam")}
        try:
            mapping = team_map(league, season, names)
        except FileNotFoundError:
            print(f"skip {league}: no results file for {season}")
            continue
        missing = sorted(names - set(mapping))
        if missing:
            print(f"{league}: no match for {missing}")
        entries = book.setdefault(league, {})
        for r in rows:
            h, a = mapping.get(r["HomeTeam"]), mapping.get(r["AwayTeam"])
            if h and a:
                key = f"{season}|{h}|{a}"
                p = probs(r)
                # Keep the first odds seen for a played match: later files hold closing odds.
                if key not in entries:
                    added += 1
                    entries[key] = p
                elif not r.get("FTHG"):
                    entries[key] = p  # an upcoming match: take the latest price
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    # One match per line keeps the daily diff small and readable.
    blocks = []
    for league, entries in sorted(book.items()):
        lines = ",\n".join(f"  {json.dumps(k, ensure_ascii=False)}: {json.dumps(v)}" for k, v in sorted(entries.items()))
        blocks.append(f"{json.dumps(league)}: {{\n{lines}\n}}")
    with open(out_path, "w", encoding="utf-8", newline="\n") as f:
        f.write("{\n" + ",\n".join(blocks) + "\n}\n")
    print(f"{added} new matches; totals " + ", ".join(f"{k} {len(v)}" for k, v in sorted(book.items())))


if __name__ == "__main__":
    main()

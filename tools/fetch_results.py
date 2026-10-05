#!/usr/bin/env python3
"""Download top-5-league results from openfootball (public domain) into data/results/.

usage: fetch_results.py [SEASON ...]   (default: 2019-20 .. 2026-27)
"""
import os
import sys
import urllib.request

BASE = "https://raw.githubusercontent.com/openfootball/football.json/master"
LEAGUES = {"en.1": "premier_league", "es.1": "la_liga", "it.1": "serie_a", "de.1": "bundesliga", "fr.1": "ligue_1"}
seasons = sys.argv[1:] or [f"{y}-{(y + 1) % 100:02d}" for y in range(2019, 2027)]
out = os.path.join(os.path.dirname(__file__), "..", "data", "results")
for season in seasons:
    os.makedirs(os.path.join(out, season), exist_ok=True)
    for code, name in LEAGUES.items():
        url = f"{BASE}/{season}/{code}.json"
        try:
            data = urllib.request.urlopen(url, timeout=30).read()
        except Exception as e:  # season not published yet
            print(f"skip {season} {name}: {e}")
            continue
        with open(os.path.join(out, season, f"{name}.json"), "wb") as f:
            f.write(data)
        print(f"{season} {name}: {len(data)} bytes")

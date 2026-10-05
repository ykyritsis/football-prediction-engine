#!/usr/bin/env python3
"""Validate a league file (see data/leagues/2011-12/FORMAT.md).

usage: validate_league.py FILE [EXPECTED_TEAMS]

Checks keys, enums and ranges, and that every starting XI fits its
formation with the same slot assignment the Rust loader uses.
"""
import json
import sys

FORMATIONS = {
    # (role, side) per slot; side +1 left, -1 right, 0 centre
    "F442": [("GK", 0), ("FB", 1), ("CB", 1), ("CB", -1), ("FB", -1), ("W", 1), ("CM", 1), ("CM", -1), ("W", -1), ("ST", 1), ("ST", -1)],
    "F433": [("GK", 0), ("FB", 1), ("CB", 1), ("CB", -1), ("FB", -1), ("DM", 0), ("CM", 1), ("CM", -1), ("W", 1), ("ST", 0), ("W", -1)],
    "F4231": [("GK", 0), ("FB", 1), ("CB", 1), ("CB", -1), ("FB", -1), ("DM", 1), ("DM", -1), ("W", 1), ("AM", 0), ("W", -1), ("ST", 0)],
    "F4141": [("GK", 0), ("FB", 1), ("CB", 1), ("CB", -1), ("FB", -1), ("DM", 0), ("W", 1), ("CM", 1), ("CM", -1), ("W", -1), ("ST", 0)],
    "F352": [("GK", 0), ("CB", 1), ("CB", 0), ("CB", -1), ("FB", 1), ("DM", 0), ("CM", 1), ("CM", -1), ("FB", -1), ("ST", 1), ("ST", -1)],
    "F532": [("GK", 0), ("FB", 1), ("CB", 1), ("CB", 0), ("CB", -1), ("FB", -1), ("CM", 1), ("DM", 0), ("CM", -1), ("ST", 1), ("ST", -1)],
}
POSITIONS = ["GK", "CB", "LB", "RB", "DMF", "CMF", "LMF", "RMF", "AMF", "LWF", "RWF", "SS", "CF"]
# Cost of putting a position into a slot role (missing = incompatible).
COST = {
    "GK": {"GK": 0},
    "CB": {"CB": 0, "DMF": 3, "LB": 4, "RB": 4},
    "FB": {"LB": 0, "RB": 0, "CB": 3, "LMF": 3, "RMF": 3, "DMF": 4},
    "DM": {"DMF": 0, "CMF": 1, "CB": 3, "AMF": 4},
    "CM": {"CMF": 0, "DMF": 1, "AMF": 1, "LMF": 3, "RMF": 3},
    "AM": {"AMF": 0, "SS": 1, "CMF": 2, "CF": 3, "LWF": 3, "RWF": 3, "LMF": 3, "RMF": 3},
    "W": {"LWF": 0, "RWF": 0, "LMF": 0, "RMF": 0, "AMF": 2, "SS": 2, "CF": 3, "LB": 4, "RB": 4, "CMF": 4},
    "ST": {"CF": 0, "SS": 1, "LWF": 2, "RWF": 2, "AMF": 3},
}
SLOT_ORDER = ["GK", "CB", "FB", "DM", "ST", "CM", "AM", "W"]


def side_of(pos):
    if pos.startswith("L"):
        return 1
    if pos.startswith("R"):
        return -1
    return 0


def slot_cost(slot, pos):
    role, side = slot
    c = COST[role].get(pos)
    if c is None:
        return None
    ps = side_of(pos)
    if side != 0 and ps != 0 and ps != side:
        c += 1
    return c


def assign(formation, xi):
    """Greedy slot assignment, mirrored in Rust (identity/database.rs)."""
    slots = FORMATIONS[formation]
    order = sorted(range(11), key=lambda i: (SLOT_ORDER.index(slots[i][0]), i))
    free = list(range(len(xi)))
    out = [None] * 11
    for si in order:
        best = None
        for pi in free:
            c = slot_cost(slots[si], xi[pi]["pos"])
            if c is not None and (best is None or c < best[0]):
                best = (c, pi)
        if best is None:
            return None, f"no player fits slot {slots[si]}"
        out[si] = best[1]
        free.remove(best[1])
    return out, None


ENUMS = {
    "attacking_style": ["CounterAttack", "PossessionGame"],
    "build_up": ["LongPass", "ShortPass"],
    "attacking_area": ["Wide", "Centre"],
    "positioning": ["MaintainFormation", "Flexible"],
    "defensive_style": ["FrontlinePressure", "AllOutDefence"],
    "containment_area": ["Middle", "Wide"],
    "pressuring": ["Aggressive", "Conservative"],
}
ATT = ["TikiTaka", "WingRotation", "FalseNine", "HugTheTouchline", "AttackingFullBacks", "FalseFullBacks", "CentringTargets"]
DEF = ["Gegenpress", "CounterTarget", "TightMarking", "DeepDefensiveLine", "SwarmTheBox", "Anchoring", "FlankProtection"]
PLAYING_STYLES = "GoalPoacher DummyRunner FoxInTheBox TargetMan CreativePlaymaker ProlificWinger RoamingFlank CrossSpecialist ClassicNo10 HolePlayer BoxToBox AnchorMan TheDestroyer ExtraFrontman OffensiveFullBack DefensiveFullBack BuildUp OffensiveGoalkeeper DefensiveGoalkeeper".split()
COM = "Trickster MazingRun SpeedingBullet IncisiveRun LongBallExpert EarlyCross LongRanger".split()
SKILLS = "ScissorsFeint DoubleTouch FlipFlap MarseilleTurn Sombrero CutBehindAndTurn ScotchMove HeelTrick Heading LongRangeCurler ChipShotControl KnuckleShot DippingShot RisingShot AcrobaticFinishing FirstTimeShot OneTouchPass ThroughPassing WeightedPass PinpointCrossing OutsideCurler Rabona NoLookPass LowLoftedPass GkLowPunt GkHighPunt LongThrow GkLongThrow PenaltySpecialist GkPenaltySaver Malicia ManMarking TrackBack Interception AcrobaticClear Captaincy SuperSub FightingSpirit".split()
RKEYS = "att ctl dri pas lob fin hea def tkl pow set spd acc bal str jmp sta".split()
GKEYS = "gk cat clr ref cov".split()


def check_player(p, where, errs):
    def e(msg):
        errs.append(f"{where} {p.get('name', '?')}: {msg}")

    for k in ["name", "pos", "age", "height", "foot", "r", "form", "inj", "wf"]:
        if k not in p:
            e(f"missing {k}")
            return
    if p["pos"] not in POSITIONS:
        e(f"bad pos {p['pos']}")
    if p["foot"] not in ("L", "R"):
        e("foot must be L or R")
    if not (15 <= p["age"] <= 45):
        e("age out of range")
    if not (155 <= p["height"] <= 210):
        e("height out of range")
    if not (1 <= p["form"] <= 8):
        e("form 1-8")
    if not (1 <= p["inj"] <= 3):
        e("inj 1-3")
    if not (1 <= p["wf"] <= 4):
        e("wf 1-4")
    r = p["r"]
    for k in RKEYS:
        v = r.get(k)
        if not isinstance(v, int) or not (40 <= v <= 99):
            e(f"r.{k} must be an int 40-99, got {v}")
    extra = set(r) - set(RKEYS)
    if extra:
        e(f"unknown rating keys {extra}")
    if p["pos"] == "GK":
        g = p.get("gk")
        if not isinstance(g, dict):
            e("keeper needs gk ratings")
        else:
            for k in GKEYS:
                v = g.get(k)
                if not isinstance(v, int) or not (40 <= v <= 99):
                    e(f"gk.{k} must be an int 40-99, got {v}")
    elif "gk" in p:
        e("only keepers have gk ratings")
    if p.get("style") is not None and p["style"] not in PLAYING_STYLES:
        e(f"bad style {p['style']}")
    for c in p.get("com", []):
        if c not in COM:
            e(f"bad com style {c}")
    for s in p.get("skills", []):
        if s not in SKILLS:
            e(f"bad skill {s}")


def main():
    path = sys.argv[1]
    expected = int(sys.argv[2]) if len(sys.argv) > 2 else None
    data = json.load(open(path, encoding="utf-8"))
    errs = []
    for k in ["league", "country", "season", "teams"]:
        if k not in data:
            errs.append(f"missing top-level {k}")
    teams = data.get("teams", [])
    if expected and len(teams) != expected:
        errs.append(f"expected {expected} teams, found {len(teams)}")
    positions = set()
    names = set()
    for t in teams:
        tn = t.get("name", "?")
        for k in ["name", "short", "manager", "real_finish", "formation", "style", "notes", "xi", "bench"]:
            if k not in t:
                errs.append(f"{tn}: missing {k}")
        if tn in names:
            errs.append(f"duplicate team {tn}")
        names.add(tn)
        rf = t.get("real_finish", {})
        pos = rf.get("position")
        positions.add(pos)
        if not isinstance(rf.get("points"), int):
            errs.append(f"{tn}: real_finish.points must be an int")
        if t.get("formation") not in FORMATIONS:
            errs.append(f"{tn}: bad formation {t.get('formation')}")
            continue
        st = t.get("style", {})
        for k, vals in ENUMS.items():
            if st.get(k) not in vals:
                errs.append(f"{tn}: style.{k} must be one of {vals}")
        for k in ["support_range", "defensive_line", "compactness"]:
            v = st.get(k)
            if not isinstance(v, int) or not 1 <= v <= 10:
                errs.append(f"{tn}: style.{k} must be 1-10")
        ins = st.get("instructions", [])
        a = [i for i in ins if i in ATT]
        d = [i for i in ins if i in DEF]
        if len(a) + len(d) != len(ins) or len(a) > 2 or len(d) > 2 or len(set(ins)) != len(ins):
            errs.append(f"{tn}: instructions must be at most 2 attacking + 2 defensive, valid names: {ins}")
        xi, bench = t.get("xi", []), t.get("bench", [])
        if len(xi) != 11:
            errs.append(f"{tn}: xi has {len(xi)} players")
        if len(bench) != 7:
            errs.append(f"{tn}: bench has {len(bench)} players")
        if sum(1 for p in xi if p.get("pos") == "GK") != 1:
            errs.append(f"{tn}: xi needs exactly one GK")
        if not any(p.get("pos") == "GK" for p in bench):
            errs.append(f"{tn}: bench needs a GK")
        pn = [p.get("name") for p in xi + bench]
        if len(set(pn)) != len(pn):
            errs.append(f"{tn}: duplicate player names")
        for p in xi:
            check_player(p, f"{tn} xi", errs)
        for p in bench:
            check_player(p, f"{tn} bench", errs)
        if len(xi) == 11 and all("pos" in p for p in xi):
            _, err = assign(t["formation"], xi)
            if err:
                errs.append(f"{tn}: XI does not fit {t['formation']}: {err}")
    if teams and positions != set(range(1, len(teams) + 1)):
        errs.append(f"real_finish positions must be 1..{len(teams)} exactly once")
    if errs:
        print(f"{path}: {len(errs)} problem(s)")
        for x in errs[:60]:
            print("  -", x)
        sys.exit(1)
    print(f"{path}: OK, {len(teams)} teams, {sum(len(t['xi']) + len(t['bench']) for t in teams)} players")


if __name__ == "__main__":
    main()

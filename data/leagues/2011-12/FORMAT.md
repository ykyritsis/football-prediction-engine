# League files (`<league>.json`)

One file per league and season. Ratings are **illustrative estimates** on a
40–99 simulation scale, written from public knowledge of each player at the
start of the season. They are not measured player statistics or prediction
inputs. Replace them with a documented data source before using them for
serious tactical analysis.

```jsonc
{
  "league": "La Liga",
  "country": "Spain",
  "season": "2011-12",
  "teams": [ Team, ... ]            // every club in the division that season
}
```

## Team

| key | meaning |
|---|---|
| `name`, `short` | Club name and a 3-letter code |
| `manager` | Manager at the start of the season |
| `real_finish` | `{ "position": 2, "points": 91 }` final league table, 2011-12 |
| `formation` | One of `F442`, `F433`, `F4231`, `F4141`, `F352`, `F532` |
| `style` | Team game plan (enums below) |
| `notes` | One sentence on how they played |
| `xi` | 11 players, the usual starting XI (exactly one GK) |
| `bench` | 7 players, at least one GK |

`style` keys: `attacking_style` (`CounterAttack` / `PossessionGame`),
`build_up` (`LongPass` / `ShortPass`), `attacking_area` (`Wide` / `Centre`),
`positioning` (`MaintainFormation` / `Flexible`), `support_range` 1–10 (1 =
close), `defensive_style` (`FrontlinePressure` / `AllOutDefence`),
`containment_area` (`Middle` / `Wide`), `pressuring` (`Aggressive` /
`Conservative`), `defensive_line` 1–10 (10 = high), `compactness` 1–10
(10 = compact), `instructions`: at most two attacking (`TikiTaka`,
`WingRotation`, `FalseNine`, `HugTheTouchline`, `AttackingFullBacks`,
`FalseFullBacks`, `CentringTargets`) and two defensive (`Gegenpress`,
`CounterTarget`, `TightMarking`, `DeepDefensiveLine`, `SwarmTheBox`,
`Anchoring`, `FlankProtection`).

## Player

| key | meaning |
|---|---|
| `name` | Full name as commonly known |
| `pos` | Position: `GK CB LB RB DMF CMF LMF RMF AMF LWF RWF SS CF` |
| `age`, `height` (cm), `foot` (`L`/`R`) | |
| `r` | 17 ratings, 40–99: `att` attacking awareness, `ctl` ball control, `dri` dribbling, `pas` low pass, `lob` lofted pass, `fin` finishing, `hea` heading, `def` defensive awareness, `tkl` ball winning, `pow` kicking power, `set` place kicking, `spd` speed, `acc` explosive power, `bal` body balance, `str` physical contact, `jmp` jump, `sta` stamina |
| `gk` | Keepers only: `{ "gk", "cat", "clr", "ref", "cov" }` (goalkeeping, catching, clearing, reflexes, coverage) |
| `form` | Consistency 1–8 |
| `inj` | Injury resistance 1–3 |
| `wf` | Weak foot 1–4 |
| `style` | Optional playing style |
| `com` | Optional COM styles |
| `skills` | Optional skills |

The loader places the XI into formation slots by position and side, so the
order of `xi` does not matter.

# Team news adjustments

`predict fixtures`, `match` and `table` read `data/adjustments/<league>.json`
and shift the fitted strengths before predicting. Use it for what results
cannot know yet: injuries, suspensions, a new manager, a squad sold off.

```json
{
  "Arsenal FC": { "attack": -0.08, "defence": 0.0, "until": "2026-11-01", "note": "Saka out (hamstring)" }
}
```

* `attack` / `defence` are on the model's log scale: +0.10 is about 10% more
  goals scored (attack) or 10% fewer conceded (defence). For scale, the best
  attacks are about +0.6 and a promoted side about -0.3.
* Rough guide for one absence: a star forward -0.05 to -0.10 attack; a first-choice
  keeper or centre-back -0.03 to -0.06 defence; a squad player 0 to -0.02.
* `until` (optional, ISO date): ignored on and after that date.
* Club names are the ones `predict` prints (openfootball names).

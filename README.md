# pes18-recomp — football AI

A deterministic, headless football simulation and tactical decision layer in
Rust, built as the "brain" for a PES 2018-style game, plus a Bevy sandbox for
inspecting it.

![Wide in the final third: box runs, overlap, scored pass options](docs/images/wide_final_third.png)

| Crate | What it is |
|---|---|
| [`crates/football_ai`](crates/football_ai) | Zero-dependency library: 22 players + ball, formations, phases, pressing, marking, support runs, passing evaluation, role behaviour, team/player identity, fixed-step simulation |
| [`crates/football_sandbox`](crates/football_sandbox) | Bevy 0.19 app that runs the simulation and draws every decision |

## Quick start

```sh
cargo test -p football_ai                                         # ~100 tests, a few seconds
cargo run --release -p football_ai --example headless_match -- 7 90 tiki-taka long-ball
cargo run --release -p football_ai --example style_report -- 90 2  # round-robin of team styles
cargo run -p football_ai --example describe                        # generated team/player descriptions
cargo run --release -p football_sandbox                            # visual sandbox
```

On Linux the sandbox uses X11 by default (works under XWayland); build with
`--features wayland` for native Wayland (needs `libwayland-dev`).
`SANDBOX_CAPTURE=<dir> football_sandbox` captures a screenshot of every preset and exits.

## What the AI does

Every tick, each team builds a **team plan** from an immutable snapshot, then
each player makes a **decision** from that snapshot and his team's plan.

| Behaviour | Module | Summary |
|---|---|---|
| Formation-relative positioning | `formation` | Slots in normalised block coordinates, stretched per phase: line height follows ball and phase, compact/wide in and out of possession, slides to the ball, holds onside, drops off when the passer has time, sweeper-keeper |
| Possession phases | `phase` | Build-up / progression / final third / attacking transition, and high press / mid block / low block / defensive transition, with hysteresis |
| Pressing triggers | `pressing` | Backward pass, receiving touch, facing own goal, touchline, isolated carrier, press zone, slow build-up, danger zone, counter-press. Intensity decides press vs contain; cover-shadow player; gegenpress swarm |
| Defensive marking | `marking` | Zonal-man hybrid, deterministic greedy assignment, positional leashes, goal-side anticipation, tightness by danger and instructions |
| Support runs | `support` | Rest defence, near/far post, penalty spot, cut-back, runs in behind timed to the passer, overlaps/underlaps, showing for the ball into free pockets, holding width |
| Passing evaluation | `passing` | Short / through / long / cross candidates; interception risk from lane timing (lofted balls only contestable at landing, aerial duels), misplacement, receiver pressure, progression, lines broken, offside excluded |
| On-ball choice | `decision`, `eval` | Pass vs shot (xG) vs carry vs shield on one expected-value scale, with identity as style utility |
| Simulation | `sim` | Fixed step, steering, rolling and lofted ball flight, tackles, saves, offside, throw-ins, corners, goal kicks, kick-offs, event log |

## Team and player identity

The `identity` module mirrors PES 2017/2018 so the full game can be filled in
directly from the extracted database:

* **Players**: 40–99 `Ratings`, a `PlayingStyle` (Goal Poacher, Creative
  Playmaker, Anchor Man, …), `ComStyle`s (Mazing Run, Long Ranger, Early
  Cross, …) and `Skill`s (Double Touch, Through Passing, Man Marking, …). Each
  trait has a description and a concrete effect on decisions.
  `PlayerProfile::behaviour_in(role)` layers them over the role defaults, so a
  dribbler stays a dribbler in any position.
* **Teams**: `TeamStyle` is the PES game plan (attacking style, build-up,
  attacking area, positioning, support range, defensive style, containment,
  pressuring, defensive line, compactness, advanced instructions such as
  Tiki-Taka, Gegenpress, Counter Target, False No. 9). It compiles to numeric
  `Tactics`.
* **Descriptions** are generated: `TeamStyle::describe()`,
  `PlayerProfile::describe()`, `signature_traits()`.

```text
L. Messi (W) — Creative Playmaker. Creates chances out of nothing with vision, dribbling and incisive
passing. Standout abilities: ball control 97, dribbling 97, balance 95. On the ball: Trickster, Mazing Run,
Long Ranger. In matches: Takes defenders on; Clinical finisher; Shoots from distance; Plays killer through balls.
```

`presets` has six team archetypes, twenty player archetypes, generated
squads that suit each style, and two example star profiles. The star
profiles' ratings are illustrative placeholders: real values should come from
the game data.

### Does identity change the football?

Round-robin of the archetypes (`style_report`, 90 min, 2 seeds per pairing):

| Style | Poss | Avg pass | Long % | Crosses | Shots | PPDA |
|---|---|---|---|---|---|---|
| Tiki-Taka | 59% | 19.6 m | 26% | 3 | 42 | 1.20 |
| Gegenpress | 48% | 27.6 m | 44% | 8 | 37 | 1.26 |
| Route One | 45% | 32.0 m | 64% | 10 | 9 | 2.02 |
| Counter-Attack | 51% | 28.6 m | 44% | 15 | 17 | 1.72 |
| Wing Play | 57% | 23.8 m | 34% | 5 | 11 | 1.57 |
| Park the Bus | 40% | 30.9 m | 51% | 1 | 7 | 2.81 |

Tests pin the important contrasts: long-ball teams play longer, possession
teams keep the ball, pressing teams win it higher, dribblers carry more,
finishers rate chances higher.

## Determinism

* One seeded RNG, consumed only by the simulation, in a fixed order.
* Planning and decisions are pure functions of the snapshot, so their order
  does not matter (tested by deciding in reverse order).
* Every sort has an explicit tie-break; there are no hash maps; search
  directions are constant tables.
* `MatchState::state_hash()` is bit-exact; tests run two simulations side by
  side and compare hashes every tick.

Results are identical on the same platform and build. A few `exp`/`atan2`
calls go through the platform maths library; for cross-platform lockstep
networking, swap those for a portable implementation.

## Integrating with Bevy ECS

The decision layer is two pure functions:

```rust
plan_team(&MatchState, TeamSide) -> TeamPlan
decide(&MatchState, &TeamPlan, PlayerId) -> Decision
```

A game maps them onto systems:

```rust
// 1. Copy components into a snapshot (22 players + ball is a tiny copy).
fn snapshot(q: Query<(&Slot, &Transform, &Velocity, &Identity)>, ball: Res<BallState>, mut snap: ResMut<Snapshot>) { /* fill MatchState */ }
// 2. One plan per team.
fn plan(snap: Res<Snapshot>, mut plans: ResMut<TeamPlans>) {
    plans.0 = [plan_team(&snap.0, TeamSide::Home), plan_team(&snap.0, TeamSide::Away)];
}
// 3. Decisions in parallel: they only read the snapshot.
fn decide_players(snap: Res<Snapshot>, plans: Res<TeamPlans>, mut q: Query<(&Slot, &mut DecisionC)>) {
    q.par_iter_mut().for_each(|(slot, mut d)| d.0 = decide(&snap.0, &plans.0[slot.0.team().index()], slot.0));
}
// 4. Locomotion / animation / ball physics consume `DecisionC` (see `sim.rs` for a reference).
```

The sandbox keeps the whole `Simulation` in a resource instead, so what you
see is exactly what the headless tests run.

## Sandbox

![Build-up under a high press](docs/images/build_up_under_press.png)

| Key | Action |
|---|---|
| Space / → or `.` | pause / single step |
| `+` / `-` | speed ×2 / ÷2 |
| `1`–`6` | presets: kick-off, build-up under press, midfield progression, wide final third, counter-attack, low block |
| `R` / `N` | reset preset / new seed |
| `H` / `A` | cycle home / away team style |
| `X` | toggle the example star players in the home team |
| Click / Tab / Esc | select player / cycle / clear |
| `T` `S` `M` `P` `U` `C` `O` | overlays: movement targets, formation shape, marking, pressing and cover, support runs, pass options, offside lines |

Overlay colours: yellow = marking (brighter = tighter), red = press/contain,
orange = cover shadow, cyan = support runs, green→red lines = the carrier's
pass candidates ranked, purple arrow = best carry. The HUD shows each team's
phase, pressing mode and triggers, active runs, the carrier's scored options,
the selected player's identity and behaviour numbers, and recent events.

## Status and roadmap

Calibration is ongoing. Styles separate clearly, but attacking styles still
create more shots and goals than real football, and crossing volume is low.
`style_report` and the SVG snapshot renderer (`football_ai::svg`) are the
tuning tools.

Next:

1. Match rules engine: fouls and advantage, cards, free kicks and penalties, halves.
2. 3D ball physics: height, spin and curl, bounces, headers, posts.
3. Fill identities from the extracted PES 2018 database.
4. Calibrate against PES 2018 match statistics.

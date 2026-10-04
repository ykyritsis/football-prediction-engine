//! # football_ai
//!
//! Deterministic, headless football simulation and tactical decision layer:
//! 22 players and a ball, formation-relative positioning, possession phases,
//! marking, pressing triggers, support runs, passing evaluation and
//! role/identity-specific behaviour.
//!
//! The crate has **no dependencies** and no global state. It is built to be
//! dropped into a Bevy ECS game later without changes to the decision code.
//!
//! ## Layers
//!
//! | Layer | Modules | Nature |
//! |---|---|---|
//! | Data | [`state`], [`pitch`], [`math`] | plain structs, no behaviour |
//! | Identity | [`identity`], [`role`], [`tactics`], [`formation`] | data + compile-to-numbers |
//! | Team plan | [`plan`] (uses [`phase`], [`pressing`], [`marking`], [`support`]) | pure fn of snapshot |
//! | Player decision | [`decision`] (uses [`passing`], [`eval`], [`ball`]) | pure fn of snapshot + plan |
//! | Simulation | [`sim`] | applies decisions; owns the RNG |
//!
//! ## Determinism
//!
//! * One seeded [`rng::SimRng`], consumed in a fixed order by [`sim`] only.
//! * Decisions read an immutable snapshot, so their order does not matter.
//! * Every sort has an explicit tie-break (usually player id); no hash maps.
//! * Directions used in searches are constant tables, not runtime trig.
//!
//! Identical config ⇒ identical [`state::MatchState::state_hash`] every tick
//! on the same platform and build. The few transcendental calls (`exp`,
//! `atan2`) go through the platform maths library; for cross-platform
//! lockstep networking swap those for a portable implementation.
//!
//! ## Bevy ECS integration
//!
//! The decision layer is two pure functions over a snapshot:
//!
//! ```text
//! plan_team(&MatchState, TeamSide) -> TeamPlan
//! decide(&MatchState, &TeamPlan, PlayerId) -> Decision
//! ```
//!
//! A Bevy game maps them onto systems like this:
//!
//! | ECS | football_ai |
//! |---|---|
//! | `Player` entity with `Transform`, `Velocity`, `PlayerIdentity(PlayerProfile)`, `Slot(PlayerId)` | [`state::Player`] |
//! | `Ball` entity / resource | [`state::Ball`] |
//! | `Team` resource / entity with `TeamStyle` | [`state::TeamState`] |
//! | `snapshot_system` (copies components into a `Res<MatchState>`; 22 players is a trivial copy) | [`state::MatchState`] |
//! | `team_plan_system` → `Res<TeamPlans>` | [`plan::plan_team`] |
//! | `decision_system` (`par_iter_mut` over players) → `Decision` component | [`decision::decide`] |
//! | animation/locomotion/physics systems consume `Decision` | your game (or [`sim`] as reference) |
//!
//! Because decisions only read the snapshot, `decision_system` can use
//! `Query::par_iter_mut` safely. The included [`sim::Simulation`] is the
//! reference implementation of the "consume decisions" half and is what the
//! tests and the sandbox run.

pub mod ball;
pub mod decision;
pub mod eval;
pub mod formation;
pub mod identity;
pub mod marking;
pub mod math;
pub mod passing;
pub mod phase;
pub mod pitch;
pub mod plan;
pub mod pressing;
pub mod rng;
pub mod role;
pub mod sim;
pub mod state;
pub mod support;
pub mod svg;
pub mod tactics;

pub use decision::{decide, Action, Activity, Decision, MoveSpeed};
pub use identity::{PlayerProfile, Squad, TeamStyle};
pub use plan::{plan_team, TeamPlan};
pub use sim::{MatchEvent, SimConfig, Simulation};
pub use state::{MatchState, PlayerId};

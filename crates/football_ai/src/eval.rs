//! Shared value model for on-ball decisions.
//!
//! Every on-ball option (pass, dribble, shot, hold) is scored as an expected
//! value in "goal probability" units:
//!
//! ```text
//! EV = P(success) * value(outcome) - P(failure) * turnover_cost(where it fails) * risk_aversion
//!      + style utility
//! ```
//!
//! `value` is a cheap possession-value surface (how likely possessing the ball
//! at a point leads to a goal), a turnover's cost is the same surface seen from
//! the opponent's side, and a shot's value is its xG. Style utilities are
//! small bonuses that express team/player identity (a long-ball team gets a
//! bonus for long passes, a dribbler for carries) on the same scale.

use crate::math::Vec2;
use crate::pitch::{zone_value_local, TeamSide, GOAL_HALF_WIDTH, HALF_LENGTH};
use crate::state::{MatchState, Player};

/// Scale from the `[0, 1]` zone surface to goal probability.
pub const POSSESSION_VALUE_SCALE: f32 = 0.3;

/// Value of possessing the ball at a team-local point.
#[inline]
pub fn possession_value(local: Vec2) -> f32 {
    POSSESSION_VALUE_SCALE * zone_value_local(local)
}

/// Cost of losing the ball at a team-local point: the opponent's possession
/// value there (their local frame is ours rotated 180°), plus a constant for
/// the loss of our own possession.
#[inline]
pub fn turnover_cost(local: Vec2) -> f32 {
    0.04 + POSSESSION_VALUE_SCALE * zone_value_local(-local)
}

/// Pressure on a point from `side`'s opponents, `0` (free) to `1` (closed down).
pub fn pressure_at(state: &MatchState, side: TeamSide, p: Vec2) -> f32 {
    let d = state.nearest_distance(side.opponent(), p);
    (1.0 - (d - 1.0) / 6.0).clamp(0.0, 1.0)
}

/// Effective risk aversion for a player: role/identity × team instruction.
pub fn risk_aversion(state: &MatchState, player: &Player) -> f32 {
    player.behaviour.risk_aversion * state.team(player.team).tactics.risk_aversion
}

/// Angle (radians) subtended by the goal mouth from a team-local point.
pub fn goal_angle(local: Vec2) -> f32 {
    let a = Vec2::new(HALF_LENGTH, GOAL_HALF_WIDTH) - local;
    let b = Vec2::new(HALF_LENGTH, -GOAL_HALF_WIDTH) - local;
    let cross = a.perp_dot(b).abs();
    let dot = a.dot(b);
    cross.atan2(dot)
}

/// Simple expected-goals model from distance and angle only.
pub fn base_xg(local: Vec2) -> f32 {
    if local.x >= HALF_LENGTH {
        return 0.0;
    }
    let dist = local.distance(Vec2::new(HALF_LENGTH, 0.0));
    let angle = goal_angle(local);
    (1.15 * angle * (-dist / 10.0).exp()).clamp(0.0, 0.9)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xg_falls_with_distance_and_angle() {
        let close = base_xg(Vec2::new(HALF_LENGTH - 11.0, 0.0));
        let far = base_xg(Vec2::new(HALF_LENGTH - 25.0, 0.0));
        let wide = base_xg(Vec2::new(HALF_LENGTH - 11.0, 18.0));
        assert!(close > far && close > wide, "{close} {far} {wide}");
        assert!(close > 0.15 && close < 0.4, "penalty spot xg {close}");
        assert!(far < 0.05, "25m xg {far}");
    }

    #[test]
    fn turnover_is_costlier_near_own_goal() {
        assert!(turnover_cost(Vec2::new(-45.0, 0.0)) > turnover_cost(Vec2::new(45.0, 0.0)));
    }

    #[test]
    fn possession_value_higher_near_goal() {
        assert!(possession_value(Vec2::new(40.0, 0.0)) > possession_value(Vec2::new(-40.0, 0.0)));
    }
}

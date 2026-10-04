//! Pitch geometry and team-relative coordinate frames.
//!
//! World space: origin at the centre spot, `x` along the length of the pitch,
//! `y` across it, metres. [`TeamSide::Home`] attacks `+x`, [`TeamSide::Away`]
//! attacks `-x`.
//!
//! Most tactical reasoning happens in a *team-local* frame in which the team
//! always attacks `+x`. The local frame is the world frame rotated by 180
//! degrees for the away side (both axes negated) so that handedness is kept:
//! a team's left-back is on the `+y` side of its local frame regardless of
//! which way it is attacking.

use crate::math::Vec2;

pub const PITCH_LENGTH: f32 = 105.0;
pub const PITCH_WIDTH: f32 = 68.0;
pub const HALF_LENGTH: f32 = PITCH_LENGTH / 2.0;
pub const HALF_WIDTH: f32 = PITCH_WIDTH / 2.0;
pub const GOAL_HALF_WIDTH: f32 = 7.32 / 2.0;
pub const PENALTY_AREA_DEPTH: f32 = 16.5;
pub const PENALTY_AREA_HALF_WIDTH: f32 = 40.32 / 2.0;
pub const CENTRE_CIRCLE_RADIUS: f32 = 9.15;

/// The two teams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TeamSide {
    Home,
    Away,
}

impl TeamSide {
    pub const BOTH: [TeamSide; 2] = [TeamSide::Home, TeamSide::Away];

    #[inline]
    pub fn opponent(self) -> TeamSide {
        match self {
            TeamSide::Home => TeamSide::Away,
            TeamSide::Away => TeamSide::Home,
        }
    }

    /// `+1` for the side attacking `+x`, `-1` otherwise.
    #[inline]
    pub fn direction(self) -> f32 {
        match self {
            TeamSide::Home => 1.0,
            TeamSide::Away => -1.0,
        }
    }

    #[inline]
    pub fn index(self) -> usize {
        match self {
            TeamSide::Home => 0,
            TeamSide::Away => 1,
        }
    }

    /// World → team-local (team attacks `+x`).
    #[inline]
    pub fn to_local(self, world: Vec2) -> Vec2 {
        world * self.direction()
    }

    /// Team-local → world.
    #[inline]
    pub fn to_world(self, local: Vec2) -> Vec2 {
        local * self.direction()
    }

    /// Centre of the goal this team defends, in world space.
    #[inline]
    pub fn own_goal(self) -> Vec2 {
        Vec2::new(-HALF_LENGTH * self.direction(), 0.0)
    }

    /// Centre of the goal this team attacks, in world space.
    #[inline]
    pub fn target_goal(self) -> Vec2 {
        Vec2::new(HALF_LENGTH * self.direction(), 0.0)
    }
}

/// Is a world-space point within the field of play?
#[inline]
pub fn in_bounds(p: Vec2) -> bool {
    p.x.abs() <= HALF_LENGTH && p.y.abs() <= HALF_WIDTH
}

/// Clamp a world-space point into the field of play with a margin.
#[inline]
pub fn clamp_to_pitch(p: Vec2, margin: f32) -> Vec2 {
    Vec2::new(
        p.x.clamp(-HALF_LENGTH + margin, HALF_LENGTH - margin),
        p.y.clamp(-HALF_WIDTH + margin, HALF_WIDTH - margin),
    )
}

/// Is a team-local point inside the opponent's penalty area?
#[inline]
pub fn in_attacking_box_local(local: Vec2) -> bool {
    local.x >= HALF_LENGTH - PENALTY_AREA_DEPTH && local.y.abs() <= PENALTY_AREA_HALF_WIDTH
}

/// Is a team-local point inside the team's own penalty area?
#[inline]
pub fn in_own_box_local(local: Vec2) -> bool {
    local.x <= -HALF_LENGTH + PENALTY_AREA_DEPTH && local.y.abs() <= PENALTY_AREA_HALF_WIDTH
}

/// Pitch thirds from a team's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Third {
    Defensive,
    Middle,
    Attacking,
}

/// Which third of the pitch a team-local `x` lies in.
#[inline]
pub fn third_of(local_x: f32) -> Third {
    let t = PITCH_LENGTH / 6.0;
    if local_x < -t {
        Third::Defensive
    } else if local_x > t {
        Third::Attacking
    } else {
        Third::Middle
    }
}

/// Simple positional value of possessing the ball at a team-local point, in
/// `[0, 1]`. Grows towards the opponent goal and towards the centre in the
/// final third (a cheap "expected threat" surface).
pub fn zone_value_local(local: Vec2) -> f32 {
    let along = ((local.x + HALF_LENGTH) / PITCH_LENGTH).clamp(0.0, 1.0);
    let goal = Vec2::new(HALF_LENGTH, 0.0);
    let d = local.distance(goal);
    let proximity = (1.0 - d / 40.0).clamp(0.0, 1.0);
    let centrality = 1.0 - (local.y.abs() / HALF_WIDTH).clamp(0.0, 1.0);
    (0.45 * along * along + 0.45 * proximity * proximity * (0.5 + 0.5 * centrality) + 0.1 * centrality * along)
        .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_frame_round_trip() {
        let p = Vec2::new(12.0, -7.0);
        for side in TeamSide::BOTH {
            assert_eq!(side.to_world(side.to_local(p)), p);
        }
        assert_eq!(TeamSide::Away.to_local(p), Vec2::new(-12.0, 7.0));
    }

    #[test]
    fn goals_are_opposite() {
        assert_eq!(TeamSide::Home.target_goal(), TeamSide::Away.own_goal());
        assert_eq!(TeamSide::Home.to_local(TeamSide::Home.target_goal()).x, HALF_LENGTH);
        assert_eq!(TeamSide::Away.to_local(TeamSide::Away.target_goal()).x, HALF_LENGTH);
    }

    #[test]
    fn thirds() {
        assert_eq!(third_of(-40.0), Third::Defensive);
        assert_eq!(third_of(0.0), Third::Middle);
        assert_eq!(third_of(40.0), Third::Attacking);
    }

    #[test]
    fn zone_value_increases_towards_goal() {
        let a = zone_value_local(Vec2::new(-40.0, 0.0));
        let b = zone_value_local(Vec2::new(0.0, 0.0));
        let c = zone_value_local(Vec2::new(40.0, 0.0));
        assert!(a < b && b < c, "{a} {b} {c}");
        // Central beats wide at the same depth in the final third.
        assert!(zone_value_local(Vec2::new(40.0, 0.0)) > zone_value_local(Vec2::new(40.0, 30.0)));
    }
}

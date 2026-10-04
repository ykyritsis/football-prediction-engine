//! Ball kinematics and interception maths.
//!
//! The ball rolls on the ground plane with constant deceleration, which keeps
//! travel times analytic: the decision layer can predict exactly where a pass
//! will be at any time, and the simulation integrates the same model.
//!
//! Lofted passes, crosses and clearances fly at constant horizontal speed for
//! their airborne distance (nobody can play the ball then), lose most of their
//! pace on landing ([`LANDING_DAMPING`]) and roll from there.

use crate::math::Vec2;
use crate::state::Player;

/// Rolling deceleration, m/s².
pub const BALL_DECEL: f32 = 3.2;
/// Speed (m/s) a ground pass should arrive at its target with.
pub const PASS_ARRIVAL_SPEED: f32 = 11.0;
/// Arrival speed for lofted passes (they drop and slow sharply on landing).
pub const LOFTED_ARRIVAL_SPEED: f32 = 4.0;
pub const MIN_PASS_SPEED: f32 = 11.0;
pub const MAX_PASS_SPEED: f32 = 32.0;
pub const SHOT_SPEED: f32 = 28.0;
/// Fraction of a lofted pass's length during which it is above head height
/// and cannot be intercepted.
pub const LOFT_SAFE_FRACTION: f32 = 0.88;
/// Fraction of horizontal speed kept when a lofted ball lands.
pub const LANDING_DAMPING: f32 = 0.35;
/// Reaction time before a player starts moving towards a ball.
pub const REACTION_TIME: f32 = 0.2;
/// Distance at which a player can play the ball.
pub const CONTROL_RADIUS: f32 = 1.1;

/// Initial speed so that a ball rolls `distance` metres and arrives at `arrival` m/s.
pub fn pass_speed_for(distance: f32, arrival: f32) -> f32 {
    (arrival * arrival + 2.0 * BALL_DECEL * distance.max(0.0))
        .sqrt()
        .clamp(MIN_PASS_SPEED, MAX_PASS_SPEED)
}

/// Horizontal speed for a lofted ball travelling `distance` metres. Longer
/// balls hang longer but travel faster overall.
pub fn lofted_speed(distance: f32) -> f32 {
    let flight = 0.8 + distance / 32.0;
    (distance / flight).clamp(8.0, 26.0)
}

/// Time for a lofted ball at horizontal speed `speed` to reach a point
/// `distance` along its path (only meaningful within the airborne part).
pub fn lofted_time(speed: f32, distance: f32) -> f32 {
    distance / speed.max(1.0)
}

/// Time for a ball struck at `v0` to cover `distance`, or `None` if it stops first.
pub fn travel_time(v0: f32, distance: f32) -> Option<f32> {
    let disc = v0 * v0 - 2.0 * BALL_DECEL * distance;
    if disc < 0.0 {
        None
    } else {
        Some((v0 - disc.sqrt()) / BALL_DECEL)
    }
}

/// Time until a ball moving at `speed` comes to rest.
pub fn stop_time(speed: f32) -> f32 {
    speed / BALL_DECEL
}

/// Distance a ball moving at `speed` rolls before stopping.
pub fn stop_distance(speed: f32) -> f32 {
    speed * speed / (2.0 * BALL_DECEL)
}

/// Predicted ball position after `t` seconds, accounting for a remaining
/// airborne distance `air`. Also returns whether the ball is still in the air.
pub fn predict_flight(pos: Vec2, vel: Vec2, air: f32, t: f32) -> (Vec2, bool) {
    let speed = vel.length();
    if air <= 0.0 || speed < 1e-4 {
        return (predict(pos, vel, t), false);
    }
    let t_air = air / speed;
    if t < t_air {
        return (pos + vel * t, true);
    }
    let land = pos + vel * t_air;
    (predict(land, vel * LANDING_DAMPING, t - t_air), false)
}

/// Predicted position of a rolling ball after `t` seconds.
pub fn predict(pos: Vec2, vel: Vec2, t: f32) -> Vec2 {
    let speed = vel.length();
    if speed < 1e-4 {
        return pos;
    }
    let t = t.min(stop_time(speed));
    let d = speed * t - 0.5 * BALL_DECEL * t * t;
    pos + vel / speed * d
}

/// Advance ball velocity by one step of rolling friction.
pub fn apply_friction(vel: Vec2, dt: f32) -> Vec2 {
    let speed = vel.length();
    if speed <= BALL_DECEL * dt {
        Vec2::ZERO
    } else {
        vel * ((speed - BALL_DECEL * dt) / speed)
    }
}

/// Estimated time for `player` to get within control range of `point`.
pub fn time_to_reach(player: &Player, point: Vec2) -> f32 {
    let d = (player.pos.distance(point) - CONTROL_RADIUS * 0.8).max(0.0);
    if d <= 0.0 {
        return 0.0;
    }
    let v = player.attrs.top_speed;
    // Account for acceleration from the current velocity component towards the target.
    let dir = (point - player.pos).normalize_or_zero();
    let v0 = player.vel.dot(dir).clamp(0.0, v);
    let accel_penalty = (v - v0) / (2.0 * player.attrs.acceleration);
    REACTION_TIME + d / v + accel_penalty
}

/// Earliest point at which `player` can play a ball currently at `pos`
/// moving with `vel` with `air` metres of flight left. Returns `(point, time)`.
pub fn intercept(player: &Player, pos: Vec2, vel: Vec2, air: f32) -> (Vec2, f32) {
    let speed = vel.length();
    let t_air = if air > 0.0 && speed > 1e-4 {
        air / speed
    } else {
        0.0
    };
    let horizon =
        (t_air + stop_time(speed * if t_air > 0.0 { LANDING_DAMPING } else { 1.0 })).min(7.0);
    let mut t = 0.0;
    while t <= horizon {
        let (p, airborne) = predict_flight(pos, vel, air, t);
        if !airborne && time_to_reach(player, p) <= t {
            return (p, t);
        }
        t += 0.1;
    }
    let (rest, _) = predict_flight(pos, vel, air, horizon);
    (rest, time_to_reach(player, rest).max(horizon))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::role::{Attributes, Behaviour, Role};
    use crate::state::PlayerId;

    fn player_at(p: Vec2) -> Player {
        let mut pl = Player::new(
            PlayerId(0),
            Role::CentralMid,
            Attributes::for_role(Role::CentralMid),
            Behaviour::for_role(Role::CentralMid),
        );
        pl.pos = p;
        pl
    }

    #[test]
    fn pass_speed_reaches_target_with_arrival_speed() {
        for d in [5.0, 15.0, 30.0] {
            let v0 = pass_speed_for(d, PASS_ARRIVAL_SPEED);
            let t = travel_time(v0, d).unwrap();
            let v_end = v0 - BALL_DECEL * t;
            assert!(
                (v_end - PASS_ARRIVAL_SPEED).abs() < 0.3 || v0 == MIN_PASS_SPEED,
                "d={d} v_end={v_end}"
            );
        }
    }

    #[test]
    fn predict_matches_stepwise_integration() {
        let mut p = Vec2::ZERO;
        let mut v = Vec2::new(15.0, 5.0);
        let dt = 1.0 / 60.0;
        for _ in 0..120 {
            v = apply_friction(v, dt);
            p += v * dt;
        }
        let analytic = predict(Vec2::ZERO, Vec2::new(15.0, 5.0), 2.0);
        assert!(p.distance(analytic) < 0.2, "{p:?} vs {analytic:?}");
    }

    #[test]
    fn lofted_ball_cannot_be_played_in_flight() {
        let pl = player_at(Vec2::new(10.0, 0.5));
        // 30 m of flight left at 20 m/s: the player under the flight path must
        // wait for the landing ~30 m away.
        let (p, t) = intercept(&pl, Vec2::ZERO, Vec2::new(20.0, 0.0), 30.0);
        assert!(p.x >= 30.0 - 1e-3, "{p:?}");
        assert!(t >= 1.5 - 1e-3);
    }

    #[test]
    fn lofted_speed_is_reasonable() {
        for d in [20.0, 40.0, 60.0] {
            let v = lofted_speed(d);
            let flight = d / v;
            assert!((0.8..3.5).contains(&flight), "d={d} flight={flight}");
        }
    }

    #[test]
    fn ball_stops() {
        let p = predict(Vec2::ZERO, Vec2::new(10.0, 0.0), 100.0);
        assert!((p.x - stop_distance(10.0)).abs() < 1e-3);
    }

    #[test]
    fn closer_player_intercepts_sooner() {
        let near = player_at(Vec2::new(10.0, 3.0));
        let far = player_at(Vec2::new(10.0, 15.0));
        let (_, tn) = intercept(&near, Vec2::ZERO, Vec2::new(12.0, 0.0), 0.0);
        let (_, tf) = intercept(&far, Vec2::ZERO, Vec2::new(12.0, 0.0), 0.0);
        assert!(tn < tf);
    }

    #[test]
    fn stationary_ball_intercept_is_reach_time() {
        let pl = player_at(Vec2::new(5.0, 0.0));
        let (p, t) = intercept(&pl, Vec2::ZERO, Vec2::ZERO, 0.0);
        assert_eq!(p, Vec2::ZERO);
        assert!((t - time_to_reach(&pl, Vec2::ZERO)).abs() < 1e-5);
    }
}

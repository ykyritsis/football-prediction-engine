//! Per-player decisions.
//!
//! [`decide`] is a pure function `(&MatchState, &TeamPlan, PlayerId) -> Decision`.
//! It never mutates anything and never depends on other players' decisions
//! made in the same tick, so it can run as a parallel ECS system.
//!
//! * **Ball carrier**: every option is scored on the shared expected-value
//!   scale (see [`crate::eval`]): each pass candidate, a shot (xG), carrying
//!   the ball in several directions, and shielding it. Identity enters through
//!   behaviour (dribble bias, shot threshold/range, risk aversion, tempo) and
//!   team tactics. [`carrier_options`] exposes the full breakdown.
//! * **Off the ball**: the player executes the part of the team plan assigned
//!   to him (press, cover, mark, support run, chase) or holds his
//!   formation-relative position.

use crate::ball::CONTROL_RADIUS;
use crate::eval::{
    base_xg, goal_angle, possession_value, pressure_at, risk_aversion, turnover_cost,
};
use crate::marking::marking_position;
use crate::math::{closest_point_on_segment, cmp_f32, logistic, Vec2};
use crate::passing::{evaluate_passes, PassCandidate, PassKind};
use crate::pitch::{
    clamp_to_pitch, in_own_box_local, CENTRE_CIRCLE_RADIUS, GOAL_HALF_WIDTH, HALF_LENGTH,
    HALF_WIDTH,
};
use crate::plan::TeamPlan;
use crate::pressing::{contain_position, cover_position, PressMode};
use crate::state::{MatchState, Player, PlayerId, RestartKind};
use crate::support::SupportRun;
use std::f32::consts::FRAC_1_SQRT_2;

/// Maximum distance at which a tackle can be attempted.
pub const TACKLE_RANGE: f32 = 1.8;

/// Locomotion urgency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MoveSpeed {
    Walk,
    Jog,
    Run,
    Sprint,
}

impl MoveSpeed {
    /// Fraction of top speed.
    pub fn factor(self) -> f32 {
        match self {
            MoveSpeed::Walk => 0.3,
            MoveSpeed::Jog => 0.55,
            MoveSpeed::Run => 0.8,
            MoveSpeed::Sprint => 1.0,
        }
    }

    fn for_distance(d: f32) -> MoveSpeed {
        if d > 14.0 {
            MoveSpeed::Run
        } else if d > 3.0 {
            MoveSpeed::Jog
        } else {
            MoveSpeed::Walk
        }
    }
}

/// What the player is doing, for logic and for debug display.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Activity {
    HoldShape,
    Goalkeeping,
    Mark(PlayerId),
    Press(PlayerId),
    Contain(PlayerId),
    Cover(PlayerId),
    Support(SupportRun),
    ChaseBall,
    ReceivePass,
    TakeRestart,
    KeepDistance,
    // on the ball
    Dribble,
    Shield,
    Passing,
    Shooting,
    Clearing,
    Recovering,
    /// Standing in the wall for an opponent's free kick.
    Wall,
    /// Sent off: takes no further part.
    SentOff,
}

impl Activity {
    pub fn label(&self) -> String {
        match self {
            Activity::HoldShape => "hold shape".into(),
            Activity::Goalkeeping => "goalkeeping".into(),
            Activity::Mark(t) => format!("mark #{}", t.0),
            Activity::Press(t) => format!("press #{}", t.0),
            Activity::Contain(t) => format!("contain #{}", t.0),
            Activity::Cover(t) => format!("cover lane to #{}", t.0),
            Activity::Support(r) => r.name().into(),
            Activity::ChaseBall => "chase ball".into(),
            Activity::ReceivePass => "receive pass".into(),
            Activity::TakeRestart => "take restart".into(),
            Activity::KeepDistance => "keep distance".into(),
            Activity::Dribble => "dribble".into(),
            Activity::Shield => "shield".into(),
            Activity::Passing => "pass".into(),
            Activity::Shooting => "shoot".into(),
            Activity::Clearing => "clear".into(),
            Activity::Recovering => "recovering".into(),
            Activity::Wall => "wall".into(),
            Activity::SentOff => "sent off".into(),
        }
    }
}

/// An on-ball shot option.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotOption {
    pub target: Vec2,
    pub xg: f32,
    pub score: f32,
}

/// An on-ball carry option.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DribbleOption {
    pub target: Vec2,
    pub risk: f32,
    pub score: f32,
}

/// Instantaneous action requested this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    None,
    Pass(PassCandidate),
    Shoot(ShotOption),
    Clear { target: Vec2, speed: f32 },
    Tackle(PlayerId),
}

/// A player's decision for one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decision {
    pub player: PlayerId,
    pub activity: Activity,
    pub action: Action,
    /// Where to move (world).
    pub target: Vec2,
    pub speed: MoveSpeed,
}

/// Full breakdown of the carrier's options, best first in each list.
#[derive(Debug, Clone, PartialEq)]
pub struct CarrierOptions {
    pub passes: Vec<PassCandidate>,
    pub shot: Option<ShotOption>,
    pub dribbles: Vec<DribbleOption>,
    pub hold_score: f32,
    /// Whether the carrier may release the ball this tick (first touch done).
    pub can_release: bool,
}

/// Unit directions for carry options in the team-local frame: forward, then
/// pairs at ±30°, ±60°, ±90°, ±135°. Constant so results do not depend on
/// platform trigonometry.
const CARRY_DIRS: [(f32, f32); 9] = [
    (1.0, 0.0),
    (0.866_025_4, 0.5),
    (0.866_025_4, -0.5),
    (0.5, 0.866_025_4),
    (0.5, -0.866_025_4),
    (0.0, 1.0),
    (0.0, -1.0),
    (-FRAC_1_SQRT_2, FRAC_1_SQRT_2),
    (-FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
];
const CARRY_DISTANCE: f32 = 6.0;

/// Team-local aim point: the corner further from the keeper.
fn aim_point(state: &MatchState, shooter: &Player) -> Vec2 {
    let side = shooter.team;
    let gk = side.to_local(state.goalkeeper(side.opponent()).pos);
    let post = GOAL_HALF_WIDTH - 0.6;
    let aim_y = if (gk.y - post).abs() >= (gk.y + post).abs() {
        post
    } else {
        -post
    };
    Vec2::new(HALF_LENGTH + 0.5, aim_y)
}

/// Expected goals of a direct free kick from `shooter`'s position, or `None`
/// when too far out to shoot.
pub fn free_kick_xg(shooter: &Player) -> Option<f32> {
    let local = shooter.team.to_local(shooter.pos);
    let d = local.distance(Vec2::new(HALF_LENGTH, 0.0));
    if d > 32.0 {
        return None;
    }
    let angle = (goal_angle(local) / 0.35).clamp(0.3, 1.0);
    Some(0.1 * (-(d - 17.0).max(0.0) / 6.0).exp() * (0.35 + 0.9 * shooter.attrs.set_pieces) * angle)
}

/// Expected goals of a penalty for `taker`.
pub fn penalty_xg(taker: &Player) -> f32 {
    0.68 + 0.14 * taker.attrs.finishing
}

/// Evaluate a shot for `shooter`. `None` if out of range.
pub fn evaluate_shot(state: &MatchState, shooter: &Player) -> Option<ShotOption> {
    let side = shooter.team;
    let local = side.to_local(shooter.pos);
    let goal = Vec2::new(HALF_LENGTH, 0.0);
    let dist = local.distance(goal);
    if dist > shooter.behaviour.shot_range || local.x >= HALF_LENGTH {
        return None;
    }
    let a = &shooter.attrs;
    let mut xg = base_xg(local) * (0.7 + 0.6 * a.finishing);
    if dist > 18.0 {
        xg *= 0.6 + 0.8 * a.kicking_power;
    }
    let aim = aim_point(state, shooter);
    for opp in state.on_pitch(side.opponent()) {
        let o = side.to_local(opp.pos);
        let (c, t) = closest_point_on_segment(local, aim, o);
        let lateral = o.distance(c);
        if opp.is_goalkeeper() {
            // A keeper standing on the shot line saves more.
            xg *= 0.75 + 0.25 * (lateral / 3.0).min(1.0);
        } else if t > 0.02 && lateral < 1.2 {
            xg *= 0.45;
        }
    }
    // A defender closing the shooter down rushes and blocks the shot.
    xg *= 1.0 - 0.55 * pressure_at(state, side, shooter.pos);
    let xg = xg.clamp(0.0, 0.95);
    let b = &shooter.behaviour;
    let gate = if xg >= b.shot_threshold { 0.0 } else { -1.0 };
    Some(ShotOption {
        target: side.to_world(aim),
        xg,
        score: xg - (1.0 - xg) * 0.012 + gate,
    })
}

/// Evaluate carrying the ball in each of [`CARRY_DIRS`].
pub fn evaluate_dribbles(state: &MatchState, carrier: &Player) -> Vec<DribbleOption> {
    let side = carrier.team;
    let here = side.to_local(carrier.pos);
    let risk_av = risk_aversion(state, carrier);
    let b = &carrier.behaviour;
    let opps: Vec<Vec2> = state
        .on_pitch(side.opponent())
        .map(|p| side.to_local(p.pos))
        .collect();
    let mut out: Vec<DribbleOption> = CARRY_DIRS
        .iter()
        .filter_map(|&(x, y)| {
            let p = clamp_to_pitch(here + Vec2::new(x, y) * CARRY_DISTANCE, 1.0);
            // Directions blocked by the touchline/goal line are not options.
            if p.distance(here) < CARRY_DISTANCE * 0.5 {
                return None;
            }
            let lane = opps
                .iter()
                .map(|o| o.distance(closest_point_on_segment(here, p, *o).0))
                .fold(f32::INFINITY, f32::min);
            let skill = 0.7 * carrier.attrs.dribbling + 0.3 * carrier.attrs.control;
            let risk = (logistic((2.4 - lane) * 1.7) * (1.2 - 0.75 * skill)).clamp(0.0, 0.95);
            let ev = (1.0 - risk) * possession_value(p) - risk * turnover_cost(here) * risk_av;
            let style = (b.dribble_bias - 0.45) * 0.03;
            Some(DribbleOption {
                target: side.to_world(p),
                risk,
                score: ev + style,
            })
        })
        .collect();
    out.sort_by(|a, b| cmp_f32(b.score, a.score));
    out
}

/// Score for shielding / holding the ball.
pub fn hold_score(state: &MatchState, carrier: &Player) -> f32 {
    let side = carrier.team;
    let here = side.to_local(carrier.pos);
    let pressure = pressure_at(state, side, carrier.pos);
    let b = &carrier.behaviour;
    let lose = pressure * 0.4 * (1.15 - 0.45 * carrier.attrs.control - 0.35 * b.hold_up);
    let ev = (1.0 - lose) * possession_value(here)
        - lose * turnover_cost(here) * risk_aversion(state, carrier);
    // Holding is a stop-gap: small constant penalty so players act.
    ev - 0.006
        + if pressure > 0.5 {
            (b.hold_up - 0.3) * 0.015
        } else {
            0.0
        }
}

/// Extra cost of continuing to shield: under pressure for more than eight
/// seconds it becomes untenable and the carrier must pass or carry.
pub fn shielding_fatigue(state: &MatchState, carrier: &Player) -> f32 {
    if pressure_at(state, carrier.team, carrier.pos) <= 0.5 {
        return 0.0;
    }
    (state.seconds_since(state.ball.owner_since) - 8.0).max(0.0) * 0.01
}

/// Full option breakdown for the ball carrier.
pub fn carrier_options(state: &MatchState, plan: &TeamPlan, id: PlayerId) -> CarrierOptions {
    let p = state.player(id);
    CarrierOptions {
        passes: evaluate_passes(state, id, plan.phase),
        shot: evaluate_shot(state, p),
        dribbles: evaluate_dribbles(state, p),
        hold_score: hold_score(state, p),
        can_release: p.action_cooldown == 0,
    }
}

fn decide_carrier(state: &MatchState, plan: &TeamPlan, p: &Player) -> Decision {
    let opts = carrier_options(state, plan, p.id);
    let mut d = Decision {
        player: p.id,
        activity: Activity::Shield,
        action: Action::None,
        target: p.pos,
        speed: MoveSpeed::Walk,
    };

    let best_dribble = opts.dribbles.first().copied();
    // A shot must beat holding on; shielding fatigue only pushes the carrier
    // towards passing or carrying, never into a forced shot.
    let shot_bar_hold = opts.hold_score;
    let mut best_score = opts.hold_score - shielding_fatigue(state, p);
    if let Some(dr) = best_dribble {
        if dr.score > best_score {
            best_score = dr.score;
            d.activity = Activity::Dribble;
            d.target = dr.target;
            d.speed = MoveSpeed::Run;
        }
    }
    if !opts.can_release {
        return d;
    }
    if let Some(pass) = opts.passes.first() {
        if pass.score > best_score {
            best_score = pass.score;
            d.activity = Activity::Passing;
            d.action = Action::Pass(*pass);
            d.target = p.pos;
        }
    }
    if let Some(shot) = opts.shot {
        if shot.score > best_score && shot.score > shot_bar_hold {
            best_score = shot.score;
            d.activity = Activity::Shooting;
            d.action = Action::Shoot(shot);
            d.target = p.pos;
        }
    }
    // Under heavy pressure in or near the own box with nothing good on: clear it.
    let local = p.team.to_local(p.pos);
    let pressure = pressure_at(state, p.team, p.pos);
    if (in_own_box_local(local) || local.x < -HALF_LENGTH + 22.0)
        && pressure > 0.7
        && best_score < -0.01
    {
        let y = if local.y >= 0.0 {
            HALF_WIDTH - 6.0
        } else {
            -(HALF_WIDTH - 6.0)
        };
        d.activity = Activity::Clearing;
        d.action = Action::Clear {
            target: p.team.to_world(Vec2::new(10.0, y)),
            speed: 26.0,
        };
        d.target = p.pos;
    }
    d
}

fn decide_restart_taker(state: &MatchState, plan: &TeamPlan, p: &Player) -> Decision {
    let mut d = Decision {
        player: p.id,
        activity: Activity::TakeRestart,
        action: Action::None,
        target: p.pos,
        speed: MoveSpeed::Walk,
    };
    if p.action_cooldown > 0 {
        return d;
    }
    let kind = state.restart.map(|r| r.kind);
    if kind == Some(RestartKind::Penalty) {
        // Pick a side: deterministic but not predictable from the position.
        let left = (state.tick / 7 + p.id.0 as u32).is_multiple_of(2);
        let y = if left {
            GOAL_HALF_WIDTH - 0.8
        } else {
            -(GOAL_HALF_WIDTH - 0.8)
        };
        let xg = penalty_xg(p);
        let target = p.team.to_world(Vec2::new(HALF_LENGTH + 0.5, y));
        d.activity = Activity::Shooting;
        d.action = Action::Shoot(ShotOption {
            target,
            xg,
            score: xg,
        });
        return d;
    }
    let opts = evaluate_passes(state, p.id, plan.phase);
    if kind == Some(RestartKind::FreeKick) {
        if let Some(xg) = free_kick_xg(p) {
            let best_pass = opts.first().map_or(-1.0, |c| c.score);
            if xg >= 0.045 && xg >= best_pass - 0.02 {
                let target = p.team.to_world(aim_point(state, p));
                d.activity = Activity::Shooting;
                d.action = Action::Shoot(ShotOption {
                    target,
                    xg,
                    score: xg,
                });
                return d;
            }
        }
    }
    {
        if let Some(best) = opts
            .iter()
            .find(|c| c.kind != PassKind::Cross)
            .or(opts.first())
        {
            d.action = Action::Pass(*best);
        }
    }
    d
}

/// During a penalty everyone except the taker and the defending keeper waits
/// outside the area and the arc.
fn outside_penalty_area(target: Vec2, defending: crate::pitch::TeamSide, spot: Vec2) -> Vec2 {
    let mut l = defending.to_local(target);
    let edge = -HALF_LENGTH + crate::pitch::PENALTY_AREA_DEPTH + 1.0;
    if l.x < edge && l.y.abs() < crate::pitch::PENALTY_AREA_HALF_WIDTH + 1.0 {
        l.x = edge;
    }
    keep_distance(defending.to_world(l), spot)
}

fn keep_distance(target: Vec2, spot: Vec2) -> Vec2 {
    let off = target - spot;
    let min = CENTRE_CIRCLE_RADIUS + 0.5;
    if off.length() >= min {
        target
    } else {
        let dir = if off.length() < 1e-3 {
            Vec2::X
        } else {
            off.normalize_or_zero()
        };
        clamp_to_pitch(spot + dir * min, 0.5)
    }
}

fn goalkeeper_decision(state: &MatchState, plan: &TeamPlan, p: &Player) -> Decision {
    let mut target = plan.shape[p.id.slot()];
    let mut speed = MoveSpeed::Jog;
    if state
        .restart
        .is_some_and(|r| r.kind == RestartKind::Penalty && r.team != p.team)
    {
        // On the line for a penalty.
        target = p.team.own_goal() + Vec2::new(p.team.direction() * 0.3, 0.0);
    }
    if let Some(shot) = state.ball.shot {
        if shot.shooter.team() != p.team {
            // Get across to the shot line.
            let end = shot.target;
            let (c, _) = closest_point_on_segment(state.ball.pos, end, p.pos);
            target = c;
            speed = MoveSpeed::Sprint;
        }
    }
    Decision {
        player: p.id,
        activity: Activity::Goalkeeping,
        action: Action::None,
        target,
        speed,
    }
}

/// Is it worth committing to a tackle? Defenders jockey and wait for a heavy
/// touch, a carrier with his back turned or one who has stopped (staying on
/// their feet in their own box unless he is static), and only dive in when
/// their chance of winning the ball clears their aggression threshold.
pub fn tackle_window(state: &MatchState, tackler: &Player, carrier: &Player) -> bool {
    let settling = carrier.action_cooldown > 0 || state.seconds_since(state.ball.owner_since) < 0.5;
    let to_tackler = (tackler.pos - carrier.pos).normalize_or_zero();
    let back_turned = carrier.facing.dot(to_tackler) < -0.2;
    let stationary = carrier.vel.length() < 1.5;
    let exposed = if crate::rules::in_penalty_area(tackler.team, carrier.pos) {
        stationary || settling
    } else {
        settling || back_turned || stationary
    };
    // Commit only when the tackle is likely enough to succeed for this player.
    exposed
        && crate::rules::tackle_success(tackler, carrier, settling)
            >= crate::rules::tackle_commit_threshold(tackler)
}

/// Decide what player `id` does this tick.
pub fn decide(state: &MatchState, plan: &TeamPlan, id: PlayerId) -> Decision {
    let p = state.player(id);
    if p.sent_off {
        return Decision {
            player: id,
            activity: Activity::SentOff,
            action: Action::None,
            target: p.pos,
            speed: MoveSpeed::Walk,
        };
    }
    let mut d = decide_on_pitch(state, plan, id);
    // Penalty: clear the area (taker and defending keeper excepted).
    if let Some(r) = state.restart.filter(|r| r.kind == RestartKind::Penalty) {
        let taker = state.ball.owner == Some(id) || plan.restart_taker == Some(id);
        let keeper = p.is_goalkeeper() && p.team != r.team;
        if !taker && !keeper {
            d.target = outside_penalty_area(d.target, r.team.opponent(), r.spot);
        }
    }
    d
}

fn decide_on_pitch(state: &MatchState, plan: &TeamPlan, id: PlayerId) -> Decision {
    let p = state.player(id);
    debug_assert_eq!(p.team, plan.side);
    let slot = id.slot();
    let shape_target = plan.shape[slot];

    if p.stun > 0 {
        return Decision {
            player: id,
            activity: Activity::Recovering,
            action: Action::None,
            target: p.pos,
            speed: MoveSpeed::Walk,
        };
    }
    if state.ball.owner == Some(id) {
        if state.restart.is_some() {
            return decide_restart_taker(state, plan, p);
        }
        return decide_carrier(state, plan, p);
    }
    if let Some((chaser, point)) = plan.ball_chaser {
        if chaser == id {
            let activity = match (state.restart, state.ball.pass) {
                (Some(_), _) => Activity::TakeRestart,
                (None, Some(pass)) if pass.to == id => Activity::ReceivePass,
                _ => Activity::ChaseBall,
            };
            return Decision {
                player: id,
                activity,
                action: Action::None,
                target: point,
                speed: MoveSpeed::Sprint,
            };
        }
    }

    // Opponent restart: form the wall, everyone else keeps their distance.
    if let Some(r) = state.restart {
        if let Some(w) = plan.wall_spot(id) {
            let d = p.pos.distance(w);
            return Decision {
                player: id,
                activity: Activity::Wall,
                action: Action::None,
                target: w,
                speed: MoveSpeed::for_distance(d).max(MoveSpeed::Jog),
            };
        }
        if r.team != p.team {
            let t = keep_distance(shape_target, r.spot);
            let d = p.pos.distance(t);
            return Decision {
                player: id,
                activity: Activity::KeepDistance,
                action: Action::None,
                target: t,
                speed: MoveSpeed::for_distance(d),
            };
        }
    }

    if p.is_goalkeeper() {
        return goalkeeper_decision(state, plan, p);
    }

    if plan.phase.in_possession() {
        if let Some(s) = plan.support_for(id) {
            let d = p.pos.distance(s.target);
            let speed = if s.run.is_sprint() && d > 2.0 {
                MoveSpeed::Sprint
            } else {
                MoveSpeed::for_distance(d).max(MoveSpeed::Jog)
            };
            return Decision {
                player: id,
                activity: Activity::Support(s.run),
                action: Action::None,
                target: s.target,
                speed,
            };
        }
        let d = p.pos.distance(shape_target);
        return Decision {
            player: id,
            activity: Activity::HoldShape,
            action: Action::None,
            target: shape_target,
            speed: MoveSpeed::for_distance(d),
        };
    }

    // ---- out of possession ----
    if let Some(carrier) = state.carrier().filter(|c| c.team != p.team) {
        let press = &plan.press;
        let engaging = press.primary == Some(id) || press.swarm.contains(&Some(id));
        if engaging {
            let dist = p.pos.distance(carrier.pos);
            let (activity, target, speed, tackle_range) =
                if press.mode == PressMode::Press || press.swarm.contains(&Some(id)) {
                    let goal_side = (p.team.own_goal() - carrier.pos).normalize_or_zero() * 0.6;
                    (
                        Activity::Press(carrier.id),
                        carrier.pos + goal_side,
                        MoveSpeed::Sprint,
                        TACKLE_RANGE,
                    )
                } else {
                    // Containing: stay goal-side, but step in on a carrier who stops
                    // or who reaches the danger zone.
                    let danger = p.team.to_local(carrier.pos).x < -HALF_LENGTH + 25.0;
                    if carrier.vel.length() < 2.0 || danger {
                        let goal_side = (p.team.own_goal() - carrier.pos).normalize_or_zero() * 0.8;
                        (
                            Activity::Contain(carrier.id),
                            carrier.pos + goal_side,
                            MoveSpeed::Run,
                            TACKLE_RANGE,
                        )
                    } else {
                        let t = contain_position(state, p.team, carrier.pos);
                        (
                            Activity::Contain(carrier.id),
                            t,
                            MoveSpeed::Run,
                            CONTROL_RADIUS + 0.2,
                        )
                    }
                };
            let action = if dist <= tackle_range
                && p.tackle_cooldown == 0
                && tackle_window(state, p, carrier)
            {
                Action::Tackle(carrier.id)
            } else {
                Action::None
            };
            return Decision {
                player: id,
                activity,
                action,
                target,
                speed,
            };
        }
        if press.cover == Some(id) {
            if let Some(t) = press.cover_target {
                let target = cover_position(carrier.pos, state.player(t).pos);
                return Decision {
                    player: id,
                    activity: Activity::Cover(t),
                    action: Action::None,
                    target,
                    speed: MoveSpeed::Run,
                };
            }
        }
    }
    if let Some(m) = plan.marking_for(id) {
        let a = state.player(m.target);
        let target = marking_position(state, p.team, m.target, m.tightness);
        let d = p.pos.distance(target);
        // Match the attacker's urgency: a runner is tracked at full speed.
        let runner = a.vel.length() > 4.0 || p.team.to_local(a.pos).x < -HALF_LENGTH + 30.0;
        let speed = if d > 2.0 && (runner || m.tightness > 0.6) {
            MoveSpeed::Sprint
        } else {
            MoveSpeed::for_distance(d).max(MoveSpeed::Jog)
        };
        return Decision {
            player: id,
            activity: Activity::Mark(m.target),
            action: Action::None,
            target,
            speed,
        };
    }
    let d = p.pos.distance(shape_target);
    // Recovery runs: sprint back when caught upfield.
    let behind_ball = p.team.to_local(p.pos).x > p.team.to_local(state.ball.pos).x;
    let speed = if behind_ball && d > 15.0 && p.behaviour.track_back > 0.5 {
        MoveSpeed::Sprint
    } else {
        MoveSpeed::for_distance(d)
    };
    Decision {
        player: id,
        activity: Activity::HoldShape,
        action: Action::None,
        target: shape_target,
        speed,
    }
}

/// Decide for every player, in id order.
pub fn decide_all(state: &MatchState, plans: &[TeamPlan; 2]) -> Vec<Decision> {
    state
        .players
        .iter()
        .map(|p| decide(state, &plans[p.team.index()], p.id))
        .collect()
}

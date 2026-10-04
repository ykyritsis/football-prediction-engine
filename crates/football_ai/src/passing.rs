//! Passing candidate generation and evaluation.
//!
//! For a ball carrier, every team-mate yields one or more candidates (to feet,
//! a lead/through ball into a runner, a long lofted ball, a cross into the
//! box). Each candidate is scored with the shared value model
//! ([`crate::eval`]):
//!
//! * **interception risk**: for every opponent, the time the ball needs to
//!   reach the closest point of the lane against the time the opponent needs
//!   to get there, squashed into a probability; combined over all opponents
//!   and with the passer's accuracy over the distance;
//! * **value**: possession value at the target, discounted by how much
//!   pressure the receiver will be under;
//! * **progression** and **lines broken**, used by style utilities;
//! * **style utility**: team instructions (directness, long ball, wide play,
//!   crossing, tempo) and player traits (through-ball, long-pass and cross
//!   biases) add small bonuses on the same scale.
//!
//! The breakdown is kept on each [`PassCandidate`] so the sandbox can show why
//! a pass was (not) chosen.

use crate::ball::{lofted_speed, lofted_time, pass_speed_for, travel_time, LOFT_SAFE_FRACTION, PASS_ARRIVAL_SPEED, REACTION_TIME};
use crate::eval::{possession_value, pressure_at, risk_aversion, turnover_cost};
use crate::math::{closest_point_on_segment, cmp_f32, logistic, Vec2};
use crate::phase::TeamPhase;
use crate::pitch::{clamp_to_pitch, TeamSide, HALF_LENGTH};
use crate::state::{MatchState, Player, PlayerId};

/// Chance a defender who reaches a lofted ball first actually wins it.
pub const AERIAL_DUEL_WIN: f32 = 0.5;

/// Kind of pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassKind {
    /// Ground pass to the receiver's feet.
    Short,
    /// Ground pass into the receiver's run.
    Through,
    /// Lofted pass over distance.
    Long,
    /// Lofted delivery into the penalty area.
    Cross,
}

impl PassKind {
    pub fn is_lofted(self) -> bool {
        matches!(self, PassKind::Long | PassKind::Cross)
    }
}

/// A scored passing option.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassCandidate {
    pub receiver: PlayerId,
    pub kind: PassKind,
    pub target: Vec2,
    pub speed: f32,
    pub distance: f32,
    pub travel_time: f32,
    /// Probability the pass is intercepted or misplaced.
    pub risk: f32,
    /// Pressure on the receiver at the target, `0..1`.
    pub receiver_pressure: f32,
    /// Forward gain in the passer's local frame (m).
    pub progression: f32,
    /// Opponent outfield players bypassed.
    pub lines_broken: u8,
    /// Expected value before style utility.
    pub ev: f32,
    /// Style utility added on top of `ev`.
    pub style: f32,
    /// Final score: `ev + style`.
    pub score: f32,
}

/// Probability that an opponent of `passer_team` cuts out a ball played from
/// `origin` to `target` at `speed`.
///
/// `aerial` marks a cross: attackers attack the ball in the air, so a defender
/// who gets there first only wins it [`AERIAL_DUEL_WIN`] of the time.
pub fn interception_risk(
    state: &MatchState,
    passer_team: TeamSide,
    origin: Vec2,
    target: Vec2,
    speed: f32,
    lofted: bool,
    aerial: bool,
) -> f32 {
    let duel = if aerial { AERIAL_DUEL_WIN } else { 1.0 };
    let length = origin.distance(target);
    let mut survive = 1.0;
    for opp in state.team_players(passer_team.opponent()) {
        let (c, t) = closest_point_on_segment(origin, target, opp.pos);
        if lofted && t < LOFT_SAFE_FRACTION {
            // Ball is overhead for most of the flight: only the landing zone is contestable.
            let landing = origin.lerp(target, LOFT_SAFE_FRACTION);
            let (c2, _) = closest_point_on_segment(landing, target, opp.pos);
            let ball_t = lofted_time(speed, origin.distance(c2));
            let opp_t = REACTION_TIME + (opp.pos.distance(c2) - 1.0).max(0.0) / opp.attrs.top_speed;
            // Getting to a dropping ball only wins the aerial duel some of the time.
            survive *= 1.0 - duel * logistic(-(opp_t - ball_t) * 5.0 - 1.2);
            continue;
        }
        let along = t * length;
        let ball_t = travel_time(speed, along).unwrap_or(10.0);
        let reach = if opp.is_goalkeeper() { 1.8 } else { 1.0 };
        let opp_t = REACTION_TIME + (opp.pos.distance(c) - reach).max(0.0) / opp.attrs.top_speed;
        let margin = opp_t - ball_t;
        let r = logistic(-margin * 5.0 - 1.2);
        if r > 0.01 {
            survive *= 1.0 - r;
        }
    }
    1.0 - survive
}

/// Number of opponent outfield players whose local `x` lies between the two
/// points (only counts forward passes).
pub fn lines_broken(state: &MatchState, team: TeamSide, from: Vec2, to: Vec2) -> u8 {
    let a = team.to_local(from).x;
    let b = team.to_local(to).x;
    if b <= a {
        return 0;
    }
    state
        .team_players(team.opponent())
        .iter()
        .filter(|p| !p.is_goalkeeper())
        .filter(|p| {
            let x = team.to_local(p.pos).x;
            x > a && x < b
        })
        .count() as u8
}

fn misplace_probability(passer: &Player, distance: f32, lofted: bool) -> f32 {
    let acc = if lofted { passer.attrs.long_passing } else { passer.attrs.passing };
    let base = (1.0 - acc) * distance / 55.0;
    (base + if lofted { 0.04 } else { 0.0 }).clamp(0.0, 0.9)
}

/// Lead position for a receiver who is on the move.
fn lead_target(receiver: &Player, origin: Vec2) -> (Vec2, f32) {
    let mut target = receiver.pos;
    let mut speed = PASS_ARRIVAL_SPEED;
    for _ in 0..3 {
        let d = origin.distance(target);
        speed = pass_speed_for(d, PASS_ARRIVAL_SPEED);
        let t = travel_time(speed, d).unwrap_or(d / speed);
        target = clamp_to_pitch(receiver.pos + receiver.vel * t, 1.0);
    }
    (target, speed)
}

/// Generate and score every passing option for the carrier, best first.
/// Ties are broken by receiver id and pass kind so the order is deterministic.
pub fn evaluate_passes(state: &MatchState, carrier: PlayerId, phase: TeamPhase) -> Vec<PassCandidate> {
    let passer = state.player(carrier);
    let team = passer.team;
    let tactics = &state.team(team).tactics;
    let b = &passer.behaviour;
    let origin = passer.pos;
    let origin_local = team.to_local(origin);
    let risk_av = risk_aversion(state, passer);

    let offside_applies = !state.restart.is_some_and(|r| r.kind.exempt_from_offside());
    let mut out = Vec::with_capacity(24);
    for receiver in state.team_players(team) {
        if receiver.id == carrier {
            continue;
        }
        if offside_applies && state.in_offside_position(receiver.id) {
            continue;
        }
        let rl = team.to_local(receiver.pos);
        let mut options: Vec<(PassKind, Vec2, f32)> = Vec::with_capacity(3);

        let d = origin.distance(receiver.pos);
        if d < 3.0 || d > 60.0 {
            continue;
        }
        if d <= 38.0 {
            options.push((PassKind::Short, receiver.pos, pass_speed_for(d, PASS_ARRIVAL_SPEED)));
        }
        // Lead pass into a forward run.
        let forward_speed = team.to_local(receiver.vel).x;
        if forward_speed > 4.0 {
            let (t, s) = lead_target(receiver, origin);
            // Only a genuine through ball if it plays the receiver into space
            // well ahead of him.
            if origin.distance(t) <= 42.0 && team.to_local(t).x - rl.x > 6.0 {
                options.push((PassKind::Through, t, s));
            }
        }
        // Long lofted ball.
        if d >= 22.0 {
            let (t, _) = if receiver.vel.length() > 3.0 { lead_target(receiver, origin) } else { (receiver.pos, 0.0) };
            let dd = origin.distance(t);
            options.push((PassKind::Long, t, lofted_speed(dd)));
        }
        // Cross: carrier wide in the final third, receiver in or arriving in the box.
        // Aimed at the zone the receiver is attacking, not at his feet.
        if origin_local.x > HALF_LENGTH - 34.0 && origin_local.y.abs() > 13.0 && rl.x > HALF_LENGTH - 28.0 && rl.y.abs() < 20.0
        {
            let aim = team.to_world(Vec2::new((rl.x + 4.0).clamp(HALF_LENGTH - 13.0, HALF_LENGTH - 5.0), rl.y * 0.5));
            let dd = origin.distance(aim);
            options.push((PassKind::Cross, aim, lofted_speed(dd)));
        }

        for (kind, target, speed) in options {
            out.push(score_candidate(state, passer, receiver, kind, target, speed, phase, risk_av, tactics, b, origin_local));
        }
    }
    out.sort_by(|a, b| {
        cmp_f32(b.score, a.score).then(a.receiver.cmp(&b.receiver)).then((a.kind as u8).cmp(&(b.kind as u8)))
    });
    out
}

#[allow(clippy::too_many_arguments)]
fn score_candidate(
    state: &MatchState,
    passer: &Player,
    receiver: &Player,
    kind: PassKind,
    target: Vec2,
    speed: f32,
    phase: TeamPhase,
    risk_av: f32,
    tactics: &crate::tactics::Tactics,
    b: &crate::role::Behaviour,
    origin_local: Vec2,
) -> PassCandidate {
    let team = passer.team;
    let lofted = kind.is_lofted();
    let distance = passer.pos.distance(target);
    let travel = if lofted {
        lofted_time(speed, distance)
    } else {
        travel_time(speed, distance).unwrap_or(distance / speed.max(1.0))
    };
    let intercept = interception_risk(state, team, passer.pos, target, speed, lofted, kind == PassKind::Cross);
    let misplace = misplace_probability(passer, distance, lofted);
    let risk = 1.0 - (1.0 - intercept) * (1.0 - misplace);

    let target_local = team.to_local(target);
    let pressure = pressure_at(state, team, target);
    // A receiver under pressure is worth less: he may lose it straight away.
    let value = possession_value(target_local) * (1.0 - 0.35 * pressure);
    // Turnovers happen around the interception point; approximate with the
    // lane midpoint for ground passes and the landing zone for lofted ones.
    let fail_at = if lofted { target_local } else { origin_local.lerp(target_local, 0.5) };
    let ev = (1.0 - risk) * value - risk * turnover_cost(fail_at) * risk_av;

    let progression = target_local.x - origin_local.x;
    let lines = lines_broken(state, team, passer.pos, target);

    // ---- style utility ----
    let mut style = 0.0;
    let prog_n = (progression / 30.0).clamp(-1.0, 1.0);
    style += (tactics.directness - 0.4) * 0.04 * prog_n;
    style += 0.004 * lines as f32 * (0.5 + tactics.directness);
    if phase == TeamPhase::AttackingTransition {
        style += tactics.counter_attack * 0.03 * prog_n.max(0.0);
    }
    if kind == PassKind::Long {
        style += (0.6 * tactics.long_ball + 0.4 * b.long_pass_bias - 0.35) * 0.06;
    }
    if kind == PassKind::Through {
        style += (b.through_ball_bias - 0.2) * 0.04;
    }
    if kind == PassKind::Cross {
        style += (0.5 * tactics.crossing + 0.5 * b.cross_bias - 0.3) * 0.06;
    }
    if distance < 16.0 && !lofted {
        // Short combination play (tiki-taka, low support range, high tempo).
        style += (tactics.support_closeness - 0.4) * 0.015 + (tactics.tempo - 0.5) * 0.01;
    }
    if target_local.y.abs() > 20.0 {
        style += (tactics.wide_play - 0.5) * 0.02;
    }
    if receiver.is_goalkeeper() {
        // Only a safety valve.
        style -= 0.01;
    }

    PassCandidate {
        receiver: receiver.id,
        kind,
        target,
        speed,
        distance,
        travel_time: travel,
        risk,
        receiver_pressure: pressure,
        progression,
        lines_broken: lines,
        ev,
        style,
        score: ev + style,
    }
}

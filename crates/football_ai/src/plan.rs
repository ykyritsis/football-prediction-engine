//! Team-level coordination for one tick.
//!
//! [`plan_team`] is a pure function of the match snapshot. It resolves every
//! decision that needs the whole team at once (phase, shape, who presses, who
//! marks whom, who makes which run, who goes for a loose ball) so that the
//! per-player [`crate::decision::decide`] calls that follow are independent of
//! each other and can run in any order, or in parallel.

use crate::ball::intercept;
use crate::formation::team_shape;
use crate::marking::{assign_marking, MarkAssignment};
use crate::math::{cmp_f32, Vec2};
use crate::phase::{phase_for, TeamPhase};
use crate::pitch::{clamp_to_pitch, TeamSide};
use crate::pressing::{plan_press, PressPlan};
use crate::state::{MatchState, PlayerId, RestartKind, TEAM_SIZE};
use crate::support::{plan_support, SupportAssignment};

/// Everything a team decided collectively this tick.
#[derive(Debug, Clone, PartialEq)]
pub struct TeamPlan {
    pub side: TeamSide,
    pub phase: TeamPhase,
    /// Formation-relative targets (world), by slot.
    pub shape: [Vec2; TEAM_SIZE],
    pub press: PressPlan,
    pub marking: Vec<MarkAssignment>,
    pub support: Vec<SupportAssignment>,
    /// Player going for a loose ball / receiving a pass, with his intercept point.
    pub ball_chaser: Option<(PlayerId, Vec2)>,
    /// Player taking a pending restart.
    pub restart_taker: Option<PlayerId>,
}

impl TeamPlan {
    pub fn marking_for(&self, id: PlayerId) -> Option<&MarkAssignment> {
        self.marking.iter().find(|m| m.marker == id)
    }

    pub fn support_for(&self, id: PlayerId) -> Option<&SupportAssignment> {
        self.support.iter().find(|s| s.player == id)
    }
}

/// Who should go for the ball when nobody controls it.
fn choose_chaser(state: &MatchState, side: TeamSide) -> Option<(PlayerId, Vec2)> {
    if state.ball.owner.is_some() {
        return None;
    }
    if let Some(r) = state.restart {
        if r.team != side {
            return None;
        }
        let taker = restart_taker(state, side, r.kind, r.spot)?;
        return Some((taker, r.spot));
    }
    let ball = &state.ball;
    // Our own pass in flight: the receiver goes to meet it.
    if let Some(pass) = ball.pass {
        if pass.to.team() == side {
            let receiver = state.player(pass.to);
            let (p, _) = intercept(receiver, ball.pos, ball.vel, ball.air_distance);
            return Some((pass.to, clamp_to_pitch(p, 0.5)));
        }
    }
    let mut best: Option<(f32, PlayerId, Vec2)> = None;
    for p in state.team_players(side) {
        if p.stun > 0 || p.action_cooldown > 0 && ball.last_touch == Some(p.id) {
            continue;
        }
        // Goalkeepers only come for balls in or near their box.
        if p.is_goalkeeper() && side.to_local(ball.pos).x > -crate::pitch::HALF_LENGTH + 20.0 {
            continue;
        }
        let (pt, t) = intercept(p, ball.pos, ball.vel, ball.air_distance);
        if best.is_none_or(|b| cmp_f32(t, b.0).then(p.id.cmp(&b.1)).is_lt()) {
            best = Some((t, p.id, pt));
        }
    }
    let (t, id, pt) = best?;
    let pt = clamp_to_pitch(pt, 0.5);
    // Opponent pass in flight: only go for it if we can get there first.
    if let Some(pass) = ball.pass {
        if pass.to.team() != side {
            let (_, tr) = intercept(state.player(pass.to), ball.pos, ball.vel, ball.air_distance);
            if t > tr + 0.15 {
                return None;
            }
        }
    }
    Some((id, pt))
}

/// Who takes a restart.
pub fn restart_taker(
    state: &MatchState,
    side: TeamSide,
    kind: RestartKind,
    spot: Vec2,
) -> Option<PlayerId> {
    if kind == RestartKind::GoalKick {
        return Some(state.goalkeeper(side).id);
    }
    state
        .team_players(side)
        .iter()
        .filter(|p| !p.is_goalkeeper())
        .min_by(|a, b| cmp_f32(a.pos.distance(spot), b.pos.distance(spot)).then(a.id.cmp(&b.id)))
        .map(|p| p.id)
}

/// Compute the team plan for `side`.
pub fn plan_team(state: &MatchState, side: TeamSide) -> TeamPlan {
    let phase = phase_for(state, side);
    let shape = team_shape(state, side, phase);
    let press = plan_press(state, side, phase);
    let mut busy: Vec<PlayerId> = [press.primary, press.cover, press.swarm[0], press.swarm[1]]
        .into_iter()
        .flatten()
        .collect();
    let ball_chaser = choose_chaser(state, side);
    if let Some((c, _)) = ball_chaser {
        busy.push(c);
    }
    let marking = assign_marking(state, side, phase, &shape, &busy);
    let support = plan_support(state, side, phase, &shape);
    let restart_taker = state
        .restart
        .filter(|r| r.team == side)
        .and_then(|r| restart_taker(state, side, r.kind, r.spot));
    TeamPlan {
        side,
        phase,
        shape,
        press,
        marking,
        support,
        ball_chaser,
        restart_taker,
    }
}

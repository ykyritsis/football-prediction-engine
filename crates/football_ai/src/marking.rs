//! Defensive marking assignments.
//!
//! A zonal-man hybrid: every defending outfield player not involved in the
//! press may pick up one opponent who is inside his zone (a radius around his
//! formation target that depends on his role and traits). Assignments are
//! solved greedily over a cost matrix sorted with explicit tie-breaking, so
//! they are deterministic and each attacker is marked by at most one player.
//!
//! The marking *position* is goal-side of the attacker and slightly ball-side,
//! with a distance set by how dangerous the attacker is, how close the ball
//! is, and the marker's tightness (traits + Tight Marking instruction).

use crate::math::{cmp_f32, Vec2};
use crate::phase::TeamPhase;
use crate::pitch::{zone_value_local, TeamSide};
use crate::role::Role;
use crate::state::{MatchState, PlayerId, TEAM_SIZE};

/// One marker → attacker assignment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkAssignment {
    pub marker: PlayerId,
    pub target: PlayerId,
    /// `0` = loose zonal marking, `1` = touch-tight.
    pub tightness: f32,
}

/// Danger posed by an attacker, `0..1`, from the defending team's perspective.
pub fn threat(state: &MatchState, defending: TeamSide, attacker: PlayerId) -> f32 {
    let a = state.player(attacker);
    let attacking = a.team;
    let local = attacking.to_local(a.pos);
    let zone = zone_value_local(local);
    // Attackers ahead of the ball (from their perspective) are more dangerous.
    let ahead = (local.x - attacking.to_local(state.ball.pos).x).clamp(-20.0, 20.0) / 40.0 + 0.5;
    let _ = defending;
    (0.75 * zone + 0.25 * ahead).clamp(0.0, 1.0)
}

/// Solve marking for `defending`. `shape` are the team's formation targets
/// (world, slot order) and `busy` players are already pressing.
pub fn assign_marking(
    state: &MatchState,
    defending: TeamSide,
    phase: TeamPhase,
    shape: &[Vec2; TEAM_SIZE],
    busy: &[PlayerId],
) -> Vec<MarkAssignment> {
    if phase.in_possession() {
        return Vec::new();
    }
    let carrier = state.ball.owner;
    let tactics = &state.team(defending).tactics;
    let attackers: Vec<(PlayerId, f32)> = state
        .team_players(defending.opponent())
        .iter()
        .filter(|p| !p.is_goalkeeper() && Some(p.id) != carrier)
        .map(|p| (p.id, threat(state, defending, p.id)))
        .filter(|&(_, t)| t > 0.12)
        .collect();

    let mut pairs: Vec<(f32, PlayerId, PlayerId, f32)> = Vec::new();
    for (slot, m) in state.team_players(defending).iter().enumerate() {
        if m.is_goalkeeper() || busy.contains(&m.id) || m.behaviour.marking_radius <= 0.0 {
            continue;
        }
        let home = shape[slot];
        // Players who don't track back only mark when the opponent comes to them.
        let radius = m.behaviour.marking_radius * (0.5 + 0.5 * m.behaviour.track_back);
        let home_local = defending.to_local(home);
        for &(aid, th) in &attackers {
            let a = state.player(aid);
            let d_home = home.distance(a.pos);
            if d_home > radius {
                continue;
            }
            // Positional leashes: centre-backs protect the middle, full-backs their flank.
            let al = defending.to_local(a.pos);
            match m.role {
                Role::CentreBack if al.y.abs() > 20.0 => continue,
                Role::FullBack if al.y * home_local.y.signum() < -6.0 => continue,
                _ => {}
            }
            let cost = d_home + 0.5 * m.pos.distance(a.pos) - 12.0 * th;
            pairs.push((cost, m.id, aid, th));
        }
    }
    pairs.sort_by(|a, b| cmp_f32(a.0, b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

    let mut used_m = [false; 32];
    let mut used_a = [false; 32];
    let mut out = Vec::new();
    for (_, mid, aid, th) in pairs {
        if used_m[mid.index()] || used_a[aid.index()] {
            continue;
        }
        used_m[mid.index()] = true;
        used_a[aid.index()] = true;
        let marker = state.player(mid);
        let ball_d = state.player(aid).pos.distance(state.ball.pos);
        let near_ball = (1.0 - ball_d / 30.0).clamp(0.0, 1.0);
        let phase_bonus = match phase {
            TeamPhase::HighPress => 0.15,
            TeamPhase::LowBlock => 0.1,
            _ => 0.0,
        };
        let tightness = (0.35 * marker.behaviour.marking_tightness
            + 0.3 * th
            + 0.25 * near_ball
            + tactics.marking_tightness
            + phase_bonus)
            .clamp(0.0, 1.0);
        out.push(MarkAssignment {
            marker: mid,
            target: aid,
            tightness,
        });
    }
    out.sort_by_key(|m| m.marker);
    out
}

/// Where a marker should stand.
pub fn marking_position(
    state: &MatchState,
    defending: TeamSide,
    attacker: PlayerId,
    tightness: f32,
) -> Vec2 {
    let a = state.player(attacker);
    // Anticipate: mark where the attacker is going, not where he is.
    let attacker_pos = a.pos + a.vel * 0.5;
    let goal = defending.own_goal();
    let to_goal = (goal - attacker_pos).normalize_or_zero();
    let to_ball = (state.ball.pos - attacker_pos).normalize_or_zero();
    let dist = 4.5 - 3.2 * tightness;
    attacker_pos + (to_goal * 0.75 + to_ball * 0.45).normalize_or_zero() * dist
}

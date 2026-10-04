//! Formations and formation-relative positioning.
//!
//! A [`Formation`] is a list of eleven [`Slot`]s expressed in *normalised
//! block coordinates*: `depth` runs from `0` (defensive line) to `1` (forward
//! line) and `lateral` from `-1` (right touchline side) to `+1` (left), both in
//! the team-local frame.
//!
//! [`team_shape`] stretches that template into a concrete target position per
//! player for the current phase: the block's height follows the ball and the
//! phase (high press vs low block), its depth and width depend on whether the
//! team has the ball, it slides across towards the ball, and individual
//! behaviours (attacking full-backs, forwards who don't track back, anchoring
//! midfielders, ...) bend individual slots.

use crate::math::Vec2;
use crate::phase::TeamPhase;
use crate::pitch::{clamp_to_pitch, TeamSide, HALF_LENGTH, HALF_WIDTH};
use crate::role::Role;
use crate::state::{MatchState, TEAM_SIZE};

/// A position in a formation template.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    pub role: Role,
    /// `0` = defensive line, `1` = forward line. Ignored for the goalkeeper.
    pub depth: f32,
    /// `-1` = right flank, `+1` = left flank (team-local).
    pub lateral: f32,
}

const fn s(role: Role, depth: f32, lateral: f32) -> Slot {
    Slot { role, depth, lateral }
}

/// Built-in formation templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormationKind {
    F442,
    F433,
    F4231,
    F4141,
    F352,
    F532,
}

impl FormationKind {
    pub const ALL: [FormationKind; 6] = [
        FormationKind::F442,
        FormationKind::F433,
        FormationKind::F4231,
        FormationKind::F4141,
        FormationKind::F352,
        FormationKind::F532,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FormationKind::F442 => "4-4-2",
            FormationKind::F433 => "4-3-3",
            FormationKind::F4231 => "4-2-3-1",
            FormationKind::F4141 => "4-1-4-1",
            FormationKind::F352 => "3-5-2",
            FormationKind::F532 => "5-3-2",
        }
    }
}

/// Eleven slots; slot 0 is always the goalkeeper.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Formation {
    pub name: &'static str,
    pub slots: [Slot; TEAM_SIZE],
}

impl Formation {
    pub fn new(kind: FormationKind) -> Self {
        use Role::*;
        let gk = s(Goalkeeper, 0.0, 0.0);
        let slots = match kind {
            FormationKind::F442 => [
                gk,
                s(FullBack, 0.03, 0.85),
                s(CentreBack, 0.0, 0.3),
                s(CentreBack, 0.0, -0.3),
                s(FullBack, 0.03, -0.85),
                s(Winger, 0.55, 0.85),
                s(CentralMid, 0.45, 0.25),
                s(CentralMid, 0.45, -0.25),
                s(Winger, 0.55, -0.85),
                s(Striker, 0.95, 0.2),
                s(Striker, 1.0, -0.2),
            ],
            FormationKind::F433 => [
                gk,
                s(FullBack, 0.03, 0.85),
                s(CentreBack, 0.0, 0.3),
                s(CentreBack, 0.0, -0.3),
                s(FullBack, 0.03, -0.85),
                s(DefensiveMid, 0.28, 0.0),
                s(CentralMid, 0.5, 0.42),
                s(CentralMid, 0.5, -0.42),
                s(Winger, 0.9, 0.85),
                s(Striker, 1.0, 0.0),
                s(Winger, 0.9, -0.85),
            ],
            FormationKind::F4231 => [
                gk,
                s(FullBack, 0.03, 0.85),
                s(CentreBack, 0.0, 0.3),
                s(CentreBack, 0.0, -0.3),
                s(FullBack, 0.03, -0.85),
                s(DefensiveMid, 0.3, 0.22),
                s(DefensiveMid, 0.3, -0.22),
                s(Winger, 0.72, 0.8),
                s(AttackingMid, 0.7, 0.0),
                s(Winger, 0.72, -0.8),
                s(Striker, 1.0, 0.0),
            ],
            FormationKind::F4141 => [
                gk,
                s(FullBack, 0.03, 0.85),
                s(CentreBack, 0.0, 0.3),
                s(CentreBack, 0.0, -0.3),
                s(FullBack, 0.03, -0.85),
                s(DefensiveMid, 0.25, 0.0),
                s(Winger, 0.6, 0.85),
                s(CentralMid, 0.55, 0.3),
                s(CentralMid, 0.55, -0.3),
                s(Winger, 0.6, -0.85),
                s(Striker, 1.0, 0.0),
            ],
            FormationKind::F352 => [
                gk,
                s(CentreBack, 0.0, 0.5),
                s(CentreBack, -0.03, 0.0),
                s(CentreBack, 0.0, -0.5),
                s(FullBack, 0.42, 0.95),
                s(DefensiveMid, 0.3, 0.0),
                s(CentralMid, 0.5, 0.35),
                s(CentralMid, 0.5, -0.35),
                s(FullBack, 0.42, -0.95),
                s(Striker, 0.95, 0.2),
                s(Striker, 1.0, -0.2),
            ],
            FormationKind::F532 => [
                gk,
                s(FullBack, 0.08, 0.9),
                s(CentreBack, 0.0, 0.45),
                s(CentreBack, -0.03, 0.0),
                s(CentreBack, 0.0, -0.45),
                s(FullBack, 0.08, -0.9),
                s(CentralMid, 0.45, 0.4),
                s(DefensiveMid, 0.38, 0.0),
                s(CentralMid, 0.45, -0.4),
                s(Striker, 0.95, 0.2),
                s(Striker, 1.0, -0.2),
            ],
        };
        Formation { name: kind.name(), slots }
    }

    pub fn role(&self, slot: usize) -> Role {
        self.slots[slot].role
    }
}

/// Height of the defensive line (team-local `x`) for a phase, given the ball's
/// local position. Before the tactics' `line_offset` is applied.
pub fn defensive_line_x(phase: TeamPhase, ball_local_x: f32) -> f32 {
    let b = ball_local_x;
    let line = match phase {
        TeamPhase::BuildUp => (b - 12.0).clamp(-38.0, -22.0),
        TeamPhase::Progression => (b - 26.0).clamp(-30.0, -2.0),
        TeamPhase::FinalThird => (b - 34.0).clamp(-8.0, 14.0),
        TeamPhase::AttackingTransition => (b - 28.0).clamp(-34.0, 4.0),
        TeamPhase::HighPress => (b - 36.0).clamp(-22.0, 8.0),
        TeamPhase::MidBlock => (b - 26.0).clamp(-34.0, -14.0),
        TeamPhase::LowBlock => (b - 14.0).clamp(-42.0, -34.0),
        TeamPhase::DefensiveTransition => (b - 22.0).clamp(-40.0, 4.0),
    };
    if phase.in_possession() {
        line
    } else {
        // Out of possession the line never stays above a deep ball: it drops
        // towards the six-yard box as the ball approaches the byline.
        line.min((b + 3.0).max(-HALF_LENGTH + 5.0))
    }
}

/// Target position for the goalkeeper (team-local), on the line from the goal
/// centre towards the ball, stepping out as a sweeper behind a high line.
pub fn goalkeeper_local(ball_local: Vec2, def_line: f32, in_possession: bool) -> Vec2 {
    let goal = Vec2::new(-HALF_LENGTH, 0.0);
    let to_ball = ball_local - goal;
    let dist = to_ball.length();
    let sweep = ((def_line + 30.0) * 0.35).clamp(0.0, 12.0);
    let mut d = (dist * 0.1).clamp(1.5, 6.0) + sweep;
    if in_possession {
        d = d.max(6.0);
    }
    d = d.min(dist * 0.6).max(1.0);
    let p = goal + to_ball.normalize_or_zero() * d;
    Vec2::new(p.x.max(-HALF_LENGTH + 0.5), p.y.clamp(-8.0, 8.0))
}

/// Concrete formation-relative target position for every player of `side`
/// (world space, indexed by slot).
pub fn team_shape(state: &MatchState, side: TeamSide, phase: TeamPhase) -> [Vec2; TEAM_SIZE] {
    let team = state.team(side);
    let t = &team.tactics;
    let ball = side.to_local(state.ball.pos);
    let in_poss = phase.in_possession();

    let mut def_line = defensive_line_x(phase, ball.x) + t.line_offset;
    // Drop off when the opponent's carrier has time to play a ball in behind.
    if !in_poss {
        if let Some(c) = state.carrier().filter(|c| c.team != side) {
            let free = state.nearest_distance(side, c.pos) > 6.0;
            let in_range = side.to_local(c.pos).x - def_line < 40.0;
            if free && in_range {
                def_line = (def_line - 6.0).max(-HALF_LENGTH + 6.0);
            }
        }
    }
    let depth = if in_poss { t.depth_in_possession } else { t.depth_out_of_possession };
    let width = if in_poss { t.width_in_possession } else { t.width_out_of_possession };
    // The block slides towards the ball; more so out of possession.
    let slide = if in_poss { 0.2 } else { 0.45 };
    let shift = (ball.y * slide).clamp(-(HALF_WIDTH - width).max(0.0) - 4.0, (HALF_WIDTH - width).max(0.0) + 4.0);
    let offside = state.offside_line_local(side);

    let mut out = [Vec2::ZERO; TEAM_SIZE];
    for (i, slot) in team.formation.slots.iter().enumerate() {
        let player = &state.team_players(side)[i];
        let b = &player.behaviour;
        if slot.role == Role::Goalkeeper {
            out[i] = side.to_world(goalkeeper_local(ball, def_line + b.possession_push, in_poss));
            continue;
        }

        let mut x = def_line + slot.depth * depth;
        let mut y = slot.lateral * width + shift;

        if in_poss {
            x += b.possession_push;
            if slot.role == Role::FullBack {
                x += t.fullback_push;
            }
            if slot.role == Role::DefensiveMid && t.anchor_dm {
                x = x.min(def_line + 10.0);
            }
            if slot.role == Role::FullBack && t.inverted_fullbacks {
                y = slot.lateral.signum() * 12.0 + shift;
                x = x.min(def_line + 0.4 * depth);
            }
            if slot.role == Role::Striker && t.false_nine {
                x -= 12.0;
            }
            if slot.role == Role::Winger && t.hug_touchline {
                y = slot.lateral.signum() * (HALF_WIDTH - 2.0);
            }
            // Attackers play on the shoulder of the last defender, not beyond.
            x = x.min(offside - 0.8);
        } else {
            // Players who do not track back hold a higher position, acting as
            // an outlet for the counter.
            let lazy = (1.0 - b.track_back).max(b.stays_high);
            let outlet = if t.counter_target && slot.role == Role::Striker { 1.0 } else { lazy };
            if outlet > 0.0 {
                let high = (ball.x - 5.0).max(def_line + depth).max(-5.0);
                x += (high - x).max(0.0) * outlet * 0.8;
            }
            if t.flank_protection && slot.role == Role::Winger {
                x = x.min(def_line + 0.4 * depth);
            }
            if slot.role == Role::DefensiveMid && t.anchor_dm {
                x = x.min(def_line + 9.0);
            }
        }

        out[i] = side.to_world(clamp_to_local(Vec2::new(x, y)));
    }
    out
}

fn clamp_to_local(p: Vec2) -> Vec2 {
    // Local and world frames share the same bounds.
    clamp_to_pitch(p, 1.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_formation_has_one_goalkeeper_first() {
        for kind in FormationKind::ALL {
            let f = Formation::new(kind);
            assert_eq!(f.slots[0].role, Role::Goalkeeper, "{}", f.name);
            assert_eq!(f.slots.iter().filter(|s| s.role == Role::Goalkeeper).count(), 1);
        }
    }

    #[test]
    fn formation_counts_match_name() {
        for kind in FormationKind::ALL {
            let f = Formation::new(kind);
            let defenders = f.slots.iter().filter(|s| s.role.is_defender() && s.depth < 0.2).count();
            let first_digit: usize = f.name[..1].parse().unwrap();
            assert_eq!(defenders, first_digit, "{}", f.name);
        }
    }

    #[test]
    fn formations_are_left_right_balanced() {
        for kind in FormationKind::ALL {
            let f = Formation::new(kind);
            let sum: f32 = f.slots.iter().map(|s| s.lateral).sum();
            assert!(sum.abs() < 1e-4, "{} lateral sum {sum}", f.name);
        }
    }

    #[test]
    fn defensive_line_ordering() {
        // With the ball in the same place, a high press defends higher than a
        // mid block, which defends higher than a low block.
        for bx in [-30.0, 0.0, 30.0] {
            let hp = defensive_line_x(TeamPhase::HighPress, bx);
            let mb = defensive_line_x(TeamPhase::MidBlock, bx);
            let lb = defensive_line_x(TeamPhase::LowBlock, bx);
            assert!(hp >= mb && mb >= lb, "ball {bx}: {hp} {mb} {lb}");
        }
    }

    #[test]
    fn goalkeeper_stays_near_goal() {
        for (bx, by) in [(-50.0, 0.0), (-40.0, 20.0), (0.0, 0.0), (50.0, -30.0)] {
            let gk = goalkeeper_local(Vec2::new(bx, by), -30.0, false);
            assert!(gk.x < -HALF_LENGTH + 15.0, "{gk:?}");
            assert!(gk.y.abs() <= 8.0);
        }
    }
}

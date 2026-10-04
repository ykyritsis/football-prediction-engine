//! Possession phases.
//!
//! Each team is always in exactly one [`TeamPhase`]. The phase is derived from
//! who has the ball, where it is (in the team's local frame), how recently
//! possession changed and the team's [`Tactics`]. A small hysteresis band
//! around each threshold stops the phase flickering when the ball sits on a
//! boundary.

use crate::pitch::{TeamSide, PITCH_LENGTH};
use crate::state::MatchState;
use crate::tactics::Tactics;

/// Phase of play from one team's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TeamPhase {
    // --- in possession ---
    /// Just won the ball: look to exploit the disorganised opponent.
    AttackingTransition,
    /// Ball in own third: patient circulation, wide centre-backs.
    BuildUp,
    /// Ball in the middle third: progress through the lines.
    Progression,
    /// Ball in the final third: create and finish chances.
    FinalThird,
    // --- out of possession ---
    /// Just lost the ball: counter-press or recover shape.
    DefensiveTransition,
    /// Opponent building up deep: press high.
    HighPress,
    /// Opponent in the middle third: compact mid block.
    MidBlock,
    /// Opponent in our third: deep, protect the box.
    LowBlock,
}

impl TeamPhase {
    pub const ALL: [TeamPhase; 8] = [
        TeamPhase::AttackingTransition,
        TeamPhase::BuildUp,
        TeamPhase::Progression,
        TeamPhase::FinalThird,
        TeamPhase::DefensiveTransition,
        TeamPhase::HighPress,
        TeamPhase::MidBlock,
        TeamPhase::LowBlock,
    ];

    pub fn in_possession(self) -> bool {
        matches!(
            self,
            TeamPhase::AttackingTransition
                | TeamPhase::BuildUp
                | TeamPhase::Progression
                | TeamPhase::FinalThird
        )
    }

    pub fn is_transition(self) -> bool {
        matches!(
            self,
            TeamPhase::AttackingTransition | TeamPhase::DefensiveTransition
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            TeamPhase::AttackingTransition => "Attacking transition",
            TeamPhase::BuildUp => "Build-up",
            TeamPhase::Progression => "Progression",
            TeamPhase::FinalThird => "Final third",
            TeamPhase::DefensiveTransition => "Defensive transition",
            TeamPhase::HighPress => "High press",
            TeamPhase::MidBlock => "Mid block",
            TeamPhase::LowBlock => "Low block",
        }
    }
}

/// Hysteresis half-width (m) around phase thresholds.
pub const PHASE_HYSTERESIS: f32 = 3.0;

/// Inputs to [`determine_phase`], separated from [`MatchState`] for testing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseInputs {
    pub in_possession: bool,
    /// Ball `x` in this team's local frame.
    pub ball_local_x: f32,
    /// Seconds since possession last changed.
    pub seconds_since_change: f32,
    /// Whether play is stopped for a restart (no transitions during restarts).
    pub dead_ball: bool,
}

/// In-possession thresholds on the ball's local `x`: (build-up | progression | final third).
fn possession_thresholds() -> (f32, f32) {
    let third = PITCH_LENGTH / 6.0;
    (-third, third)
}

/// Out-of-possession thresholds on the ball's local `x`: (low | mid | high).
/// A higher `press_height` lowers the high-press threshold.
fn defending_thresholds(t: &Tactics) -> (f32, f32) {
    let third = PITCH_LENGTH / 6.0;
    let shift = (t.press_height - 0.5) * 20.0;
    (-third - shift * 0.5, third - shift)
}

fn band(prev: Option<i32>, x: f32, lo: f32, hi: f32) -> i32 {
    // Returns 0 (below lo), 1 (between), 2 (above hi), staying in the previous
    // band while within the hysteresis margin.
    let raw = if x < lo {
        0
    } else if x > hi {
        2
    } else {
        1
    };
    let Some(p) = prev else { return raw };
    if p == raw {
        return raw;
    }
    let sticky = match (p, raw) {
        (0, _) => x < lo + PHASE_HYSTERESIS,
        (2, _) => x > hi - PHASE_HYSTERESIS,
        (1, 0) => x > lo - PHASE_HYSTERESIS,
        (1, 2) => x < hi + PHASE_HYSTERESIS,
        _ => false,
    };
    if sticky {
        p
    } else {
        raw
    }
}

/// Pure phase function.
pub fn determine_phase(prev: TeamPhase, inputs: PhaseInputs, tactics: &Tactics) -> TeamPhase {
    let transition = !inputs.dead_ball && inputs.seconds_since_change < tactics.transition_seconds;
    if inputs.in_possession {
        if transition {
            // Transition ends early once the ball reaches the final third.
            let (_, hi) = possession_thresholds();
            if inputs.ball_local_x < hi {
                return TeamPhase::AttackingTransition;
            }
        }
        let (lo, hi) = possession_thresholds();
        let prev_band = match prev {
            TeamPhase::BuildUp => Some(0),
            TeamPhase::Progression => Some(1),
            TeamPhase::FinalThird => Some(2),
            _ => None,
        };
        match band(prev_band, inputs.ball_local_x, lo, hi) {
            0 => TeamPhase::BuildUp,
            1 => TeamPhase::Progression,
            _ => TeamPhase::FinalThird,
        }
    } else {
        let (lo, hi) = defending_thresholds(tactics);
        // Transition ends early once the ball is deep in the own half: the
        // team settles into its block.
        if transition && inputs.ball_local_x >= lo {
            return TeamPhase::DefensiveTransition;
        }
        let prev_band = match prev {
            TeamPhase::LowBlock => Some(0),
            TeamPhase::MidBlock => Some(1),
            TeamPhase::HighPress => Some(2),
            _ => None,
        };
        match band(prev_band, inputs.ball_local_x, lo, hi) {
            0 => TeamPhase::LowBlock,
            1 => TeamPhase::MidBlock,
            _ => TeamPhase::HighPress,
        }
    }
}

/// Compute the phase of `side` from a match snapshot.
pub fn phase_for(state: &MatchState, side: TeamSide) -> TeamPhase {
    let team = state.team(side);
    let inputs = PhaseInputs {
        in_possession: state.possession == side,
        ball_local_x: side.to_local(state.ball.pos).x,
        seconds_since_change: state.seconds_since(state.possession_since),
        dead_ball: state.restart.is_some(),
    };
    determine_phase(team.phase, inputs, &team.tactics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp(pos: bool, x: f32, secs: f32) -> PhaseInputs {
        PhaseInputs {
            in_possession: pos,
            ball_local_x: x,
            seconds_since_change: secs,
            dead_ball: false,
        }
    }

    #[test]
    fn in_possession_phases_follow_ball() {
        let t = Tactics::default();
        assert_eq!(
            determine_phase(TeamPhase::BuildUp, inp(true, -40.0, 30.0), &t),
            TeamPhase::BuildUp
        );
        assert_eq!(
            determine_phase(TeamPhase::Progression, inp(true, 0.0, 30.0), &t),
            TeamPhase::Progression
        );
        assert_eq!(
            determine_phase(TeamPhase::FinalThird, inp(true, 40.0, 30.0), &t),
            TeamPhase::FinalThird
        );
    }

    #[test]
    fn out_of_possession_phases_follow_ball() {
        let t = Tactics::default();
        // Ball deep in the opponent's half (high local x) => press high.
        assert_eq!(
            determine_phase(TeamPhase::MidBlock, inp(false, 40.0, 30.0), &t),
            TeamPhase::HighPress
        );
        assert_eq!(
            determine_phase(TeamPhase::MidBlock, inp(false, 0.0, 30.0), &t),
            TeamPhase::MidBlock
        );
        assert_eq!(
            determine_phase(TeamPhase::MidBlock, inp(false, -40.0, 30.0), &t),
            TeamPhase::LowBlock
        );
    }

    #[test]
    fn transitions_after_possession_change() {
        let t = Tactics::default();
        assert_eq!(
            determine_phase(TeamPhase::MidBlock, inp(true, 0.0, 1.0), &t),
            TeamPhase::AttackingTransition
        );
        assert_eq!(
            determine_phase(TeamPhase::Progression, inp(false, 0.0, 1.0), &t),
            TeamPhase::DefensiveTransition
        );
        // No transition during dead balls.
        let mut i = inp(false, 0.0, 1.0);
        i.dead_ball = true;
        assert_eq!(
            determine_phase(TeamPhase::Progression, i, &t),
            TeamPhase::MidBlock
        );
    }

    #[test]
    fn attacking_transition_ends_in_final_third() {
        let t = Tactics::default();
        assert_eq!(
            determine_phase(TeamPhase::AttackingTransition, inp(true, 40.0, 1.0), &t),
            TeamPhase::FinalThird
        );
    }

    #[test]
    fn hysteresis_prevents_flicker() {
        let t = Tactics::default();
        let edge = PITCH_LENGTH / 6.0;
        // Just past the threshold, the previous phase sticks.
        assert_eq!(
            determine_phase(TeamPhase::Progression, inp(true, edge + 1.0, 30.0), &t),
            TeamPhase::Progression
        );
        // Well past it, the phase changes.
        assert_eq!(
            determine_phase(TeamPhase::Progression, inp(true, edge + 5.0, 30.0), &t),
            TeamPhase::FinalThird
        );
        // And coming back requires crossing the band too.
        assert_eq!(
            determine_phase(TeamPhase::FinalThird, inp(true, edge - 1.0, 30.0), &t),
            TeamPhase::FinalThird
        );
        assert_eq!(
            determine_phase(TeamPhase::FinalThird, inp(true, edge - 5.0, 30.0), &t),
            TeamPhase::Progression
        );
    }

    #[test]
    fn press_height_moves_high_press_threshold() {
        let high = Tactics::high_press();
        let low = Tactics::low_block();
        // Ball just inside the opponent's half.
        let x = 15.0;
        assert_eq!(
            determine_phase(TeamPhase::MidBlock, inp(false, x, 30.0), &high),
            TeamPhase::HighPress
        );
        assert_eq!(
            determine_phase(TeamPhase::MidBlock, inp(false, x, 30.0), &low),
            TeamPhase::MidBlock
        );
    }

    #[test]
    fn possession_flag_partitions_phases() {
        let t = Tactics::default();
        for x in [-50.0, -20.0, 0.0, 20.0, 50.0] {
            for secs in [0.0, 10.0] {
                assert!(
                    determine_phase(TeamPhase::MidBlock, inp(true, x, secs), &t).in_possession()
                );
                assert!(
                    !determine_phase(TeamPhase::MidBlock, inp(false, x, secs), &t).in_possession()
                );
            }
        }
    }
}

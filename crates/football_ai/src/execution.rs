//! Execution: how well a player carries out what he decided.
//!
//! The decision layer plans with a player's *expected* ability (his
//! attributes). What actually happens in the simulation uses his *execution*
//! ability, which differs in two ways that let players beat expectations:
//!
//! * **Star quality** is non-linear: above 0.8 an attribute counts for more
//!   than its face value, so elite finishers, dribblers, passers and keepers
//!   produce results the model does not predict.
//! * **Match-day form** (`Player::form`, PES condition arrows) scales
//!   execution up or down for the whole match. Part of it is shared by the
//!   team (a team can have a good or bad day), part is individual, and its
//!   size depends on the player's consistency. Fighting Spirit lifts a player
//!   when his team is behind.

use crate::rng::SimRng;
use crate::state::{MatchState, Player};

/// Above this, attributes count for more than their face value.
pub const STAR_KNEE: f32 = 0.8;
/// Extra weight per unit above the knee.
pub const STAR_GAIN: f32 = 0.9;
/// Ceiling on dribbling in duels (see [`executing`]).
pub const DUEL_DRIBBLING_CAP: f32 = 0.95;
/// Ceiling on execution values: even a star on his best day is not flawless.
pub const EXEC_CAP: f32 = 1.1;
/// Execution change per unit of form.
pub const FORM_EFFECT: f32 = 0.25;

/// Non-linear star curve for an attribute in `0..1`.
pub fn star(x: f32) -> f32 {
    x + (x - STAR_KNEE).max(0.0) * STAR_GAIN
}

/// Effective form right now: match-day form, in-match confidence and
/// situational boosts.
pub fn effective_form(state: &MatchState, p: &Player) -> f32 {
    let t = p.team.index();
    let behind = state.score[t] < state.score[1 - t];
    let spirit = if p.fighting_spirit && behind {
        0.4
    } else {
        0.0
    };
    (p.form + p.confidence + spirit).clamp(-1.0, 1.5)
}

/// Execution value of an attribute for `p`.
pub fn exec(state: &MatchState, p: &Player, attribute: f32) -> f32 {
    (star(attribute) * (1.0 + FORM_EFFECT * effective_form(state, p))).clamp(0.0, EXEC_CAP)
}

/// A copy of `p` whose technical attributes are replaced by their execution
/// values (for resolving duels with the shared rules functions).
pub fn executing(state: &MatchState, p: &Player) -> Player {
    let mut q = p.clone();
    let a = &mut q.attrs;
    a.passing = exec(state, p, a.passing);
    a.long_passing = exec(state, p, a.long_passing);
    a.control = exec(state, p, a.control);
    // In a duel even the best dribbler can be stopped: past this point skill
    // shows in how often he tries and where he goes, not in invincibility.
    a.dribbling = exec(state, p, a.dribbling).min(DUEL_DRIBBLING_CAP);
    a.tackling = exec(state, p, a.tackling);
    a.finishing = exec(state, p, a.finishing);
    a.set_pieces = exec(state, p, a.set_pieces);
    a.goalkeeping = exec(state, p, a.goalkeeping);
    q
}

/// Draw match-day form for a squad: `team_day` is shared, each player adds
/// his own swing, and steadier players swing less.
pub fn draw_form(rng: &mut SimRng, consistency: f32, team_day: f32) -> f32 {
    let own = rng.range(-1.0, 1.0);
    let swing = 1.25 - 0.75 * consistency;
    ((0.55 * team_day + 0.45 * own) * swing).clamp(-1.0, 1.0)
}

/// PES-style condition arrow for a form value.
pub fn condition_label(form: f32) -> &'static str {
    match form {
        f if f >= 0.5 => "excellent",
        f if f >= 0.15 => "good",
        f if f > -0.15 => "normal",
        f if f > -0.5 => "poor",
        _ => "terrible",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_curve_only_lifts_the_elite() {
        assert_eq!(star(0.6), 0.6);
        assert_eq!(star(0.8), 0.8);
        assert!(star(0.95) > 0.95 + 0.1);
        assert!(star(0.95) - 0.95 > star(0.85) - 0.85);
    }

    #[test]
    fn consistent_players_swing_less() {
        let mut rng = SimRng::new(3);
        let (mut steady, mut volatile) = (0.0f32, 0.0f32);
        for _ in 0..2000 {
            steady += draw_form(&mut rng, 1.0, 0.0).abs();
            volatile += draw_form(&mut rng, 0.0, 0.0).abs();
        }
        assert!(volatile > steady * 1.8, "{volatile} vs {steady}");
    }

    #[test]
    fn labels_cover_the_range() {
        assert_eq!(condition_label(0.9), "excellent");
        assert_eq!(condition_label(0.0), "normal");
        assert_eq!(condition_label(-0.9), "terrible");
    }
}

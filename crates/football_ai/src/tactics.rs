//! Low-level team tactics consumed by the decision layer.
//!
//! [`Tactics`] is the *compiled* form of a team's identity: plain numbers the
//! positioning, pressing, passing and support code read directly. Designers
//! (and imported team data) work with the higher level
//! [`crate::identity::TeamStyle`], which compiles down to this struct.

/// Numeric team instructions. [`Tactics::default`] is a balanced mid-block
/// team with mixed passing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tactics {
    // ---- shape ----
    /// Half-width (m) of the team shape in possession.
    pub width_in_possession: f32,
    /// Half-width (m) of the team shape out of possession.
    pub width_out_of_possession: f32,
    /// Distance (m) from defensive line to forward line in possession.
    pub depth_in_possession: f32,
    /// Distance (m) from defensive line to forward line out of possession.
    pub depth_out_of_possession: f32,
    /// Additional height (m) applied to the defensive line in every phase.
    pub line_offset: f32,
    /// How far players may drift from their formation slot to support play
    /// (`0` = rigid, `1` = fluid / rotations).
    pub fluidity: f32,
    /// How close supporting players come to the ball carrier (`0` = far, `1`
    /// = very close, tiki-taka triangles).
    pub support_closeness: f32,
    /// Extra push (m) for full-backs in possession (attacking full-backs).
    pub fullback_push: f32,
    /// Wingers stay on the touchline in possession.
    pub hug_touchline: bool,
    /// Defensive midfielder anchors in front of the defence in every phase.
    pub anchor_dm: bool,
    /// Willingness of extra players to attack the box in the final third.
    pub box_crowding: f32,
    /// The centre-forward drops into midfield in possession (false nine).
    pub false_nine: bool,
    /// Full-backs tuck into midfield in possession (false/inverted full-backs).
    pub inverted_fullbacks: bool,

    // ---- out of possession ----
    /// How far up the pitch the team presses, `0` (deep) to `1` (high).
    pub press_height: f32,
    /// Multiplier on pressing-trigger intensity.
    pub press_aggression: f32,
    /// Seconds after a possession change that count as a transition.
    pub transition_seconds: f32,
    /// Counter-press intensity in defensive transition (`0` = drop off and
    /// regroup, `1` = swarm the ball, gegenpressing).
    pub counter_press: f32,
    /// Global marking tightness bonus.
    pub marking_tightness: f32,
    /// Forwards are excused defensive duties and stay high as an outlet.
    pub counter_target: bool,
    /// Wide players track back to protect the flanks.
    pub flank_protection: bool,
    /// Steer the opponent wide (`1`) or into the middle (`0`) when containing.
    pub show_wide: f32,

    // ---- in possession ----
    /// `0` = patient and short, `1` = direct and vertical.
    pub directness: f32,
    /// Preference for long, lofted passes over the top (long-ball teams).
    pub long_ball: f32,
    /// Preference for attacking down the flanks (`1`) or the centre (`0`).
    pub wide_play: f32,
    /// Preference for crossing in the final third.
    pub crossing: f32,
    /// Speed of ball circulation (`1` = one/two-touch, `0` = slow build).
    pub tempo: f32,
    /// Team-level multiplier on risk aversion (`< 1` = adventurous).
    pub risk_aversion: f32,
    /// Look for forwards running in behind as soon as possession is won.
    pub counter_attack: f32,
    /// Added to every player's xG threshold for shooting: patient teams work
    /// the ball for a better chance (`> 0`), direct teams shoot on sight (`< 0`).
    pub shot_selectivity: f32,
}

impl Default for Tactics {
    fn default() -> Self {
        Self {
            width_in_possession: 30.0,
            width_out_of_possession: 21.0,
            depth_in_possession: 42.0,
            depth_out_of_possession: 30.0,
            line_offset: 0.0,
            fluidity: 0.5,
            support_closeness: 0.5,
            fullback_push: 0.0,
            hug_touchline: false,
            anchor_dm: false,
            box_crowding: 0.5,
            false_nine: false,
            inverted_fullbacks: false,
            press_height: 0.5,
            press_aggression: 1.0,
            transition_seconds: 4.0,
            counter_press: 0.5,
            marking_tightness: 0.0,
            counter_target: false,
            flank_protection: false,
            show_wide: 0.6,
            directness: 0.5,
            long_ball: 0.3,
            wide_play: 0.5,
            crossing: 0.4,
            tempo: 0.5,
            risk_aversion: 1.0,
            counter_attack: 0.5,
            shot_selectivity: 0.0,
        }
    }
}

impl Tactics {
    /// Aggressive, high-pressing preset.
    pub fn high_press() -> Self {
        Self {
            press_height: 0.9,
            press_aggression: 1.4,
            counter_press: 0.9,
            line_offset: 6.0,
            depth_out_of_possession: 27.0,
            ..Self::default()
        }
    }

    /// Deep, compact preset.
    pub fn low_block() -> Self {
        Self {
            press_height: 0.1,
            press_aggression: 0.7,
            counter_press: 0.2,
            line_offset: -6.0,
            width_out_of_possession: 18.0,
            depth_out_of_possession: 25.0,
            ..Self::default()
        }
    }
}

/// How a team adapts to the scoreline late in a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mentality {
    /// Play to the team's style.
    Normal,
    /// Behind: push up, press harder, go more direct.
    Chasing,
    /// Behind late on: everything forward.
    AllOut,
    /// Ahead late on: sit deeper, take no risks, keep an outlet.
    Protecting,
}

impl Mentality {
    pub fn name(self) -> &'static str {
        match self {
            Mentality::Normal => "normal",
            Mentality::Chasing => "chasing the game",
            Mentality::AllOut => "all out attack",
            Mentality::Protecting => "protecting the lead",
        }
    }

    /// Choose a mentality from the goal difference (own - opponent) and the
    /// match minute.
    pub fn for_situation(goal_difference: i32, minute: f32) -> Mentality {
        match goal_difference {
            d if d < 0 && minute >= 80.0 => Mentality::AllOut,
            d if d < 0 && minute >= 55.0 => Mentality::Chasing,
            d if d <= -2 && minute >= 30.0 => Mentality::Chasing,
            d if d > 0 && minute >= 65.0 => Mentality::Protecting,
            d if d >= 2 && minute >= 55.0 => Mentality::Protecting,
            _ => Mentality::Normal,
        }
    }

    /// Apply to a team's base tactics.
    pub fn apply(self, base: &Tactics) -> Tactics {
        let mut t = *base;
        match self {
            Mentality::Normal => {}
            Mentality::Chasing | Mentality::AllOut => {
                let k = if self == Mentality::AllOut { 2.0 } else { 1.0 };
                t.line_offset += 4.0 * k;
                t.press_height = (t.press_height + 0.15 * k).min(1.0);
                t.press_aggression *= 1.0 + 0.15 * k;
                t.directness = (t.directness + 0.15 * k).min(1.0);
                t.risk_aversion *= 1.0 - 0.15 * k;
                t.box_crowding = (t.box_crowding + 0.25 * k).min(1.0);
                t.fullback_push += 3.0 * k;
                t.counter_target = false;
            }
            Mentality::Protecting => {
                t.line_offset -= 6.0;
                t.press_height = (t.press_height - 0.25).max(0.0);
                t.press_aggression *= 0.8;
                t.risk_aversion *= 1.3;
                t.fullback_push = 0.0;
                t.counter_target = true;
                t.depth_out_of_possession -= 3.0;
            }
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentality_follows_score_and_time() {
        assert_eq!(Mentality::for_situation(-1, 50.0), Mentality::Normal);
        assert_eq!(Mentality::for_situation(-1, 65.0), Mentality::Chasing);
        assert_eq!(Mentality::for_situation(-2, 85.0), Mentality::AllOut);
        assert_eq!(Mentality::for_situation(1, 60.0), Mentality::Normal);
        assert_eq!(Mentality::for_situation(1, 70.0), Mentality::Protecting);
        assert_eq!(Mentality::for_situation(-2, 35.0), Mentality::Chasing);
        assert_eq!(Mentality::for_situation(0, 89.0), Mentality::Normal);
    }

    #[test]
    fn chasing_pushes_up_and_protecting_drops_off() {
        let base = Tactics::default();
        let chase = Mentality::Chasing.apply(&base);
        let all_out = Mentality::AllOut.apply(&base);
        let protect = Mentality::Protecting.apply(&base);
        assert!(chase.line_offset > base.line_offset && all_out.line_offset > chase.line_offset);
        assert!(chase.risk_aversion < base.risk_aversion);
        assert!(protect.line_offset < base.line_offset);
        assert!(protect.risk_aversion > base.risk_aversion);
        assert_eq!(Mentality::Normal.apply(&base), base);
    }
}

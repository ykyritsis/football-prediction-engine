//! Player roles and the behavioural parameters attached to them.
//!
//! A [`Role`] describes *what a player is asked to do*; the [`Behaviour`]
//! turns that into numbers consumed by the decision layer (how willing the
//! player is to press, how far he pushes on in possession, how much risk he
//! accepts on the ball, ...).
//!
//! [`Behaviour::for_role`] gives the role defaults. Individual players then
//! modify those defaults through their identity (playing style, skills, COM
//! styles; see [`crate::identity`]), which is how a dribbler feels different
//! from a poacher even when both are deployed as wingers.

/// Tactical role of a formation slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Role {
    Goalkeeper,
    CentreBack,
    FullBack,
    DefensiveMid,
    CentralMid,
    AttackingMid,
    Winger,
    Striker,
}

impl Role {
    pub const ALL: [Role; 8] = [
        Role::Goalkeeper,
        Role::CentreBack,
        Role::FullBack,
        Role::DefensiveMid,
        Role::CentralMid,
        Role::AttackingMid,
        Role::Winger,
        Role::Striker,
    ];

    pub fn short_name(self) -> &'static str {
        match self {
            Role::Goalkeeper => "GK",
            Role::CentreBack => "CB",
            Role::FullBack => "FB",
            Role::DefensiveMid => "DM",
            Role::CentralMid => "CM",
            Role::AttackingMid => "AM",
            Role::Winger => "W",
            Role::Striker => "ST",
        }
    }

    pub fn is_defender(self) -> bool {
        matches!(self, Role::CentreBack | Role::FullBack)
    }

    pub fn is_forward(self) -> bool {
        matches!(self, Role::Winger | Role::Striker)
    }

    /// Role default behaviour.
    pub fn behaviour(self) -> Behaviour {
        Behaviour::for_role(self)
    }
}

/// Behavioural tuning consumed by the decision layer. Factors are in `[0, 1]`
/// unless noted otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behaviour {
    /// Multiplier on the cost of losing the ball when evaluating actions.
    /// Higher = more conservative (`> 0`, ~1 is neutral).
    pub risk_aversion: f32,
    /// Willingness to be selected as the primary presser.
    pub press_willingness: f32,
    /// Radius (m) around the shape position inside which the player picks up
    /// an opponent to mark.
    pub marking_radius: f32,
    /// How tightly the player marks (0 = zonal/loose, 1 = touch-tight).
    pub marking_tightness: f32,
    /// Extra forward push (m, team-local `x`) of the shape position in possession.
    pub possession_push: f32,
    /// Tendency to make runs in behind the defensive line.
    pub run_in_behind: f32,
    /// Tendency to overlap/underlap the ball carrier.
    pub overlap: f32,
    /// Tendency to arrive late in the box when the ball is wide in the final third.
    pub box_presence: f32,
    /// Stays high on the last line and does little defending (poacher /
    /// counter-attack outlet). Reduces defensive tracking.
    pub stays_high: f32,
    /// How diligently the player tracks back when out of possession.
    pub track_back: f32,
    /// Whether the role forms part of the rest-defence when in possession.
    pub rest_defence: bool,
    /// Expected-goals threshold above which the player prefers to shoot.
    pub shot_threshold: f32,
    /// Maximum distance (m) from goal the player will shoot from.
    pub shot_range: f32,
    /// Preference for carrying the ball over releasing it.
    pub dribble_bias: f32,
    /// Preference for through/lead passes into runners.
    pub through_ball_bias: f32,
    /// Preference for long, lofted passes.
    pub long_pass_bias: f32,
    /// Preference for crossing from wide areas.
    pub cross_bias: f32,
    /// Shields and holds the ball up under pressure (target man).
    pub hold_up: f32,
    /// Speed of play: 1 = releases the ball as soon as possible, 0 = takes
    /// extra touches before acting.
    pub tempo: f32,
}

impl Behaviour {
    pub fn for_role(role: Role) -> Self {
        let base = Behaviour {
            risk_aversion: 1.0,
            press_willingness: 0.7,
            marking_radius: 15.0,
            marking_tightness: 0.5,
            possession_push: 6.0,
            run_in_behind: 0.2,
            overlap: 0.2,
            box_presence: 0.2,
            stays_high: 0.0,
            track_back: 0.7,
            rest_defence: false,
            shot_threshold: 0.13,
            shot_range: 25.0,
            dribble_bias: 0.4,
            through_ball_bias: 0.3,
            long_pass_bias: 0.3,
            cross_bias: 0.3,
            hold_up: 0.3,
            tempo: 0.5,
        };
        match role {
            Role::Goalkeeper => Behaviour {
                risk_aversion: 2.2,
                press_willingness: 0.0,
                marking_radius: 0.0,
                possession_push: 0.0,
                run_in_behind: 0.0,
                overlap: 0.0,
                box_presence: 0.0,
                track_back: 1.0,
                rest_defence: true,
                shot_threshold: 1.0,
                shot_range: 0.0,
                dribble_bias: 0.0,
                through_ball_bias: 0.1,
                long_pass_bias: 0.5,
                cross_bias: 0.0,
                hold_up: 0.0,
                ..base
            },
            Role::CentreBack => Behaviour {
                risk_aversion: 1.6,
                press_willingness: 0.45,
                marking_radius: 20.0,
                marking_tightness: 0.6,
                possession_push: 2.0,
                run_in_behind: 0.0,
                overlap: 0.0,
                box_presence: 0.0,
                track_back: 1.0,
                rest_defence: true,
                shot_threshold: 0.25,
                shot_range: 18.0,
                dribble_bias: 0.15,
                through_ball_bias: 0.15,
                long_pass_bias: 0.35,
                cross_bias: 0.0,
                ..base
            },
            Role::FullBack => Behaviour {
                risk_aversion: 1.25,
                marking_radius: 18.0,
                possession_push: 14.0,
                run_in_behind: 0.15,
                overlap: 0.9,
                box_presence: 0.05,
                track_back: 0.95,
                shot_threshold: 0.2,
                shot_range: 20.0,
                cross_bias: 0.6,
                ..base
            },
            Role::DefensiveMid => Behaviour {
                risk_aversion: 1.3,
                press_willingness: 0.75,
                marking_radius: 16.0,
                possession_push: 3.0,
                run_in_behind: 0.0,
                overlap: 0.1,
                box_presence: 0.05,
                track_back: 1.0,
                rest_defence: true,
                shot_threshold: 0.17,
                dribble_bias: 0.25,
                ..base
            },
            Role::CentralMid => Behaviour {
                press_willingness: 0.85,
                possession_push: 8.0,
                overlap: 0.6,
                box_presence: 0.35,
                track_back: 0.85,
                dribble_bias: 0.45,
                through_ball_bias: 0.4,
                ..base
            },
            Role::AttackingMid => Behaviour {
                risk_aversion: 0.75,
                press_willingness: 0.8,
                marking_radius: 12.0,
                marking_tightness: 0.4,
                possession_push: 8.0,
                run_in_behind: 0.4,
                overlap: 0.3,
                box_presence: 0.5,
                track_back: 0.55,
                shot_threshold: 0.1,
                shot_range: 27.0,
                dribble_bias: 0.65,
                through_ball_bias: 0.6,
                long_pass_bias: 0.2,
                ..base
            },
            Role::Winger => Behaviour {
                risk_aversion: 0.7,
                press_willingness: 0.8,
                marking_radius: 12.0,
                marking_tightness: 0.4,
                possession_push: 6.0,
                run_in_behind: 0.7,
                box_presence: 0.5,
                track_back: 0.6,
                shot_threshold: 0.1,
                dribble_bias: 0.6,
                through_ball_bias: 0.35,
                long_pass_bias: 0.15,
                cross_bias: 0.65,
                ..base
            },
            Role::Striker => Behaviour {
                risk_aversion: 0.6,
                press_willingness: 1.0,
                marking_radius: 10.0,
                marking_tightness: 0.3,
                possession_push: 4.0,
                run_in_behind: 1.0,
                box_presence: 1.0,
                stays_high: 0.3,
                track_back: 0.35,
                shot_threshold: 0.08,
                shot_range: 24.0,
                dribble_bias: 0.55,
                through_ball_bias: 0.25,
                long_pass_bias: 0.1,
                cross_bias: 0.1,
                hold_up: 0.6,
                ..base
            },
        }
    }

    /// Clamp every factor into its documented range. Identity modifiers are
    /// additive, so they are always followed by a call to this.
    pub fn clamped(mut self) -> Self {
        let u = |v: &mut f32| *v = v.clamp(0.0, 1.0);
        self.risk_aversion = self.risk_aversion.clamp(0.2, 3.0);
        u(&mut self.press_willingness);
        self.marking_radius = self.marking_radius.clamp(0.0, 30.0);
        u(&mut self.marking_tightness);
        self.possession_push = self.possession_push.clamp(-10.0, 25.0);
        u(&mut self.run_in_behind);
        u(&mut self.overlap);
        u(&mut self.box_presence);
        u(&mut self.stays_high);
        u(&mut self.track_back);
        self.shot_threshold = self.shot_threshold.clamp(0.02, 1.0);
        self.shot_range = self.shot_range.clamp(0.0, 40.0);
        u(&mut self.dribble_bias);
        u(&mut self.through_ball_bias);
        u(&mut self.long_pass_bias);
        u(&mut self.cross_bias);
        u(&mut self.hold_up);
        u(&mut self.tempo);
        self
    }
}

/// Physical and technical attributes used by the simulation. Speeds are in
/// m/s and m/s², everything else normalised to `[0, 1]`.
///
/// These are derived from the PES-style 40–99 ratings in
/// [`crate::identity::Ratings`]; [`Attributes::for_role`] provides neutral
/// defaults for tests and quick setups.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attributes {
    pub top_speed: f32,
    pub acceleration: f32,
    /// Short/ground passing accuracy.
    pub passing: f32,
    /// Long/lofted passing accuracy.
    pub long_passing: f32,
    /// First touch and ball control.
    pub control: f32,
    /// Close control while running with the ball; resistance to tackles.
    pub dribbling: f32,
    pub tackling: f32,
    pub finishing: f32,
    /// Shot power, affects long-range shooting.
    pub kicking_power: f32,
    /// Dead-ball delivery: free kicks, corners, penalties placement.
    pub set_pieces: f32,
    /// How slowly energy drains.
    pub stamina: f32,
    /// Resistance to injury `0..1`.
    pub injury_resistance: f32,
    /// How steady the player's form is from match to match `0..1` (PES "Form").
    pub consistency: f32,
    /// Winning the ball in the air: heading, jump, strength and height.
    pub aerial: f32,
    pub goalkeeping: f32,
}

impl Attributes {
    pub fn for_role(role: Role) -> Self {
        let base = Attributes {
            top_speed: 8.0,
            acceleration: 6.0,
            passing: 0.7,
            long_passing: 0.65,
            control: 0.7,
            dribbling: 0.65,
            tackling: 0.6,
            finishing: 0.5,
            kicking_power: 0.65,
            set_pieces: 0.5,
            stamina: 0.7,
            injury_resistance: 0.5,
            consistency: 0.5,
            aerial: 0.6,
            goalkeeping: 0.0,
        };
        match role {
            Role::Goalkeeper => Attributes {
                top_speed: 6.5,
                passing: 0.6,
                goalkeeping: 0.8,
                tackling: 0.3,
                dribbling: 0.4,
                ..base
            },
            Role::CentreBack => Attributes {
                top_speed: 7.6,
                aerial: 0.75,
                tackling: 0.8,
                passing: 0.65,
                finishing: 0.3,
                dribbling: 0.5,
                ..base
            },
            Role::FullBack => Attributes {
                top_speed: 8.4,
                tackling: 0.7,
                ..base
            },
            Role::DefensiveMid => Attributes {
                tackling: 0.75,
                passing: 0.75,
                long_passing: 0.7,
                ..base
            },
            Role::CentralMid => Attributes {
                passing: 0.8,
                long_passing: 0.72,
                control: 0.75,
                ..base
            },
            Role::AttackingMid => Attributes {
                passing: 0.82,
                control: 0.82,
                dribbling: 0.78,
                finishing: 0.65,
                tackling: 0.45,
                ..base
            },
            Role::Winger => Attributes {
                top_speed: 8.8,
                control: 0.8,
                dribbling: 0.8,
                finishing: 0.6,
                tackling: 0.4,
                ..base
            },
            Role::Striker => Attributes {
                top_speed: 8.4,
                control: 0.75,
                finishing: 0.8,
                kicking_power: 0.75,
                tackling: 0.4,
                ..base
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_defaults_are_within_range() {
        for role in Role::ALL {
            let b = role.behaviour();
            assert_eq!(b, b.clamped(), "{role:?} defaults out of range");
        }
    }

    #[test]
    fn defenders_are_more_conservative_than_forwards() {
        assert!(
            Role::CentreBack.behaviour().risk_aversion > Role::Striker.behaviour().risk_aversion
        );
        assert!(
            Role::Goalkeeper.behaviour().risk_aversion > Role::CentreBack.behaviour().risk_aversion
        );
        assert!(
            Role::Striker.behaviour().shot_threshold < Role::CentreBack.behaviour().shot_threshold
        );
    }

    #[test]
    fn forwards_run_in_behind_defenders_do_not() {
        assert!(Role::Striker.behaviour().run_in_behind > 0.5);
        assert_eq!(Role::CentreBack.behaviour().run_in_behind, 0.0);
    }

    #[test]
    fn clamping_bounds_modifiers() {
        let mut b = Role::Winger.behaviour();
        b.dribble_bias = 5.0;
        b.risk_aversion = -1.0;
        let c = b.clamped();
        assert_eq!(c.dribble_bias, 1.0);
        assert_eq!(c.risk_aversion, 0.2);
    }
}

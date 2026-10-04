//! Playing styles, COM playing styles and player skills.
//!
//! These mirror the PES 2017/2018 trait lists. Each trait has a human-readable
//! description and a concrete effect on the decision layer, expressed as
//! additive tweaks to a [`Behaviour`]. Traits stack: a Creative Playmaker with
//! the Mazing Run COM style and Double Touch is a stronger dribbler than any
//! one of those alone.

use crate::role::{Attributes, Behaviour};

/// Primary playing style (one per player in PES).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayingStyle {
    GoalPoacher,
    DummyRunner,
    FoxInTheBox,
    TargetMan,
    CreativePlaymaker,
    ProlificWinger,
    RoamingFlank,
    CrossSpecialist,
    ClassicNo10,
    HolePlayer,
    BoxToBox,
    AnchorMan,
    TheDestroyer,
    ExtraFrontman,
    OffensiveFullBack,
    DefensiveFullBack,
    BuildUp,
    OffensiveGoalkeeper,
    DefensiveGoalkeeper,
}

impl PlayingStyle {
    pub const ALL: [PlayingStyle; 19] = [
        PlayingStyle::GoalPoacher,
        PlayingStyle::DummyRunner,
        PlayingStyle::FoxInTheBox,
        PlayingStyle::TargetMan,
        PlayingStyle::CreativePlaymaker,
        PlayingStyle::ProlificWinger,
        PlayingStyle::RoamingFlank,
        PlayingStyle::CrossSpecialist,
        PlayingStyle::ClassicNo10,
        PlayingStyle::HolePlayer,
        PlayingStyle::BoxToBox,
        PlayingStyle::AnchorMan,
        PlayingStyle::TheDestroyer,
        PlayingStyle::ExtraFrontman,
        PlayingStyle::OffensiveFullBack,
        PlayingStyle::DefensiveFullBack,
        PlayingStyle::BuildUp,
        PlayingStyle::OffensiveGoalkeeper,
        PlayingStyle::DefensiveGoalkeeper,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PlayingStyle::GoalPoacher => "Goal Poacher",
            PlayingStyle::DummyRunner => "Dummy Runner",
            PlayingStyle::FoxInTheBox => "Fox in the Box",
            PlayingStyle::TargetMan => "Target Man",
            PlayingStyle::CreativePlaymaker => "Creative Playmaker",
            PlayingStyle::ProlificWinger => "Prolific Winger",
            PlayingStyle::RoamingFlank => "Roaming Flank",
            PlayingStyle::CrossSpecialist => "Cross Specialist",
            PlayingStyle::ClassicNo10 => "Classic No. 10",
            PlayingStyle::HolePlayer => "Hole Player",
            PlayingStyle::BoxToBox => "Box-to-Box",
            PlayingStyle::AnchorMan => "Anchor Man",
            PlayingStyle::TheDestroyer => "The Destroyer",
            PlayingStyle::ExtraFrontman => "Extra Frontman",
            PlayingStyle::OffensiveFullBack => "Offensive Full-back",
            PlayingStyle::DefensiveFullBack => "Defensive Full-back",
            PlayingStyle::BuildUp => "Build Up",
            PlayingStyle::OffensiveGoalkeeper => "Offensive Goalkeeper",
            PlayingStyle::DefensiveGoalkeeper => "Defensive Goalkeeper",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            PlayingStyle::GoalPoacher => "Lives on the shoulder of the last defender, waiting to spring the offside trap and finish.",
            PlayingStyle::DummyRunner => "Makes decoy runs that drag defenders away and open space for team-mates.",
            PlayingStyle::FoxInTheBox => "Stays in the penalty area, anticipating where the ball will drop to score from close range.",
            PlayingStyle::TargetMan => "A physical reference point who holds the ball up and brings others into play.",
            PlayingStyle::CreativePlaymaker => "Creates chances out of nothing with vision, dribbling and incisive passing.",
            PlayingStyle::ProlificWinger => "A goal-scoring wide player who cuts inside to shoot rather than cross.",
            PlayingStyle::RoamingFlank => "Drifts inside from the flank to combine and create in the half-spaces.",
            PlayingStyle::CrossSpecialist => "Stays wide and looks to deliver crosses into the box at every opportunity.",
            PlayingStyle::ClassicNo10 => "A static playmaker who dictates attacks with passing rather than running, and does little defending.",
            PlayingStyle::HolePlayer => "Bursts forward from midfield into the space behind the striker to score.",
            PlayingStyle::BoxToBox => "Covers every blade of grass, contributing in both penalty areas.",
            PlayingStyle::AnchorMan => "Sits in front of the back line, screening the defence and recycling possession.",
            PlayingStyle::TheDestroyer => "Hunts the ball aggressively and breaks up opposition attacks.",
            PlayingStyle::ExtraFrontman => "A deeper player who pushes up to join the attack as an extra forward.",
            PlayingStyle::OffensiveFullBack => "Bombs forward on the overlap to provide width and crosses.",
            PlayingStyle::DefensiveFullBack => "Prioritises defensive duties and rarely ventures forward.",
            PlayingStyle::BuildUp => "A ball-playing defender who starts attacks from the back.",
            PlayingStyle::OffensiveGoalkeeper => "A sweeper-keeper who comes off the line and starts attacks with his feet.",
            PlayingStyle::DefensiveGoalkeeper => "A line-keeper who stays on his line and keeps distribution safe.",
        }
    }

    /// Apply the style's behavioural tweaks.
    pub fn apply(self, b: &mut Behaviour) {
        match self {
            PlayingStyle::GoalPoacher => {
                b.run_in_behind += 0.3;
                b.stays_high += 0.5;
                b.track_back -= 0.2;
                b.box_presence += 0.3;
                b.shot_threshold -= 0.01;
                b.dribble_bias -= 0.15;
                b.hold_up -= 0.2;
            }
            PlayingStyle::DummyRunner => {
                b.run_in_behind += 0.25;
                b.box_presence += 0.1;
                b.dribble_bias -= 0.2;
            }
            PlayingStyle::FoxInTheBox => {
                b.box_presence += 0.4;
                b.stays_high += 0.3;
                b.shot_range -= 4.0;
                b.run_in_behind -= 0.1;
                b.track_back -= 0.15;
            }
            PlayingStyle::TargetMan => {
                b.hold_up += 0.4;
                b.run_in_behind -= 0.4;
                b.box_presence += 0.2;
                b.dribble_bias -= 0.2;
            }
            PlayingStyle::CreativePlaymaker => {
                b.through_ball_bias += 0.3;
                b.dribble_bias += 0.15;
                b.risk_aversion -= 0.2;
                b.box_presence += 0.1;
            }
            PlayingStyle::ProlificWinger => {
                b.shot_threshold -= 0.02;
                b.box_presence += 0.3;
                b.cross_bias -= 0.25;
                b.dribble_bias += 0.1;
            }
            PlayingStyle::RoamingFlank => {
                b.box_presence += 0.2;
                b.through_ball_bias += 0.1;
                b.cross_bias -= 0.15;
                b.dribble_bias += 0.05;
            }
            PlayingStyle::CrossSpecialist => {
                b.cross_bias += 0.4;
                b.dribble_bias -= 0.1;
                b.box_presence -= 0.2;
            }
            PlayingStyle::ClassicNo10 => {
                b.through_ball_bias += 0.35;
                b.track_back -= 0.3;
                b.press_willingness -= 0.3;
                b.dribble_bias += 0.1;
                b.tempo -= 0.1;
                b.run_in_behind -= 0.2;
            }
            PlayingStyle::HolePlayer => {
                b.run_in_behind += 0.3;
                b.box_presence += 0.4;
                b.shot_threshold -= 0.01;
            }
            PlayingStyle::BoxToBox => {
                b.press_willingness += 0.15;
                b.track_back += 0.2;
                b.box_presence += 0.3;
                b.possession_push += 4.0;
            }
            PlayingStyle::AnchorMan => {
                b.rest_defence = true;
                b.possession_push -= 4.0;
                b.track_back += 0.2;
                b.run_in_behind = 0.0;
                b.box_presence = 0.0;
                b.risk_aversion += 0.2;
            }
            PlayingStyle::TheDestroyer => {
                b.press_willingness += 0.25;
                b.marking_tightness += 0.3;
                b.track_back += 0.2;
            }
            PlayingStyle::ExtraFrontman => {
                b.possession_push += 8.0;
                b.box_presence += 0.3;
                b.run_in_behind += 0.2;
            }
            PlayingStyle::OffensiveFullBack => {
                b.overlap += 0.2;
                b.possession_push += 6.0;
                b.cross_bias += 0.1;
                b.track_back -= 0.1;
            }
            PlayingStyle::DefensiveFullBack => {
                b.overlap -= 0.5;
                b.possession_push -= 8.0;
                b.track_back += 0.1;
                b.marking_tightness += 0.2;
            }
            PlayingStyle::BuildUp => {
                b.risk_aversion -= 0.3;
                b.long_pass_bias += 0.1;
                b.dribble_bias += 0.15;
            }
            PlayingStyle::OffensiveGoalkeeper => {
                b.possession_push += 6.0;
                b.risk_aversion -= 0.5;
            }
            PlayingStyle::DefensiveGoalkeeper => {
                b.possession_push -= 3.0;
                b.long_pass_bias += 0.2;
            }
        }
    }
}

/// COM playing styles: how the AI uses the player on the ball.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ComStyle {
    Trickster,
    MazingRun,
    SpeedingBullet,
    IncisiveRun,
    LongBallExpert,
    EarlyCross,
    LongRanger,
}

impl ComStyle {
    pub const ALL: [ComStyle; 7] = [
        ComStyle::Trickster,
        ComStyle::MazingRun,
        ComStyle::SpeedingBullet,
        ComStyle::IncisiveRun,
        ComStyle::LongBallExpert,
        ComStyle::EarlyCross,
        ComStyle::LongRanger,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ComStyle::Trickster => "Trickster",
            ComStyle::MazingRun => "Mazing Run",
            ComStyle::SpeedingBullet => "Speeding Bullet",
            ComStyle::IncisiveRun => "Incisive Run",
            ComStyle::LongBallExpert => "Long Ball Expert",
            ComStyle::EarlyCross => "Early Cross",
            ComStyle::LongRanger => "Long Ranger",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ComStyle::Trickster => "Uses tricks and feints to beat defenders one-on-one.",
            ComStyle::MazingRun => "Weaves through tight spaces on long, slaloming dribbles.",
            ComStyle::SpeedingBullet => "Carries the ball at full speed into space.",
            ComStyle::IncisiveRun => "Drives with the ball into the penalty area.",
            ComStyle::LongBallExpert => "Frequently plays long passes to switch play or find runners.",
            ComStyle::EarlyCross => "Crosses early from deep before the defence is set.",
            ComStyle::LongRanger => "Shoots from distance whenever he gets the chance.",
        }
    }

    pub fn apply(self, b: &mut Behaviour) {
        match self {
            ComStyle::Trickster => b.dribble_bias += 0.2,
            ComStyle::MazingRun => {
                b.dribble_bias += 0.3;
                b.risk_aversion -= 0.1;
            }
            ComStyle::SpeedingBullet => {
                b.dribble_bias += 0.15;
                b.run_in_behind += 0.1;
            }
            ComStyle::IncisiveRun => {
                b.dribble_bias += 0.15;
                b.box_presence += 0.15;
            }
            ComStyle::LongBallExpert => b.long_pass_bias += 0.4,
            ComStyle::EarlyCross => b.cross_bias += 0.25,
            ComStyle::LongRanger => {
                b.shot_range += 8.0;
                b.shot_threshold -= 0.02;
            }
        }
    }
}

/// Player skills (special abilities).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Skill {
    ScissorsFeint,
    DoubleTouch,
    FlipFlap,
    MarseilleTurn,
    Sombrero,
    CutBehindAndTurn,
    ScotchMove,
    HeelTrick,
    Heading,
    LongRangeCurler,
    ChipShotControl,
    KnuckleShot,
    DippingShot,
    RisingShot,
    AcrobaticFinishing,
    FirstTimeShot,
    OneTouchPass,
    ThroughPassing,
    WeightedPass,
    PinpointCrossing,
    OutsideCurler,
    Rabona,
    NoLookPass,
    LowLoftedPass,
    GkLowPunt,
    GkHighPunt,
    LongThrow,
    GkLongThrow,
    PenaltySpecialist,
    GkPenaltySaver,
    Malicia,
    ManMarking,
    TrackBack,
    Interception,
    AcrobaticClear,
    Captaincy,
    SuperSub,
    FightingSpirit,
}

impl Skill {
    pub const ALL: [Skill; 38] = [
        Skill::ScissorsFeint,
        Skill::DoubleTouch,
        Skill::FlipFlap,
        Skill::MarseilleTurn,
        Skill::Sombrero,
        Skill::CutBehindAndTurn,
        Skill::ScotchMove,
        Skill::HeelTrick,
        Skill::Heading,
        Skill::LongRangeCurler,
        Skill::ChipShotControl,
        Skill::KnuckleShot,
        Skill::DippingShot,
        Skill::RisingShot,
        Skill::AcrobaticFinishing,
        Skill::FirstTimeShot,
        Skill::OneTouchPass,
        Skill::ThroughPassing,
        Skill::WeightedPass,
        Skill::PinpointCrossing,
        Skill::OutsideCurler,
        Skill::Rabona,
        Skill::NoLookPass,
        Skill::LowLoftedPass,
        Skill::GkLowPunt,
        Skill::GkHighPunt,
        Skill::LongThrow,
        Skill::GkLongThrow,
        Skill::PenaltySpecialist,
        Skill::GkPenaltySaver,
        Skill::Malicia,
        Skill::ManMarking,
        Skill::TrackBack,
        Skill::Interception,
        Skill::AcrobaticClear,
        Skill::Captaincy,
        Skill::SuperSub,
        Skill::FightingSpirit,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Skill::ScissorsFeint => "Scissors Feint",
            Skill::DoubleTouch => "Double Touch",
            Skill::FlipFlap => "Flip Flap",
            Skill::MarseilleTurn => "Marseille Turn",
            Skill::Sombrero => "Sombrero",
            Skill::CutBehindAndTurn => "Cut Behind & Turn",
            Skill::ScotchMove => "Scotch Move",
            Skill::HeelTrick => "Heel Trick",
            Skill::Heading => "Heading",
            Skill::LongRangeCurler => "Long Range Curler",
            Skill::ChipShotControl => "Chip Shot Control",
            Skill::KnuckleShot => "Knuckle Shot",
            Skill::DippingShot => "Dipping Shot",
            Skill::RisingShot => "Rising Shot",
            Skill::AcrobaticFinishing => "Acrobatic Finishing",
            Skill::FirstTimeShot => "First-time Shot",
            Skill::OneTouchPass => "One-touch Pass",
            Skill::ThroughPassing => "Through Passing",
            Skill::WeightedPass => "Weighted Pass",
            Skill::PinpointCrossing => "Pinpoint Crossing",
            Skill::OutsideCurler => "Outside Curler",
            Skill::Rabona => "Rabona",
            Skill::NoLookPass => "No Look Pass",
            Skill::LowLoftedPass => "Low Lofted Pass",
            Skill::GkLowPunt => "GK Low Punt",
            Skill::GkHighPunt => "GK High Punt",
            Skill::LongThrow => "Long Throw",
            Skill::GkLongThrow => "GK Long Throw",
            Skill::PenaltySpecialist => "Penalty Specialist",
            Skill::GkPenaltySaver => "GK Penalty Saver",
            Skill::Malicia => "Malicia",
            Skill::ManMarking => "Man Marking",
            Skill::TrackBack => "Track Back",
            Skill::Interception => "Interception",
            Skill::AcrobaticClear => "Acrobatic Clear",
            Skill::Captaincy => "Captaincy",
            Skill::SuperSub => "Super-sub",
            Skill::FightingSpirit => "Fighting Spirit",
        }
    }

    pub fn is_dribbling_trick(self) -> bool {
        matches!(
            self,
            Skill::ScissorsFeint
                | Skill::DoubleTouch
                | Skill::FlipFlap
                | Skill::MarseilleTurn
                | Skill::Sombrero
                | Skill::CutBehindAndTurn
                | Skill::ScotchMove
                | Skill::HeelTrick
        )
    }

    pub fn is_shooting(self) -> bool {
        matches!(
            self,
            Skill::LongRangeCurler
                | Skill::ChipShotControl
                | Skill::KnuckleShot
                | Skill::DippingShot
                | Skill::RisingShot
                | Skill::AcrobaticFinishing
                | Skill::FirstTimeShot
        )
    }

    /// Behavioural effect of the skill.
    pub fn apply(self, b: &mut Behaviour) {
        match self {
            s if s.is_dribbling_trick() => b.dribble_bias += 0.04,
            Skill::LongRangeCurler | Skill::KnuckleShot | Skill::DippingShot | Skill::RisingShot => b.shot_range += 2.5,
            Skill::AcrobaticFinishing | Skill::FirstTimeShot | Skill::ChipShotControl => b.shot_threshold -= 0.008,
            Skill::Heading => b.box_presence += 0.05,
            Skill::OneTouchPass => b.tempo += 0.2,
            Skill::ThroughPassing => b.through_ball_bias += 0.2,
            Skill::WeightedPass => b.through_ball_bias += 0.1,
            Skill::PinpointCrossing => b.cross_bias += 0.1,
            Skill::LowLoftedPass => b.long_pass_bias += 0.05,
            Skill::ManMarking => b.marking_tightness += 0.25,
            Skill::TrackBack => b.track_back += 0.3,
            Skill::Interception => b.press_willingness += 0.05,
            _ => {}
        }
    }

    /// Small technical bonus the skill grants in the simulation.
    pub fn apply_attributes(self, a: &mut Attributes) {
        match self {
            s if s.is_dribbling_trick() => a.dribbling = (a.dribbling + 0.012).min(1.0),
            Skill::AcrobaticFinishing | Skill::FirstTimeShot => a.finishing = (a.finishing + 0.02).min(1.0),
            Skill::OneTouchPass | Skill::ThroughPassing | Skill::WeightedPass => a.passing = (a.passing + 0.015).min(1.0),
            Skill::LowLoftedPass => a.long_passing = (a.long_passing + 0.02).min(1.0),
            Skill::Interception | Skill::ManMarking => a.tackling = (a.tackling + 0.015).min(1.0),
            _ => {}
        }
    }
}

/// Compact bit set of [`Skill`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct SkillSet(u64);

impl SkillSet {
    pub fn new(skills: &[Skill]) -> Self {
        let mut s = SkillSet(0);
        for &k in skills {
            s.insert(k);
        }
        s
    }
    pub fn insert(&mut self, s: Skill) {
        self.0 |= 1 << (s as u8);
    }
    pub fn contains(&self, s: Skill) -> bool {
        self.0 & (1 << (s as u8)) != 0
    }
    pub fn len(&self) -> usize {
        self.0.count_ones() as usize
    }
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
    /// Skills in declaration order (deterministic).
    pub fn iter(&self) -> impl Iterator<Item = Skill> + '_ {
        Skill::ALL.iter().copied().filter(move |&s| self.contains(s))
    }
}

/// Compact bit set of [`ComStyle`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct ComStyleSet(u8);

impl ComStyleSet {
    pub fn new(styles: &[ComStyle]) -> Self {
        let mut s = ComStyleSet(0);
        for &k in styles {
            s.insert(k);
        }
        s
    }
    pub fn insert(&mut self, s: ComStyle) {
        self.0 |= 1 << (s as u8);
    }
    pub fn contains(&self, s: ComStyle) -> bool {
        self.0 & (1 << (s as u8)) != 0
    }
    pub fn iter(&self) -> impl Iterator<Item = ComStyle> + '_ {
        ComStyle::ALL.iter().copied().filter(move |&s| self.contains(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::role::Role;

    #[test]
    fn skill_set_round_trip() {
        let s = SkillSet::new(&[Skill::DoubleTouch, Skill::FightingSpirit, Skill::ScissorsFeint]);
        assert_eq!(s.len(), 3);
        assert!(s.contains(Skill::DoubleTouch));
        assert!(!s.contains(Skill::Rabona));
        let v: Vec<Skill> = s.iter().collect();
        assert_eq!(v, vec![Skill::ScissorsFeint, Skill::DoubleTouch, Skill::FightingSpirit]);
    }

    #[test]
    fn all_skills_fit_in_bitset() {
        assert!(Skill::ALL.len() <= 64);
        for (i, s) in Skill::ALL.iter().enumerate() {
            assert_eq!(*s as usize, i, "Skill::ALL must be in declaration order");
        }
        for (i, s) in ComStyle::ALL.iter().enumerate() {
            assert_eq!(*s as usize, i);
        }
    }

    #[test]
    fn every_trait_has_text() {
        for s in PlayingStyle::ALL {
            assert!(!s.name().is_empty() && s.description().len() > 20);
        }
        for s in ComStyle::ALL {
            assert!(!s.name().is_empty() && s.description().len() > 20);
        }
        for s in Skill::ALL {
            assert!(!s.name().is_empty());
        }
    }

    #[test]
    fn poacher_stays_higher_than_target_man_runs() {
        let mut poacher = Role::Striker.behaviour();
        PlayingStyle::GoalPoacher.apply(&mut poacher);
        let mut target = Role::Striker.behaviour();
        PlayingStyle::TargetMan.apply(&mut target);
        assert!(poacher.clamped().run_in_behind > target.clamped().run_in_behind);
        assert!(target.clamped().hold_up > poacher.clamped().hold_up);
    }
}

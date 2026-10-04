//! Ready-made team styles, player archetypes and example profiles.
//!
//! These exist so the simulation, tests and sandbox have recognisable
//! identities to work with before real data is imported. The intended data
//! path for the full game is to fill [`TeamStyle`] / [`PlayerProfile`]
//! directly from the extracted PES 2018 database, which already contains the
//! same fields (ratings, playing style, COM styles, skills, game plan).

use super::profile::PlayerProfile;
use super::ratings::{Foot, Ratings};
use super::styles::{ComStyle, ComStyleSet, PlayingStyle, Skill, SkillSet};
use super::team::*;
use crate::formation::FormationKind;
use crate::role::Role;

// ---------------------------------------------------------------- teams ----

/// Patient short passing, fluid positions, high aggressive press.
pub fn tiki_taka() -> TeamStyle {
    TeamStyle {
        name: "Tiki-Taka".into(),
        formation: FormationKind::F433,
        attacking_style: AttackingStyle::PossessionGame,
        build_up: BuildUp::ShortPass,
        attacking_area: AttackingArea::Centre,
        positioning: Positioning::Flexible,
        support_range: 2,
        defensive_style: DefensiveStyle::FrontlinePressure,
        containment_area: ContainmentArea::Middle,
        pressuring: Pressuring::Aggressive,
        defensive_line: 8,
        compactness: 7,
        instructions: vec![
            Instruction::TikiTaka,
            Instruction::FalseNine,
            Instruction::Gegenpress,
        ],
    }
}

/// Win it back immediately, attack vertically and fast.
pub fn gegenpress() -> TeamStyle {
    TeamStyle {
        name: "Gegenpress".into(),
        formation: FormationKind::F4231,
        attacking_style: AttackingStyle::CounterAttack,
        build_up: BuildUp::ShortPass,
        attacking_area: AttackingArea::Centre,
        positioning: Positioning::Flexible,
        support_range: 4,
        defensive_style: DefensiveStyle::FrontlinePressure,
        containment_area: ContainmentArea::Middle,
        pressuring: Pressuring::Aggressive,
        defensive_line: 9,
        compactness: 9,
        instructions: vec![
            Instruction::Gegenpress,
            Instruction::TightMarking,
            Instruction::AttackingFullBacks,
        ],
    }
}

/// Bypass midfield with long balls to a big forward.
pub fn long_ball() -> TeamStyle {
    TeamStyle {
        name: "Route One".into(),
        formation: FormationKind::F442,
        attacking_style: AttackingStyle::CounterAttack,
        build_up: BuildUp::LongPass,
        attacking_area: AttackingArea::Wide,
        positioning: Positioning::MaintainFormation,
        support_range: 8,
        defensive_style: DefensiveStyle::AllOutDefence,
        containment_area: ContainmentArea::Middle,
        pressuring: Pressuring::Conservative,
        defensive_line: 4,
        compactness: 6,
        instructions: vec![Instruction::CentringTargets, Instruction::FlankProtection],
    }
}

/// Sit deep and break with pace.
pub fn counter_attack() -> TeamStyle {
    TeamStyle {
        name: "Counter-Attack".into(),
        formation: FormationKind::F4141,
        attacking_style: AttackingStyle::CounterAttack,
        build_up: BuildUp::ShortPass,
        attacking_area: AttackingArea::Wide,
        positioning: Positioning::MaintainFormation,
        support_range: 7,
        defensive_style: DefensiveStyle::AllOutDefence,
        containment_area: ContainmentArea::Middle,
        pressuring: Pressuring::Aggressive,
        defensive_line: 3,
        compactness: 8,
        instructions: vec![Instruction::CounterTarget, Instruction::Anchoring],
    }
}

/// Stretch the pitch and cross.
pub fn wing_play() -> TeamStyle {
    TeamStyle {
        name: "Wing Play".into(),
        formation: FormationKind::F442,
        attacking_style: AttackingStyle::PossessionGame,
        build_up: BuildUp::ShortPass,
        attacking_area: AttackingArea::Wide,
        positioning: Positioning::MaintainFormation,
        support_range: 6,
        defensive_style: DefensiveStyle::FrontlinePressure,
        containment_area: ContainmentArea::Wide,
        pressuring: Pressuring::Conservative,
        defensive_line: 5,
        compactness: 5,
        instructions: vec![Instruction::HugTheTouchline, Instruction::CentringTargets],
    }
}

/// Defend the box with everyone.
pub fn park_the_bus() -> TeamStyle {
    TeamStyle {
        name: "Park the Bus".into(),
        formation: FormationKind::F532,
        attacking_style: AttackingStyle::CounterAttack,
        build_up: BuildUp::LongPass,
        attacking_area: AttackingArea::Centre,
        positioning: Positioning::MaintainFormation,
        support_range: 9,
        defensive_style: DefensiveStyle::AllOutDefence,
        containment_area: ContainmentArea::Middle,
        pressuring: Pressuring::Conservative,
        defensive_line: 1,
        compactness: 10,
        instructions: vec![Instruction::SwarmTheBox, Instruction::CounterTarget],
    }
}

/// All team archetypes.
pub fn team_archetypes() -> Vec<TeamStyle> {
    vec![
        tiki_taka(),
        gegenpress(),
        long_ball(),
        counter_attack(),
        wing_play(),
        park_the_bus(),
    ]
}

// -------------------------------------------------------------- players ----

/// Player archetypes used to populate generated squads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerArchetype {
    SweeperKeeper,
    LineKeeper,
    BallPlayingDefender,
    Stopper,
    AttackingFullBack,
    DefensiveFullBack,
    Anchor,
    Destroyer,
    DeepPlaymaker,
    BoxToBox,
    Playmaker,
    ClassicNo10,
    ShadowStriker,
    DribblingWinger,
    CrossingWinger,
    InsideForward,
    Poacher,
    TargetMan,
    CompleteForward,
    FoxInTheBox,
}

impl PlayerArchetype {
    pub fn position(self) -> Role {
        use PlayerArchetype::*;
        match self {
            SweeperKeeper | LineKeeper => Role::Goalkeeper,
            BallPlayingDefender | Stopper => Role::CentreBack,
            AttackingFullBack | DefensiveFullBack => Role::FullBack,
            Anchor | Destroyer | DeepPlaymaker => Role::DefensiveMid,
            BoxToBox | Playmaker => Role::CentralMid,
            ClassicNo10 | ShadowStriker => Role::AttackingMid,
            DribblingWinger | CrossingWinger | InsideForward => Role::Winger,
            Poacher | TargetMan | CompleteForward | FoxInTheBox => Role::Striker,
        }
    }

    pub fn name(self) -> &'static str {
        use PlayerArchetype::*;
        match self {
            SweeperKeeper => "Sweeper Keeper",
            LineKeeper => "Line Keeper",
            BallPlayingDefender => "Ball-Playing Defender",
            Stopper => "Stopper",
            AttackingFullBack => "Attacking Full-back",
            DefensiveFullBack => "Defensive Full-back",
            Anchor => "Anchor",
            Destroyer => "Destroyer",
            DeepPlaymaker => "Deep-Lying Playmaker",
            BoxToBox => "Box-to-Box",
            Playmaker => "Playmaker",
            ClassicNo10 => "Classic No. 10",
            ShadowStriker => "Shadow Striker",
            DribblingWinger => "Dribbling Winger",
            CrossingWinger => "Crossing Winger",
            InsideForward => "Inside Forward",
            Poacher => "Poacher",
            TargetMan => "Target Man",
            CompleteForward => "Complete Forward",
            FoxInTheBox => "Fox in the Box",
        }
    }
}

fn base_ratings(role: Role, q: u8) -> Ratings {
    let mut r = Ratings::uniform(q);
    let lo = q.saturating_sub(18).max(40);
    match role {
        Role::Goalkeeper => {
            r = Ratings::uniform(lo);
            r.goalkeeping = q;
            r.catching = q;
            r.clearing = q.saturating_sub(3);
            r.reflexes = q;
            r.coverage = q.saturating_sub(2);
            r.low_pass = q.saturating_sub(15);
            r.lofted_pass = q.saturating_sub(12);
            r.height_cm = 190;
        }
        Role::CentreBack => {
            r.finishing = lo;
            r.dribbling = q.saturating_sub(10);
            r.offensive_awareness = lo;
            r.header = q + 3;
            r.physical_contact = q + 3;
            r.defensive_awareness = q + 4;
            r.ball_winning = q + 4;
            r.speed = q.saturating_sub(6);
            r.height_cm = 188;
        }
        Role::FullBack => {
            r.finishing = q.saturating_sub(12);
            r.speed = q + 4;
            r.stamina = q + 4;
        }
        Role::DefensiveMid => {
            r.finishing = q.saturating_sub(12);
            r.ball_winning = q + 3;
            r.defensive_awareness = q + 3;
        }
        Role::CentralMid => {
            r.low_pass = q + 3;
            r.stamina = q + 3;
        }
        Role::AttackingMid => {
            r.defensive_awareness = lo;
            r.ball_winning = q.saturating_sub(15);
            r.low_pass = q + 3;
            r.ball_control = q + 3;
        }
        Role::Winger => {
            r.defensive_awareness = q.saturating_sub(18);
            r.ball_winning = q.saturating_sub(18);
            r.speed = q + 5;
            r.dribbling = q + 3;
        }
        Role::Striker => {
            r.defensive_awareness = lo;
            r.ball_winning = q.saturating_sub(18);
            r.finishing = q + 4;
            r.offensive_awareness = q + 4;
        }
    }
    r
}

fn bump(v: &mut u8, by: i16) {
    *v = (*v as i16 + by).clamp(40, 99) as u8;
}

/// Build a profile for an archetype at quality `q` (40–99).
pub fn archetype_profile(arch: PlayerArchetype, name: impl Into<String>, q: u8) -> PlayerProfile {
    use PlayerArchetype::*;
    let position = arch.position();
    let mut r = base_ratings(position, q);
    let (style, com, skills): (Option<PlayingStyle>, Vec<ComStyle>, Vec<Skill>) = match arch {
        SweeperKeeper => {
            bump(&mut r.low_pass, 12);
            bump(&mut r.coverage, 4);
            (
                Some(PlayingStyle::OffensiveGoalkeeper),
                vec![],
                vec![Skill::GkLowPunt],
            )
        }
        LineKeeper => {
            bump(&mut r.reflexes, 4);
            (
                Some(PlayingStyle::DefensiveGoalkeeper),
                vec![],
                vec![Skill::GkHighPunt, Skill::GkLongThrow],
            )
        }
        BallPlayingDefender => {
            bump(&mut r.low_pass, 10);
            bump(&mut r.lofted_pass, 10);
            (
                Some(PlayingStyle::BuildUp),
                vec![ComStyle::LongBallExpert],
                vec![Skill::LowLoftedPass],
            )
        }
        Stopper => {
            bump(&mut r.physical_contact, 6);
            bump(&mut r.ball_winning, 4);
            (
                Some(PlayingStyle::TheDestroyer),
                vec![],
                vec![Skill::Heading, Skill::ManMarking, Skill::AcrobaticClear],
            )
        }
        AttackingFullBack => {
            bump(&mut r.speed, 4);
            bump(&mut r.lofted_pass, 4);
            (
                Some(PlayingStyle::OffensiveFullBack),
                vec![ComStyle::EarlyCross],
                vec![Skill::PinpointCrossing],
            )
        }
        DefensiveFullBack => {
            bump(&mut r.defensive_awareness, 6);
            (
                Some(PlayingStyle::DefensiveFullBack),
                vec![],
                vec![Skill::ManMarking],
            )
        }
        Anchor => {
            bump(&mut r.defensive_awareness, 6);
            (
                Some(PlayingStyle::AnchorMan),
                vec![],
                vec![Skill::Interception],
            )
        }
        Destroyer => {
            bump(&mut r.ball_winning, 6);
            bump(&mut r.stamina, 5);
            (
                Some(PlayingStyle::TheDestroyer),
                vec![],
                vec![Skill::Interception, Skill::FightingSpirit],
            )
        }
        DeepPlaymaker => {
            bump(&mut r.lofted_pass, 10);
            bump(&mut r.low_pass, 8);
            (
                Some(PlayingStyle::BuildUp),
                vec![ComStyle::LongBallExpert],
                vec![Skill::WeightedPass, Skill::OneTouchPass],
            )
        }
        BoxToBox => {
            bump(&mut r.stamina, 8);
            (
                Some(PlayingStyle::BoxToBox),
                vec![ComStyle::LongRanger],
                vec![Skill::TrackBack],
            )
        }
        Playmaker => {
            bump(&mut r.low_pass, 8);
            bump(&mut r.ball_control, 5);
            (
                Some(PlayingStyle::CreativePlaymaker),
                vec![],
                vec![Skill::ThroughPassing, Skill::OneTouchPass],
            )
        }
        ClassicNo10 => {
            bump(&mut r.low_pass, 8);
            bump(&mut r.speed, -8);
            (
                Some(PlayingStyle::ClassicNo10),
                vec![],
                vec![Skill::ThroughPassing, Skill::NoLookPass],
            )
        }
        ShadowStriker => {
            bump(&mut r.finishing, 6);
            bump(&mut r.offensive_awareness, 6);
            (
                Some(PlayingStyle::HolePlayer),
                vec![ComStyle::IncisiveRun],
                vec![Skill::FirstTimeShot],
            )
        }
        DribblingWinger => {
            bump(&mut r.dribbling, 8);
            bump(&mut r.body_control, 6);
            (
                Some(PlayingStyle::RoamingFlank),
                vec![ComStyle::Trickster, ComStyle::MazingRun],
                vec![Skill::DoubleTouch, Skill::ScissorsFeint, Skill::FlipFlap],
            )
        }
        CrossingWinger => {
            bump(&mut r.lofted_pass, 8);
            bump(&mut r.curl, 8);
            (
                Some(PlayingStyle::CrossSpecialist),
                vec![ComStyle::EarlyCross],
                vec![Skill::PinpointCrossing],
            )
        }
        InsideForward => {
            bump(&mut r.finishing, 6);
            bump(&mut r.dribbling, 4);
            (
                Some(PlayingStyle::ProlificWinger),
                vec![ComStyle::IncisiveRun],
                vec![Skill::LongRangeCurler],
            )
        }
        Poacher => {
            bump(&mut r.finishing, 6);
            bump(&mut r.offensive_awareness, 6);
            (
                Some(PlayingStyle::GoalPoacher),
                vec![],
                vec![Skill::FirstTimeShot],
            )
        }
        TargetMan => {
            bump(&mut r.physical_contact, 10);
            bump(&mut r.header, 10);
            bump(&mut r.speed, -8);
            (Some(PlayingStyle::TargetMan), vec![], vec![Skill::Heading])
        }
        CompleteForward => {
            bump(&mut r.low_pass, 4);
            (
                Some(PlayingStyle::DummyRunner),
                vec![],
                vec![Skill::OneTouchPass, Skill::FirstTimeShot],
            )
        }
        FoxInTheBox => {
            bump(&mut r.finishing, 8);
            bump(&mut r.speed, -4);
            (
                Some(PlayingStyle::FoxInTheBox),
                vec![],
                vec![Skill::AcrobaticFinishing, Skill::FirstTimeShot],
            )
        }
    };
    PlayerProfile {
        name: name.into(),
        position,
        ratings: r,
        playing_style: style,
        com_styles: ComStyleSet::new(&com),
        skills: SkillSet::new(&skills),
    }
}

// ------------------------------------------------------------ examples ----

/// Example of an elite dribbling playmaker in the mould of Lionel Messi.
///
/// Ratings are illustrative approximations for testing and tuning only;
/// real values should come from the imported game database.
pub fn example_messi() -> PlayerProfile {
    let r = Ratings {
        offensive_awareness: 94,
        ball_control: 97,
        dribbling: 97,
        low_pass: 91,
        lofted_pass: 85,
        finishing: 94,
        place_kicking: 90,
        curl: 93,
        header: 70,
        defensive_awareness: 48,
        ball_winning: 50,
        kicking_power: 78,
        speed: 88,
        explosive_power: 94,
        body_control: 95,
        physical_contact: 66,
        jump: 68,
        goalkeeping: 40,
        catching: 40,
        clearing: 40,
        reflexes: 40,
        coverage: 40,
        stamina: 78,
        weak_foot_usage: 3,
        weak_foot_accuracy: 3,
        form: 8,
        injury_resistance: 3,
        height_cm: 170,
        stronger_foot: Foot::Left,
    };
    PlayerProfile {
        name: "L. Messi".into(),
        position: Role::Winger,
        ratings: r,
        playing_style: Some(PlayingStyle::CreativePlaymaker),
        com_styles: ComStyleSet::new(&[
            ComStyle::MazingRun,
            ComStyle::Trickster,
            ComStyle::LongRanger,
        ]),
        skills: SkillSet::new(&[
            Skill::DoubleTouch,
            Skill::ScissorsFeint,
            Skill::LongRangeCurler,
            Skill::ChipShotControl,
            Skill::ThroughPassing,
            Skill::WeightedPass,
            Skill::OneTouchPass,
            Skill::FirstTimeShot,
            Skill::Captaincy,
        ]),
    }
}

/// Example of an elite penalty-box finisher in the mould of Cristiano Ronaldo.
///
/// Ratings are illustrative approximations for testing and tuning only;
/// real values should come from the imported game database.
pub fn example_ronaldo() -> PlayerProfile {
    let r = Ratings {
        offensive_awareness: 97,
        ball_control: 91,
        dribbling: 88,
        low_pass: 80,
        lofted_pass: 76,
        finishing: 97,
        place_kicking: 86,
        curl: 82,
        header: 92,
        defensive_awareness: 45,
        ball_winning: 46,
        kicking_power: 95,
        speed: 90,
        explosive_power: 89,
        body_control: 84,
        physical_contact: 84,
        jump: 95,
        goalkeeping: 40,
        catching: 40,
        clearing: 40,
        reflexes: 40,
        coverage: 40,
        stamina: 82,
        weak_foot_usage: 3,
        weak_foot_accuracy: 3,
        form: 8,
        injury_resistance: 3,
        height_cm: 187,
        stronger_foot: Foot::Right,
    };
    PlayerProfile {
        name: "C. Ronaldo".into(),
        position: Role::Striker,
        ratings: r,
        playing_style: Some(PlayingStyle::GoalPoacher),
        com_styles: ComStyleSet::new(&[ComStyle::LongRanger, ComStyle::SpeedingBullet]),
        skills: SkillSet::new(&[
            Skill::Heading,
            Skill::KnuckleShot,
            Skill::AcrobaticFinishing,
            Skill::FirstTimeShot,
            Skill::RisingShot,
            Skill::ScissorsFeint,
            Skill::PenaltySpecialist,
            Skill::FightingSpirit,
        ]),
    }
}

// -------------------------------------------------------------- squads ----

/// A team identity plus eleven players in formation-slot order.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Squad {
    pub style: TeamStyle,
    /// The starting eleven, in formation-slot order.
    pub players: Vec<PlayerProfile>,
    /// Substitutes.
    pub bench: Vec<PlayerProfile>,
}

impl Squad {
    /// Generate a squad whose players suit the style: possession teams get
    /// playmakers, long-ball teams get a target man, and so on.
    pub fn generated(style: TeamStyle, quality: u8) -> Squad {
        let formation = crate::formation::Formation::new(style.formation);
        let possession = style.attacking_style == AttackingStyle::PossessionGame;
        let long = style.build_up == BuildUp::LongPass;
        let wide = style.attacking_area == AttackingArea::Wide;
        let pressing = style.pressuring == Pressuring::Aggressive;

        let mut counts = [0usize; 8];
        let players = formation
            .slots
            .iter()
            .map(|slot| {
                let k = counts[slot.role as usize];
                counts[slot.role as usize] += 1;
                use PlayerArchetype::*;
                let arch = match slot.role {
                    Role::Goalkeeper => {
                        if style.defensive_line >= 7 {
                            SweeperKeeper
                        } else {
                            LineKeeper
                        }
                    }
                    Role::CentreBack => {
                        if possession && k == 0 {
                            BallPlayingDefender
                        } else {
                            Stopper
                        }
                    }
                    Role::FullBack => {
                        if wide || style.has(Instruction::AttackingFullBacks) {
                            AttackingFullBack
                        } else {
                            DefensiveFullBack
                        }
                    }
                    Role::DefensiveMid => {
                        if possession || long {
                            if k == 0 {
                                DeepPlaymaker
                            } else {
                                Anchor
                            }
                        } else if pressing {
                            Destroyer
                        } else {
                            Anchor
                        }
                    }
                    Role::CentralMid => {
                        if possession && k == 0 {
                            Playmaker
                        } else {
                            BoxToBox
                        }
                    }
                    Role::AttackingMid => {
                        if possession {
                            ClassicNo10
                        } else {
                            ShadowStriker
                        }
                    }
                    Role::Winger => {
                        if wide {
                            CrossingWinger
                        } else if k == 0 {
                            DribblingWinger
                        } else {
                            InsideForward
                        }
                    }
                    Role::Striker => {
                        if long && k == 0 {
                            TargetMan
                        } else if style.has(Instruction::FalseNine) {
                            CompleteForward
                        } else if k == 0 {
                            Poacher
                        } else {
                            FoxInTheBox
                        }
                    }
                };
                archetype_profile(arch, format!("{} {}", arch.name(), k + 1), quality)
            })
            .collect();
        // A balanced bench, slightly weaker than the starters.
        let bq = quality.saturating_sub(3);
        let bench = [
            PlayerArchetype::LineKeeper,
            PlayerArchetype::Stopper,
            PlayerArchetype::DefensiveFullBack,
            PlayerArchetype::BoxToBox,
            PlayerArchetype::Anchor,
            PlayerArchetype::DribblingWinger,
            PlayerArchetype::Poacher,
        ]
        .iter()
        .map(|&a| archetype_profile(a, format!("Sub {}", a.name()), bq))
        .collect();
        Squad {
            style,
            players,
            bench,
        }
    }

    /// Replace the player in `slot` with `profile`.
    pub fn with_player(mut self, slot: usize, profile: PlayerProfile) -> Squad {
        self.players[slot] = profile;
        self
    }

    /// Team description followed by one line per player.
    pub fn describe(&self) -> String {
        let mut s = self.style.describe();
        for p in &self.players {
            s.push_str("\n  - ");
            s.push_str(&p.describe());
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_squads_match_formation_roles() {
        for style in team_archetypes() {
            let f = crate::formation::Formation::new(style.formation);
            let squad = Squad::generated(style, 78);
            assert_eq!(squad.players.len(), 11);
            for (slot, p) in f.slots.iter().zip(&squad.players) {
                assert_eq!(slot.role, p.position, "{}", squad.style.name);
            }
        }
    }

    #[test]
    fn long_ball_squad_has_target_man() {
        let squad = Squad::generated(long_ball(), 75);
        assert!(squad
            .players
            .iter()
            .any(|p| p.playing_style == Some(PlayingStyle::TargetMan)));
    }

    #[test]
    fn possession_squad_has_playmaker() {
        let squad = Squad::generated(tiki_taka(), 80);
        assert!(squad
            .players
            .iter()
            .any(|p| p.playing_style == Some(PlayingStyle::CreativePlaymaker)));
    }

    #[test]
    fn archetype_positions_are_consistent() {
        use PlayerArchetype::*;
        for a in [
            SweeperKeeper,
            Stopper,
            Anchor,
            Playmaker,
            DribblingWinger,
            Poacher,
            TargetMan,
        ] {
            assert_eq!(archetype_profile(a, "x", 75).position, a.position());
        }
    }
}

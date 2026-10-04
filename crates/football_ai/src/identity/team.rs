//! Team identity: PES-style team tactics and advanced instructions.
//!
//! [`TeamStyle`] mirrors the "Game Plan" screen of PES 2017/2018 (attacking
//! style, build up, attacking area, positioning, support range, defensive
//! style, containment area, pressuring, defensive line, compactness, plus up
//! to two attacking and two defensive advanced instructions). It compiles into
//! the numeric [`Tactics`] the decision layer consumes, and can describe
//! itself in plain language.

use crate::formation::FormationKind;
use crate::tactics::Tactics;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttackingStyle {
    /// Exploit space quickly after winning the ball.
    CounterAttack,
    /// Keep the ball and wear the opponent down.
    PossessionGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildUp {
    LongPass,
    ShortPass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttackingArea {
    Wide,
    Centre,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Positioning {
    MaintainFormation,
    Flexible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefensiveStyle {
    FrontlinePressure,
    AllOutDefence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainmentArea {
    /// Protect the middle, forcing the opponent wide.
    Middle,
    /// Close the flanks, forcing the opponent inside.
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pressuring {
    Aggressive,
    Conservative,
}

/// Advanced instructions (PES allows two attacking and two defensive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Instruction {
    // attacking
    TikiTaka,
    WingRotation,
    FalseNine,
    HugTheTouchline,
    AttackingFullBacks,
    FalseFullBacks,
    CentringTargets,
    // defensive
    Gegenpress,
    CounterTarget,
    TightMarking,
    DeepDefensiveLine,
    SwarmTheBox,
    Anchoring,
    FlankProtection,
}

impl Instruction {
    pub fn is_attacking(self) -> bool {
        matches!(
            self,
            Instruction::TikiTaka
                | Instruction::WingRotation
                | Instruction::FalseNine
                | Instruction::HugTheTouchline
                | Instruction::AttackingFullBacks
                | Instruction::FalseFullBacks
                | Instruction::CentringTargets
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            Instruction::TikiTaka => "Tiki-Taka",
            Instruction::WingRotation => "Wing Rotation",
            Instruction::FalseNine => "False No. 9",
            Instruction::HugTheTouchline => "Hug the Touchline",
            Instruction::AttackingFullBacks => "Attacking Full-backs",
            Instruction::FalseFullBacks => "False Full-backs",
            Instruction::CentringTargets => "Centring Targets",
            Instruction::Gegenpress => "Gegenpress",
            Instruction::CounterTarget => "Counter Target",
            Instruction::TightMarking => "Tight Marking",
            Instruction::DeepDefensiveLine => "Deep Defensive Line",
            Instruction::SwarmTheBox => "Swarm the Box",
            Instruction::Anchoring => "Anchoring",
            Instruction::FlankProtection => "Flank Protection",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Instruction::TikiTaka => "short, quick passing in tight triangles around the ball",
            Instruction::WingRotation => {
                "wide players rotate positions to drag markers out of shape"
            }
            Instruction::FalseNine => "the centre-forward drops into midfield to overload it",
            Instruction::HugTheTouchline => {
                "wingers stay glued to the touchline to stretch the defence"
            }
            Instruction::AttackingFullBacks => "full-backs push high to join every attack",
            Instruction::FalseFullBacks => "full-backs tuck into midfield in possession",
            Instruction::CentringTargets => "extra bodies attack the box for crosses",
            Instruction::Gegenpress => "the nearest players swarm the ball the moment it is lost",
            Instruction::CounterTarget => "a forward stays high as an outlet for counter-attacks",
            Instruction::TightMarking => "defenders mark their men touch-tight",
            Instruction::DeepDefensiveLine => "the back line drops deep to deny space in behind",
            Instruction::SwarmTheBox => "the team packs its own penalty area when under pressure",
            Instruction::Anchoring => {
                "the defensive midfielder never leaves the space in front of the defence"
            }
            Instruction::FlankProtection => "wide players track back to double up on the flanks",
        }
    }
}

/// A team's tactical identity.
#[derive(Debug, Clone, PartialEq)]
pub struct TeamStyle {
    pub name: String,
    pub formation: FormationKind,
    pub attacking_style: AttackingStyle,
    pub build_up: BuildUp,
    pub attacking_area: AttackingArea,
    pub positioning: Positioning,
    /// 1 (players close to the ball) – 10 (spread out).
    pub support_range: u8,
    pub defensive_style: DefensiveStyle,
    pub containment_area: ContainmentArea,
    pub pressuring: Pressuring,
    /// 1 (deep) – 10 (high).
    pub defensive_line: u8,
    /// 1 (stretched) – 10 (compact).
    pub compactness: u8,
    /// Advanced instructions; PES allows up to two attacking + two defensive.
    pub instructions: Vec<Instruction>,
}

impl TeamStyle {
    /// Is the instruction set legal (≤ 2 attacking, ≤ 2 defensive, no duplicates)?
    pub fn instructions_valid(&self) -> bool {
        let att = self
            .instructions
            .iter()
            .filter(|i| i.is_attacking())
            .count();
        let def = self.instructions.len() - att;
        let mut sorted: Vec<u8> = self.instructions.iter().map(|&i| i as u8).collect();
        sorted.sort_unstable();
        sorted.dedup();
        att <= 2 && def <= 2 && sorted.len() == self.instructions.len()
    }

    pub fn has(&self, i: Instruction) -> bool {
        self.instructions.contains(&i)
    }

    /// Compile into numeric tactics.
    pub fn compile(&self) -> Tactics {
        let mut t = Tactics::default();
        let sr = self.support_range.clamp(1, 10) as f32;
        let dl = self.defensive_line.clamp(1, 10) as f32;
        let co = self.compactness.clamp(1, 10) as f32;

        match self.attacking_style {
            AttackingStyle::CounterAttack => {
                t.counter_attack = 0.9;
                t.directness = 0.75;
                t.tempo = 0.7;
                t.risk_aversion = 0.9;
            }
            AttackingStyle::PossessionGame => {
                t.counter_attack = 0.25;
                t.directness = 0.3;
                t.tempo = 0.45;
                t.risk_aversion = 1.15;
            }
        }
        match self.build_up {
            BuildUp::LongPass => {
                t.long_ball = 0.8;
                t.directness = (t.directness + 0.2).min(1.0);
            }
            BuildUp::ShortPass => {
                t.long_ball = 0.1;
                t.support_closeness += 0.1;
            }
        }
        match self.attacking_area {
            AttackingArea::Wide => {
                t.wide_play = 0.8;
                t.crossing = 0.65;
                t.width_in_possession = 32.0;
            }
            AttackingArea::Centre => {
                t.wide_play = 0.2;
                t.crossing = 0.2;
                t.width_in_possession = 26.0;
            }
        }
        t.fluidity = match self.positioning {
            Positioning::Flexible => 0.8,
            Positioning::MaintainFormation => 0.25,
        };
        t.support_closeness = (t.support_closeness + (5.5 - sr) * 0.08).clamp(0.0, 1.0);
        t.depth_in_possession = 34.0 + sr * 1.6;

        match self.defensive_style {
            DefensiveStyle::FrontlinePressure => t.press_height = 0.75,
            DefensiveStyle::AllOutDefence => t.press_height = 0.2,
        }
        t.show_wide = match self.containment_area {
            ContainmentArea::Middle => 0.85,
            ContainmentArea::Wide => 0.15,
        };
        match self.pressuring {
            Pressuring::Aggressive => {
                t.press_aggression = 1.35;
                t.counter_press = 0.7;
            }
            Pressuring::Conservative => {
                t.press_aggression = 0.75;
                t.counter_press = 0.3;
            }
        }
        t.line_offset = (dl - 5.5) * 2.2;
        t.press_height = (t.press_height + (dl - 5.5) * 0.04).clamp(0.0, 1.0);
        t.width_out_of_possession = 25.0 - co * 0.8;
        t.depth_out_of_possession = 34.0 - co * 0.8;

        for &i in &self.instructions {
            match i {
                Instruction::TikiTaka => {
                    t.support_closeness = t.support_closeness.max(0.9);
                    t.tempo = t.tempo.max(0.85);
                    t.long_ball = 0.05;
                    t.directness = (t.directness - 0.15).max(0.0);
                }
                Instruction::WingRotation => t.fluidity = (t.fluidity + 0.3).min(1.0),
                Instruction::FalseNine => t.false_nine = true,
                Instruction::HugTheTouchline => t.hug_touchline = true,
                Instruction::AttackingFullBacks => t.fullback_push = 8.0,
                Instruction::FalseFullBacks => t.inverted_fullbacks = true,
                Instruction::CentringTargets => {
                    t.box_crowding = (t.box_crowding + 0.4).min(1.0);
                    t.crossing = (t.crossing + 0.2).min(1.0);
                }
                Instruction::Gegenpress => {
                    t.counter_press = 1.0;
                    t.transition_seconds = 5.0;
                }
                Instruction::CounterTarget => t.counter_target = true,
                Instruction::TightMarking => t.marking_tightness = 0.35,
                Instruction::DeepDefensiveLine => {
                    t.line_offset -= 8.0;
                    t.press_height = (t.press_height - 0.15).max(0.0);
                }
                Instruction::SwarmTheBox => {
                    t.width_out_of_possession -= 4.0;
                    t.line_offset -= 4.0;
                }
                Instruction::Anchoring => t.anchor_dm = true,
                Instruction::FlankProtection => t.flank_protection = true,
            }
        }
        t
    }

    /// Plain-language description of how the team plays.
    pub fn describe(&self) -> String {
        let mut s = format!("{} line up in a {}. ", self.name, self.formation.name());
        s.push_str(match (self.attacking_style, self.build_up) {
            (AttackingStyle::PossessionGame, BuildUp::ShortPass) => {
                "They keep the ball, building patiently from the back with short passes"
            }
            (AttackingStyle::PossessionGame, BuildUp::LongPass) => {
                "They like to have the ball but are happy to go long to switch play"
            }
            (AttackingStyle::CounterAttack, BuildUp::ShortPass) => {
                "They attack quickly after winning the ball, combining at pace through short passes"
            }
            (AttackingStyle::CounterAttack, BuildUp::LongPass) => {
                "They are direct: when they win the ball it goes forward early and long"
            }
        });
        s.push_str(match self.attacking_area {
            AttackingArea::Wide => ", attacking down the flanks",
            AttackingArea::Centre => ", attacking through the middle",
        });
        s.push_str(match self.positioning {
            Positioning::Flexible => " with fluid, interchanging positions. ",
            Positioning::MaintainFormation => " while holding a disciplined shape. ",
        });
        s.push_str(match (self.defensive_style, self.pressuring) {
            (DefensiveStyle::FrontlinePressure, Pressuring::Aggressive) => {
                "Without the ball they press high and aggressively, trying to win it back near the opponent's goal"
            }
            (DefensiveStyle::FrontlinePressure, Pressuring::Conservative) => {
                "Without the ball they hold a high line but press selectively"
            }
            (DefensiveStyle::AllOutDefence, Pressuring::Aggressive) => {
                "Without the ball they drop off but snap into tackles once the opponent enters their half"
            }
            (DefensiveStyle::AllOutDefence, Pressuring::Conservative) => {
                "Without the ball they sit deep in a patient block and invite the opponent on"
            }
        });
        s.push_str(match self.containment_area {
            ContainmentArea::Middle => ", protecting the centre and forcing play wide",
            ContainmentArea::Wide => ", closing down the flanks and forcing play inside",
        });
        let line = match self.defensive_line {
            1..=3 => "a deep line",
            4..=6 => "a medium line",
            _ => "a high line",
        };
        let compact = match self.compactness {
            1..=3 => "stretched",
            4..=6 => "balanced",
            _ => "very compact",
        };
        s.push_str(&format!(", with {line} and a {compact} block."));
        if !self.instructions.is_empty() {
            let parts: Vec<String> = self
                .instructions
                .iter()
                .map(|i| format!("{} ({})", i.name(), i.description()))
                .collect();
            s.push_str(&format!(" Special instructions: {}.", parts.join("; ")));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::presets;

    #[test]
    fn presets_have_valid_instructions() {
        for style in presets::team_archetypes() {
            assert!(style.instructions_valid(), "{}", style.name);
        }
    }

    #[test]
    fn invalid_instruction_sets_are_detected() {
        let mut s = presets::tiki_taka();
        s.instructions = vec![
            Instruction::TikiTaka,
            Instruction::FalseNine,
            Instruction::HugTheTouchline,
        ];
        assert!(!s.instructions_valid());
        s.instructions = vec![Instruction::Gegenpress, Instruction::Gegenpress];
        assert!(!s.instructions_valid());
    }

    #[test]
    fn long_ball_team_compiles_to_long_ball() {
        let lb = presets::long_ball().compile();
        let tt = presets::tiki_taka().compile();
        assert!(lb.long_ball > tt.long_ball + 0.5);
        assert!(lb.directness > tt.directness);
        assert!(tt.support_closeness > lb.support_closeness);
        assert!(tt.tempo > 0.8);
    }

    #[test]
    fn pressing_team_compiles_to_high_press() {
        let gp = presets::gegenpress().compile();
        let bus = presets::park_the_bus().compile();
        assert!(gp.press_height > bus.press_height + 0.4);
        assert!(gp.press_aggression > bus.press_aggression);
        assert!(gp.counter_press > 0.9);
        assert!(gp.line_offset > bus.line_offset + 10.0);
        assert!(bus.counter_target);
    }

    #[test]
    fn description_mentions_key_ideas() {
        let d = presets::gegenpress().describe();
        assert!(d.contains("press high"), "{d}");
        assert!(d.contains("Gegenpress"), "{d}");
        let d = presets::long_ball().describe();
        assert!(d.contains("long"), "{d}");
    }
}

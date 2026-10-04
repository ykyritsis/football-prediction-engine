//! A player's identity: ratings + playing style + COM styles + skills.
//!
//! [`PlayerProfile::behaviour_in`] layers the identity on top of the role
//! defaults, so the same player deployed in different roles still keeps his
//! character (a dribbler played at full-back still dribbles more than other
//! full-backs).

use super::ratings::{norm, Ratings};
use super::styles::{ComStyleSet, PlayingStyle, SkillSet};
use crate::role::{Attributes, Behaviour, Role};

/// Everything that makes a player *that* player.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerProfile {
    pub name: String,
    /// Registered (best) position.
    pub position: Role,
    pub ratings: Ratings,
    pub playing_style: Option<PlayingStyle>,
    pub com_styles: ComStyleSet,
    pub skills: SkillSet,
}

impl PlayerProfile {
    /// Simulation attributes, including the small technical bonuses skills grant.
    pub fn attributes(&self) -> Attributes {
        let mut a = self.ratings.to_attributes();
        for s in self.skills.iter() {
            s.apply_attributes(&mut a);
        }
        a
    }

    /// Effective behaviour when deployed in `role`.
    ///
    /// Order: role defaults → rating-driven tendencies → playing style → COM
    /// styles → skills → clamp.
    pub fn behaviour_in(&self, role: Role) -> Behaviour {
        let mut b = Behaviour::for_role(role);
        let r = &self.ratings;
        let n = norm;

        // Ratings shape tendencies: players lean on their strengths.
        b.dribble_bias += (0.65 * n(r.dribbling) + 0.35 * n(r.ball_control) - 0.6) * 0.6;
        b.shot_threshold -= (n(r.finishing) - 0.55) * 0.05;
        b.shot_range += (n(r.kicking_power) - 0.6) * 10.0;
        b.risk_aversion *= 1.0 - (n(r.low_pass) - 0.6) * 0.4;
        b.through_ball_bias += (n(r.low_pass) - 0.6) * 0.3;
        b.long_pass_bias += (n(r.lofted_pass) - 0.6) * 0.4;
        b.marking_tightness += (n(r.defensive_awareness) - 0.6) * 0.3;
        b.press_willingness += (n(r.stamina) - 0.6) * 0.2;
        if b.run_in_behind > 0.0 {
            b.run_in_behind += (0.5 * n(r.offensive_awareness) + 0.5 * n(r.speed) - 0.6) * 0.4;
        }
        if b.hold_up > 0.0 {
            b.hold_up += (n(r.physical_contact) - 0.6) * 0.4;
        }

        if let Some(style) = self.playing_style {
            style.apply(&mut b);
        }
        for c in self.com_styles.iter() {
            c.apply(&mut b);
        }
        for s in self.skills.iter() {
            s.apply(&mut b);
        }
        b.clamped()
    }

    /// The player's standout abilities, strongest first (ties broken by the
    /// fixed label order so the output is deterministic).
    pub fn strengths(&self, count: usize) -> Vec<(&'static str, u8)> {
        let mut v: Vec<(usize, (&'static str, u8))> =
            self.ratings.labelled().into_iter().enumerate().collect();
        v.sort_by(|a, b| b.1 .1.cmp(&a.1 .1).then(a.0.cmp(&b.0)));
        v.into_iter()
            .take(count)
            .map(|(_, x)| x)
            .filter(|x| x.1 >= 80)
            .collect()
    }

    /// Short tags summarising how the player *feels* in the simulation.
    pub fn signature_traits(&self) -> Vec<&'static str> {
        let b = self.behaviour_in(self.position);
        let base = Behaviour::for_role(self.position);
        let a = self.attributes();
        let mut tags = Vec::new();
        if b.dribble_bias >= base.dribble_bias + 0.2 || (a.dribbling > 0.9 && b.dribble_bias > 0.8)
        {
            tags.push("Takes defenders on");
        }
        if a.finishing >= 0.85 || b.shot_threshold <= base.shot_threshold - 0.025 {
            tags.push("Clinical finisher");
        }
        if b.shot_range >= 30.0 {
            tags.push("Shoots from distance");
        }
        if b.through_ball_bias >= base.through_ball_bias + 0.25 {
            tags.push("Plays killer through balls");
        }
        if b.long_pass_bias >= base.long_pass_bias + 0.3 {
            tags.push("Sprays long passes");
        }
        if b.run_in_behind >= base.run_in_behind + 0.2 && b.run_in_behind > 0.5 {
            tags.push("Runs in behind");
        }
        if b.stays_high >= 0.5 {
            tags.push("Lurks on the last line");
        }
        if b.hold_up >= 0.75 {
            tags.push("Holds the ball up");
        }
        if b.cross_bias >= 0.8 {
            tags.push("Crosses at every chance");
        }
        if b.press_willingness >= 0.95 && self.position != Role::Striker {
            tags.push("Relentless presser");
        }
        if b.marking_tightness >= 0.8 {
            tags.push("Tight marker");
        }
        if b.track_back <= 0.3 && self.position != Role::Goalkeeper {
            tags.push("Excused defensive duties");
        }
        if b.overlap >= 0.95 {
            tags.push("Overlaps constantly");
        }
        if a.top_speed >= 9.0 {
            tags.push("Explosive pace");
        }
        tags
    }

    /// Generated scouting-report style description.
    pub fn describe(&self) -> String {
        let mut out = format!("{} ({})", self.name, self.position.short_name());
        if let Some(style) = self.playing_style {
            out.push_str(&format!(" — {}. {}", style.name(), style.description()));
        } else {
            out.push('.');
        }
        let strengths = self.strengths(3);
        if !strengths.is_empty() {
            let s: Vec<String> = strengths.iter().map(|(l, v)| format!("{l} {v}")).collect();
            out.push_str(&format!(" Standout abilities: {}.", s.join(", ")));
        }
        let com: Vec<&str> = self.com_styles.iter().map(|c| c.name()).collect();
        if !com.is_empty() {
            out.push_str(&format!(" On the ball: {}.", com.join(", ")));
        }
        let tags = self.signature_traits();
        if !tags.is_empty() {
            out.push_str(&format!(" In matches: {}.", tags.join("; ")));
        }
        out
    }
}

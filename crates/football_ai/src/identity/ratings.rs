//! PES-style player ratings.
//!
//! Field names follow the PES 2017/2018 ability list so that values extracted
//! from the game database can be copied across 1:1. Exact in-game labels vary
//! slightly between versions and languages; where a field may be named
//! differently in the extracted data, the doc comment notes the alternative.

use crate::role::Attributes;

/// Preferred foot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Foot {
    Left,
    Right,
}

/// Ability ratings on the PES 40–99 scale (plus a few 1–N fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ratings {
    /// "Offensive Awareness" / "Attacking Prowess".
    pub offensive_awareness: u8,
    pub ball_control: u8,
    pub dribbling: u8,
    pub low_pass: u8,
    pub lofted_pass: u8,
    pub finishing: u8,
    pub place_kicking: u8,
    /// "Controlled Spin" / "Swerve" / "Curl".
    pub curl: u8,
    pub header: u8,
    /// "Defensive Awareness" / "Defensive Prowess".
    pub defensive_awareness: u8,
    pub ball_winning: u8,
    pub kicking_power: u8,
    pub speed: u8,
    pub explosive_power: u8,
    /// "Body Control" / "Unwavering Balance".
    pub body_control: u8,
    pub physical_contact: u8,
    pub jump: u8,
    pub goalkeeping: u8,
    /// "Catching" / "GK Catching".
    pub catching: u8,
    /// "Clearing" / "GK Clearing".
    pub clearing: u8,
    /// "Reflexes" / "GK Reflexes".
    pub reflexes: u8,
    /// "Coverage" / "GK Reach".
    pub coverage: u8,
    pub stamina: u8,
    /// 1–4.
    pub weak_foot_usage: u8,
    /// 1–4.
    pub weak_foot_accuracy: u8,
    /// 1–8.
    pub form: u8,
    /// 1–3.
    pub injury_resistance: u8,
    pub height_cm: u8,
    pub stronger_foot: Foot,
}

/// Map a 40–99 rating onto `[0, 1]`.
#[inline]
pub fn norm(r: u8) -> f32 {
    ((r as f32 - 40.0) / 59.0).clamp(0.0, 1.0)
}

impl Ratings {
    /// A flat profile with every 40–99 ability set to `v`.
    pub fn uniform(v: u8) -> Self {
        Ratings {
            offensive_awareness: v,
            ball_control: v,
            dribbling: v,
            low_pass: v,
            lofted_pass: v,
            finishing: v,
            place_kicking: v,
            curl: v,
            header: v,
            defensive_awareness: v,
            ball_winning: v,
            kicking_power: v,
            speed: v,
            explosive_power: v,
            body_control: v,
            physical_contact: v,
            jump: v,
            goalkeeping: 40,
            catching: 40,
            clearing: 40,
            reflexes: 40,
            coverage: 40,
            stamina: v,
            weak_foot_usage: 2,
            weak_foot_accuracy: 2,
            form: 4,
            injury_resistance: 2,
            height_cm: 180,
            stronger_foot: Foot::Right,
        }
    }

    /// Average of the outfield abilities; a rough "overall" for display.
    pub fn overall_outfield(&self) -> u8 {
        let v = [
            self.offensive_awareness,
            self.ball_control,
            self.dribbling,
            self.low_pass,
            self.lofted_pass,
            self.finishing,
            self.defensive_awareness,
            self.ball_winning,
            self.speed,
            self.explosive_power,
            self.body_control,
            self.physical_contact,
        ];
        (v.iter().map(|&x| x as u32).sum::<u32>() / v.len() as u32) as u8
    }

    /// Goalkeeping overall.
    pub fn overall_goalkeeping(&self) -> u8 {
        let v = [self.goalkeeping, self.catching, self.clearing, self.reflexes, self.coverage];
        (v.iter().map(|&x| x as u32).sum::<u32>() / v.len() as u32) as u8
    }

    /// Convert to the simulation's normalised attributes.
    pub fn to_attributes(&self) -> Attributes {
        let n = norm;
        Attributes {
            top_speed: 6.5 + 3.0 * n(self.speed),
            acceleration: 4.0 + 4.0 * n(self.explosive_power),
            passing: n(self.low_pass),
            long_passing: n(self.lofted_pass),
            control: n(self.ball_control),
            dribbling: 0.65 * n(self.dribbling) + 0.35 * n(self.body_control),
            tackling: 0.6 * n(self.ball_winning) + 0.4 * n(self.defensive_awareness),
            finishing: n(self.finishing),
            kicking_power: n(self.kicking_power),
            goalkeeping: (n(self.goalkeeping) + n(self.catching) + n(self.reflexes) + n(self.coverage)) / 4.0,
        }
    }

    /// Named abilities as `(label, value)` pairs, used for generated
    /// descriptions and UI.
    pub fn labelled(&self) -> [(&'static str, u8); 17] {
        [
            ("offensive awareness", self.offensive_awareness),
            ("ball control", self.ball_control),
            ("dribbling", self.dribbling),
            ("low passing", self.low_pass),
            ("lofted passing", self.lofted_pass),
            ("finishing", self.finishing),
            ("set pieces", self.place_kicking),
            ("curl", self.curl),
            ("heading", self.header),
            ("defensive awareness", self.defensive_awareness),
            ("ball winning", self.ball_winning),
            ("shot power", self.kicking_power),
            ("pace", self.speed),
            ("acceleration", self.explosive_power),
            ("balance", self.body_control),
            ("strength", self.physical_contact),
            ("stamina", self.stamina),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_bounds() {
        assert_eq!(norm(40), 0.0);
        assert_eq!(norm(99), 1.0);
        assert_eq!(norm(0), 0.0);
        assert_eq!(norm(255), 1.0);
    }

    #[test]
    fn faster_rating_means_faster_player() {
        let mut slow = Ratings::uniform(70);
        let mut fast = slow;
        slow.speed = 60;
        fast.speed = 95;
        assert!(fast.to_attributes().top_speed > slow.to_attributes().top_speed);
        assert!(fast.to_attributes().top_speed <= 9.5);
    }
}

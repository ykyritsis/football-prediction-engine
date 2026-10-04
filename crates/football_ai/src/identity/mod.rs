//! Team and player identity.
//!
//! This layer is what makes teams feel like *teams* and players feel like
//! *players*. It is data-oriented and mirrors PES 2017/2018 so the full game
//! can be populated straight from the extracted database:
//!
//! * [`Ratings`] – the 40–99 ability sheet.
//! * [`PlayingStyle`], [`ComStyle`], [`Skill`] – player traits, each with a
//!   description and a concrete effect on decisions.
//! * [`PlayerProfile`] – a player's full identity; compiles to simulation
//!   [`Attributes`](crate::role::Attributes) and a role-aware
//!   [`Behaviour`](crate::role::Behaviour), and can describe itself.
//! * [`TeamStyle`] – the team game plan (attacking/defensive style, lines,
//!   compactness, advanced instructions); compiles to
//!   [`Tactics`](crate::tactics::Tactics) and can describe itself.
//! * [`presets`] – team archetypes, player archetypes, example profiles and
//!   generated squads.
//!
//! How identity reaches the pitch:
//!
//! | Identity                            | Decision-layer effect                                  |
//! |-------------------------------------|--------------------------------------------------------|
//! | Build up: Long Pass / Long Ball Exp. | long passes score higher in pass evaluation            |
//! | Tiki-Taka, low support range         | supporting players come closer, short passes favoured |
//! | Frontline Pressure + Aggressive      | high press phase earlier, stronger pressing triggers  |
//! | Gegenpress                           | swarm the ball during defensive transition            |
//! | Counter Target / Goal Poacher        | forwards stay high out of possession                  |
//! | Mazing Run / Trickster / dribbling   | carrying the ball beats passing more often            |
//! | Finishing / shooting skills          | lower xG threshold to shoot, longer shooting range    |
//! | Through Passing / Creative Playmaker | lead passes into runners favoured                     |
//! | Offensive Full-back / Attacking FBs  | overlaps more often, higher in possession             |
//! | Anchor Man / Anchoring               | DM stays in front of the defence                      |

pub mod presets;
mod profile;
mod ratings;
mod styles;
mod team;

pub use presets::Squad;
pub use profile::PlayerProfile;
pub use ratings::{norm, Foot, Ratings};
pub use styles::{ComStyle, ComStyleSet, PlayingStyle, Skill, SkillSet};
pub use team::{
    AttackingArea, AttackingStyle, BuildUp, ContainmentArea, DefensiveStyle, Instruction,
    Positioning, Pressuring, TeamStyle,
};

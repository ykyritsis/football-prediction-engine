//! Plain-data match state.
//!
//! [`MatchState`] is the *snapshot* the decision layer reads. It contains no
//! behaviour and no references, so in an ECS it can be rebuilt each frame from
//! component queries (22 players + a ball is a trivially small copy) and handed
//! to the pure decision functions. See the crate docs for the Bevy mapping.

use crate::formation::Formation;
use crate::math::Vec2;
use crate::phase::TeamPhase;
use crate::pitch::TeamSide;
use crate::role::{Attributes, Behaviour, Role};
use crate::tactics::Tactics;

/// Number of players per team.
pub const TEAM_SIZE: usize = 11;
/// Number of players on the pitch.
pub const PLAYER_COUNT: usize = TEAM_SIZE * 2;

/// Stable player identifier: `0..11` are home, `11..22` are away. Within a
/// team the index matches the formation slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerId(pub u8);

impl PlayerId {
    #[inline]
    pub fn new(team: TeamSide, slot: usize) -> Self {
        debug_assert!(slot < TEAM_SIZE);
        PlayerId((team.index() * TEAM_SIZE + slot) as u8)
    }

    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn team(self) -> TeamSide {
        if (self.0 as usize) < TEAM_SIZE {
            TeamSide::Home
        } else {
            TeamSide::Away
        }
    }

    #[inline]
    pub fn slot(self) -> usize {
        self.0 as usize % TEAM_SIZE
    }
}

/// Dynamic + static data for one player.
#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub id: PlayerId,
    pub team: TeamSide,
    pub role: Role,
    pub attrs: Attributes,
    /// Effective behaviour: role defaults modified by the player's identity.
    pub behaviour: Behaviour,
    pub pos: Vec2,
    pub vel: Vec2,
    /// Unit vector the player is facing.
    pub facing: Vec2,
    /// Ticks until the player may attempt another tackle.
    pub tackle_cooldown: u16,
    /// Ticks until the player may perform another on-ball action (pass/shot)
    /// or touch the ball again after releasing it.
    pub action_cooldown: u16,
    /// Ticks the player is recovering (e.g. after being tackled) and cannot
    /// control the ball.
    pub stun: u16,
}

impl Player {
    #[inline]
    pub fn is_goalkeeper(&self) -> bool {
        self.role == Role::Goalkeeper
    }

    /// Position in the player's own team-local frame.
    #[inline]
    pub fn local_pos(&self) -> Vec2 {
        self.team.to_local(self.pos)
    }
}

/// A pass that has been struck and not yet resolved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassInFlight {
    pub from: PlayerId,
    pub to: PlayerId,
    pub origin: Vec2,
    pub target: Vec2,
    pub kicked_tick: u32,
}

/// A completed pass, remembered for pressing triggers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassRecord {
    pub team: TeamSide,
    pub from: PlayerId,
    pub to: PlayerId,
    pub origin: Vec2,
    pub received_at: Vec2,
    pub completed_tick: u32,
}

/// A shot in flight, used for goalkeeper reactions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotInFlight {
    pub shooter: PlayerId,
    pub target: Vec2,
    pub kicked_tick: u32,
}

/// Ball state. The ball is modelled on the ground plane.
#[derive(Debug, Clone, PartialEq)]
pub struct Ball {
    pub pos: Vec2,
    pub vel: Vec2,
    /// Player currently in control of the ball.
    pub owner: Option<PlayerId>,
    /// Tick at which the current owner gained control.
    pub owner_since: u32,
    /// Last player to touch the ball.
    pub last_touch: Option<PlayerId>,
    pub pass: Option<PassInFlight>,
    pub shot: Option<ShotInFlight>,
}

/// Kinds of dead-ball restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartKind {
    KickOff,
    ThrowIn,
    GoalKick,
    Corner,
}

/// A pending restart: only `team` may play the ball, opponents keep their
/// distance from `spot`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Restart {
    pub kind: RestartKind,
    pub team: TeamSide,
    pub spot: Vec2,
}

/// Per-team state that persists between ticks.
#[derive(Debug, Clone, PartialEq)]
pub struct TeamState {
    pub side: TeamSide,
    pub formation: Formation,
    pub tactics: Tactics,
    /// Phase chosen on the previous tick (used for hysteresis).
    pub phase: TeamPhase,
}

/// Complete snapshot of a match.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchState {
    pub tick: u32,
    /// Fixed timestep, seconds.
    pub dt: f32,
    /// Players indexed by [`PlayerId::index`].
    pub players: Vec<Player>,
    pub ball: Ball,
    pub teams: [TeamState; 2],
    /// Team currently in possession (the last team to control the ball).
    pub possession: TeamSide,
    /// Tick at which `possession` last changed.
    pub possession_since: u32,
    pub last_completed_pass: Option<PassRecord>,
    pub restart: Option<Restart>,
    pub score: [u16; 2],
}

impl MatchState {
    #[inline]
    pub fn player(&self, id: PlayerId) -> &Player {
        &self.players[id.index()]
    }

    #[inline]
    pub fn team(&self, side: TeamSide) -> &TeamState {
        &self.teams[side.index()]
    }

    /// Players of one team, in slot order.
    #[inline]
    pub fn team_players(&self, side: TeamSide) -> &[Player] {
        let s = side.index() * TEAM_SIZE;
        &self.players[s..s + TEAM_SIZE]
    }

    /// The ball carrier, if any.
    #[inline]
    pub fn carrier(&self) -> Option<&Player> {
        self.ball.owner.map(|id| self.player(id))
    }

    /// The goalkeeper of a team (slot 0 by convention).
    #[inline]
    pub fn goalkeeper(&self, side: TeamSide) -> &Player {
        &self.team_players(side)[0]
    }

    /// Seconds elapsed since `tick`.
    #[inline]
    pub fn seconds_since(&self, tick: u32) -> f32 {
        self.tick.saturating_sub(tick) as f32 * self.dt
    }

    /// Nearest player of `side` to `p`, ties broken by id.
    pub fn nearest_player(&self, side: TeamSide, p: Vec2, include_gk: bool) -> Option<&Player> {
        self.team_players(side)
            .iter()
            .filter(|pl| include_gk || !pl.is_goalkeeper())
            .min_by(|a, b| {
                crate::math::cmp_f32(a.pos.distance_squared(p), b.pos.distance_squared(p)).then(a.id.cmp(&b.id))
            })
    }

    /// Distance from `p` to the nearest player of `side`.
    pub fn nearest_distance(&self, side: TeamSide, p: Vec2) -> f32 {
        self.team_players(side)
            .iter()
            .map(|pl| pl.pos.distance(p))
            .fold(f32::INFINITY, f32::min)
    }

    /// The "offside line" a team's attackers play against, in the attacking
    /// team's local frame: the second-last opponent, but never behind the
    /// halfway line or the ball.
    pub fn offside_line_local(&self, attacking: TeamSide) -> f32 {
        let mut xs: Vec<f32> = self
            .team_players(attacking.opponent())
            .iter()
            .map(|p| attacking.to_local(p.pos).x)
            .collect();
        xs.sort_by(|a, b| b.total_cmp(a));
        let second_last = xs.get(1).copied().unwrap_or(0.0);
        let ball_x = attacking.to_local(self.ball.pos).x;
        second_last.max(0.0).max(ball_x)
    }

    /// Deterministic 64-bit FNV-1a hash of the whole dynamic state. Two runs
    /// with the same seed and inputs must produce identical hashes every tick.
    pub fn state_hash(&self) -> u64 {
        let mut h = Fnv::new();
        h.u32(self.tick);
        for p in &self.players {
            h.vec(p.pos);
            h.vec(p.vel);
            h.vec(p.facing);
            h.u32(p.tackle_cooldown as u32);
            h.u32(p.action_cooldown as u32);
            h.u32(p.stun as u32);
        }
        h.vec(self.ball.pos);
        h.vec(self.ball.vel);
        h.u32(self.ball.owner.map_or(u32::MAX, |o| o.0 as u32));
        h.u32(self.ball.owner_since);
        h.u32(self.ball.last_touch.map_or(u32::MAX, |o| o.0 as u32));
        if let Some(pass) = self.ball.pass {
            h.u32(pass.from.0 as u32);
            h.u32(pass.to.0 as u32);
            h.vec(pass.target);
        }
        h.u32(self.possession.index() as u32);
        h.u32(self.possession_since);
        h.u32(self.score[0] as u32);
        h.u32(self.score[1] as u32);
        if let Some(r) = self.restart {
            h.u32(r.kind as u32);
            h.vec(r.spot);
        }
        for t in &self.teams {
            h.u32(t.phase as u32);
        }
        h.finish()
    }
}

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn u32(&mut self, v: u32) {
        for b in v.to_le_bytes() {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    fn vec(&mut self, v: Vec2) {
        let (x, y) = v.to_bits();
        self.u32(x);
        self.u32(y);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_id_layout() {
        let h = PlayerId::new(TeamSide::Home, 3);
        let a = PlayerId::new(TeamSide::Away, 3);
        assert_eq!(h.0, 3);
        assert_eq!(a.0, 14);
        assert_eq!(h.team(), TeamSide::Home);
        assert_eq!(a.team(), TeamSide::Away);
        assert_eq!(a.slot(), 3);
    }
}

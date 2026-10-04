//! Scenario construction: put the match into a specific situation.
//!
//! Used by tests (to check behaviours in controlled positions) and by the
//! sandbox (preset situations to inspect). All helpers operate on a
//! [`Simulation`] directly; positions in `*_local` helpers are in the given
//! team's local frame (attacking `+x`).

use crate::formation::team_shape;
use crate::identity::presets::{self, Squad};
use crate::math::Vec2;
use crate::phase::phase_for;
use crate::pitch::{clamp_to_pitch, TeamSide};
use crate::sim::{SimConfig, Simulation};
use crate::state::{PlayerId, TEAM_SIZE};

impl Simulation {
    /// Remove any pending restart.
    pub fn clear_restart(&mut self) {
        self.state.restart = None;
    }

    /// Teleport a player (world space) and stop him.
    pub fn place(&mut self, id: PlayerId, pos: Vec2) {
        let p = &mut self.state.players[id.index()];
        p.pos = pos;
        p.vel = Vec2::ZERO;
    }

    /// Teleport a player using his team's local frame.
    pub fn place_local(&mut self, id: PlayerId, local: Vec2) {
        let side = id.team();
        self.place(id, side.to_world(local));
    }

    /// Set a player's velocity (world space).
    pub fn set_velocity(&mut self, id: PlayerId, vel: Vec2) {
        let p = &mut self.state.players[id.index()];
        p.vel = vel;
        if vel.length() > 0.1 {
            p.facing = vel.normalize_or_zero();
        }
    }

    /// Give `id` the ball, settled (first touch done, possession established
    /// long enough that no transition phase applies).
    pub fn set_carrier(&mut self, id: PlayerId) {
        let st = &mut self.state;
        let settle = (30.0 / st.dt) as u32;
        // Advance the clock so "seconds since" arithmetic has headroom.
        st.tick = st.tick.max(settle * 2);
        let p = &mut st.players[id.index()];
        p.action_cooldown = 0;
        p.stun = 0;
        let pos = p.pos;
        let facing = Vec2::new(id.team().direction(), 0.0);
        p.facing = facing;
        st.ball.pos = pos + facing * 0.55;
        st.ball.vel = Vec2::ZERO;
        st.ball.owner = Some(id);
        st.ball.owner_since = st.tick - settle;
        st.ball.last_touch = Some(id);
        st.ball.pass = None;
        st.ball.shot = None;
        st.ball.air_distance = 0.0;
        st.possession = id.team();
        st.possession_since = st.tick - settle;
        st.restart = None;
    }

    /// Mark the current possession as freshly won (transition phases apply).
    pub fn mark_possession_change_now(&mut self) {
        self.state.possession_since = self.state.tick;
    }

    /// Arrange both teams in their formation-relative shape for a ball at
    /// `ball` (world) carried by `carrier`, then give him the ball.
    ///
    /// Shapes are iterated a few times because each team's phase depends on
    /// the ball and the shape depends on the phase and on opponents (offside line).
    pub fn arrange(&mut self, carrier: PlayerId, ball: Vec2) {
        self.place(carrier, ball);
        self.set_carrier(carrier);
        for _ in 0..3 {
            for side in TeamSide::BOTH {
                let phase = phase_for(&self.state, side);
                self.state.teams[side.index()].phase = phase;
                let shape = team_shape(&self.state, side, phase);
                for (slot, &pos) in shape.iter().enumerate().take(TEAM_SIZE) {
                    let id = PlayerId::new(side, slot);
                    if id != carrier {
                        self.place(id, pos);
                    }
                }
            }
        }
        self.place(carrier, ball);
        self.set_carrier(carrier);
    }
}

/// Preset situations for the sandbox and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Preset {
    /// Normal kick-off.
    KickOff,
    /// Home centre-back on the ball in his own third against a high press.
    BuildUpUnderPress,
    /// Home midfielder on the ball in midfield, opponent in a mid block.
    MidfieldProgression,
    /// Home winger on the ball wide in the final third.
    WideFinalThird,
    /// Away team just lost the ball in the home half: home counter-attack.
    CounterAttack,
    /// Away team attacking the home box: home in a low block.
    LowBlockDefending,
}

impl Preset {
    pub const ALL: [Preset; 6] = [
        Preset::KickOff,
        Preset::BuildUpUnderPress,
        Preset::MidfieldProgression,
        Preset::WideFinalThird,
        Preset::CounterAttack,
        Preset::LowBlockDefending,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Preset::KickOff => "Kick-off",
            Preset::BuildUpUnderPress => "Build-up under high press",
            Preset::MidfieldProgression => "Midfield progression vs mid block",
            Preset::WideFinalThird => "Wide in the final third",
            Preset::CounterAttack => "Counter-attack after a turnover",
            Preset::LowBlockDefending => "Defending a low block",
        }
    }

    /// Build the scenario. `home`/`away` default to a gegenpress vs low-block
    /// matchup when `None`.
    pub fn build(self, seed: u64, home: Option<Squad>, away: Option<Squad>) -> Simulation {
        let cfg = SimConfig {
            seed,
            dt: 1.0 / 30.0,
            home: home.unwrap_or_else(|| Squad::generated(presets::gegenpress(), 80)),
            away: away.unwrap_or_else(|| Squad::generated(presets::park_the_bus(), 80)),
        };
        let mut sim = Simulation::new(cfg);
        let a = |slot| PlayerId::new(TeamSide::Away, slot);
        let first_of =
            |sim: &Simulation, side: TeamSide, pred: &dyn Fn(crate::role::Role) -> bool| {
                sim.state
                    .team_players(side)
                    .iter()
                    .find(|p| pred(p.role))
                    .map(|p| p.id)
                    .unwrap_or(PlayerId::new(side, 1))
            };
        use crate::role::Role;
        match self {
            Preset::KickOff => {}
            Preset::BuildUpUnderPress => {
                let cb = first_of(&sim, TeamSide::Home, &|r| r == Role::CentreBack);
                sim.arrange(cb, Vec2::new(-38.0, 6.0));
            }
            Preset::MidfieldProgression => {
                let cm = first_of(&sim, TeamSide::Home, &|r| {
                    matches!(r, Role::CentralMid | Role::DefensiveMid)
                });
                sim.arrange(cm, Vec2::new(-2.0, 4.0));
            }
            Preset::WideFinalThird => {
                let w = first_of(&sim, TeamSide::Home, &|r| {
                    r == Role::Winger || r == Role::FullBack
                });
                let y = TeamSide::Home.to_local(sim.state.player(w).pos).y.signum() * 26.0;
                sim.arrange(w, Vec2::new(33.0, if y == 0.0 { 26.0 } else { y }));
            }
            Preset::CounterAttack => {
                let m = first_of(&sim, TeamSide::Home, &|r| {
                    matches!(r, Role::DefensiveMid | Role::CentralMid)
                });
                sim.arrange(m, Vec2::new(-8.0, -3.0));
                // The away side was attacking: push its players upfield (towards -x).
                for slot in 1..TEAM_SIZE {
                    let id = a(slot);
                    let p = sim.state.player(id).pos;
                    sim.place(id, clamp_to_pitch(Vec2::new(p.x - 18.0, p.y), 2.0));
                }
                sim.mark_possession_change_now();
            }
            Preset::LowBlockDefending => {
                let st = first_of(&sim, TeamSide::Away, &|r| {
                    matches!(r, Role::AttackingMid | Role::CentralMid | Role::Striker)
                });
                sim.arrange(st, Vec2::new(-30.0, -4.0));
            }
        }
        sim
    }
}

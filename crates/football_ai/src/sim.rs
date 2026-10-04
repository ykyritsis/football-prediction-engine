//! Deterministic fixed-timestep match simulation.
//!
//! Each [`Simulation::step`]:
//!
//! 1. **Plan** – [`plan_team`] for both teams from the current snapshot.
//! 2. **Decide** – [`decide`] for all 22 players from the same snapshot.
//! 3. **Act** – apply on-ball actions and tackles in player-id order.
//! 4. **Move** – steer players towards their targets, integrate the ball.
//! 5. **Resolve** – goals, out of play, restarts, ball control.
//!
//! Steps 1–2 are pure and order-independent; 3–5 consume random numbers from
//! the single seeded [`SimRng`] in a fixed order. Two simulations built from
//! the same config therefore produce bit-identical states every tick (see
//! [`MatchState::state_hash`]).

use crate::ball::{apply_friction, CONTROL_RADIUS, SHOT_SPEED};
use crate::decision::{decide_all, Action, Decision, TACKLE_RANGE};
use crate::formation::{team_shape, Formation};
use crate::identity::presets::{self, Squad};
use crate::math::{cmp_f32, Vec2};
use crate::phase::TeamPhase;
use crate::pitch::{clamp_to_pitch, TeamSide, CENTRE_CIRCLE_RADIUS, GOAL_HALF_WIDTH, HALF_LENGTH, HALF_WIDTH};
use crate::plan::{plan_team, TeamPlan};
use crate::rng::SimRng;
use crate::role::Role;
use crate::state::*;

/// Match configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct SimConfig {
    pub seed: u64,
    /// Fixed timestep (s).
    pub dt: f32,
    pub home: Squad,
    pub away: Squad,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            seed: 1,
            dt: 1.0 / 30.0,
            home: Squad::generated(presets::gegenpress(), 78),
            away: Squad::generated(presets::long_ball(), 78),
        }
    }
}

/// Something notable that happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatchEvent {
    Pass { from: PlayerId, to: PlayerId, kind: crate::passing::PassKind },
    PassCompleted { from: PlayerId, to: PlayerId },
    Interception { by: PlayerId, from: PlayerId },
    Tackle { by: PlayerId, on: PlayerId, won: bool },
    Shot { by: PlayerId, xg: f32 },
    Save { by: PlayerId, held: bool },
    Goal { team: TeamSide, scorer: Option<PlayerId> },
    Clearance { by: PlayerId },
    Offside { player: PlayerId },
    OutOfPlay { restart: RestartKind, team: TeamSide },
    PossessionChange { team: TeamSide },
}

/// The simulation.
#[derive(Debug, Clone)]
pub struct Simulation {
    pub state: MatchState,
    pub rng: SimRng,
    /// Squads (names and identities) by team index.
    pub squads: [Squad; 2],
    /// Plans and decisions from the most recent step (for inspection).
    pub plans: Option<[TeamPlan; 2]>,
    pub decisions: Vec<Decision>,
    /// `(tick, event)` log.
    pub events: Vec<(u32, MatchEvent)>,
}

fn secs(state: &MatchState, s: f32) -> u16 {
    (s / state.dt).round().max(0.0) as u16
}

impl Simulation {
    pub fn new(config: SimConfig) -> Self {
        let SimConfig { seed, dt, home, away } = config;
        let mut players = Vec::with_capacity(PLAYER_COUNT);
        let mut teams = Vec::with_capacity(2);
        for (side, squad) in [(TeamSide::Home, &home), (TeamSide::Away, &away)] {
            let formation = Formation::new(squad.style.formation);
            for (slot, profile) in squad.players.iter().enumerate().take(TEAM_SIZE) {
                let role = formation.role(slot);
                players.push(Player {
                    id: PlayerId::new(side, slot),
                    team: side,
                    role,
                    attrs: profile.attributes(),
                    behaviour: profile.behaviour_in(role),
                    pos: Vec2::ZERO,
                    vel: Vec2::ZERO,
                    facing: Vec2::new(side.direction(), 0.0),
                    tackle_cooldown: 0,
                    action_cooldown: 0,
                    stun: 0,
                });
            }
            teams.push(TeamState { side, formation, tactics: squad.style.compile(), phase: TeamPhase::MidBlock });
        }
        assert_eq!(players.len(), PLAYER_COUNT, "each squad needs eleven players");
        let state = MatchState {
            tick: 0,
            dt,
            players,
            ball: Ball {
                pos: Vec2::ZERO,
                vel: Vec2::ZERO,
                owner: None,
                owner_since: 0,
                last_touch: None,
                pass: None,
                shot: None,
                air_distance: 0.0,
            },
            teams: [teams[0].clone(), teams[1].clone()],
            possession: TeamSide::Home,
            possession_since: 0,
            last_completed_pass: None,
            restart: None,
            score: [0, 0],
        };
        let mut sim = Simulation {
            state,
            rng: SimRng::new(seed),
            squads: [home, away],
            plans: None,
            decisions: Vec::new(),
            events: Vec::new(),
        };
        sim.kick_off(TeamSide::Home);
        sim
    }

    /// Display name of a player.
    pub fn name(&self, id: PlayerId) -> &str {
        &self.squads[id.team().index()].players[id.slot()].name
    }

    fn event(&mut self, e: MatchEvent) {
        self.events.push((self.state.tick, e));
    }

    /// Reset positions for a kick-off by `team`.
    pub fn kick_off(&mut self, team: TeamSide) {
        let st = &mut self.state;
        st.ball.pos = Vec2::ZERO;
        st.ball.vel = Vec2::ZERO;
        st.ball.pass = None;
        st.ball.shot = None;
        st.ball.air_distance = 0.0;
        st.possession = team;
        st.possession_since = st.tick;
        for side in TeamSide::BOTH {
            let phase = if side == team { TeamPhase::BuildUp } else { TeamPhase::MidBlock };
            st.teams[side.index()].phase = phase;
            let shape = team_shape(st, side, phase);
            for slot in 0..TEAM_SIZE {
                let mut local = side.to_local(shape[slot]);
                local.x = local.x.min(-1.0);
                if side != team && local.length() < CENTRE_CIRCLE_RADIUS + 0.5 {
                    local = local.normalize_or_zero() * (CENTRE_CIRCLE_RADIUS + 0.5);
                }
                let p = &mut st.players[PlayerId::new(side, slot).index()];
                p.pos = side.to_world(local);
                p.vel = Vec2::ZERO;
                p.facing = Vec2::new(side.direction(), 0.0);
                p.stun = 0;
                p.action_cooldown = 0;
                p.tackle_cooldown = 0;
            }
        }
        // The most advanced player of the kicking team takes it.
        let taker = st
            .team_players(team)
            .iter()
            .filter(|p| p.role != Role::Goalkeeper)
            .max_by(|a, b| cmp_f32(team.to_local(a.pos).x, team.to_local(b.pos).x).then(b.id.cmp(&a.id)))
            .map(|p| p.id)
            .unwrap();
        st.players[taker.index()].pos = team.to_world(Vec2::new(-0.4, 0.0));
        st.ball.owner = Some(taker);
        st.ball.owner_since = st.tick;
        st.ball.last_touch = Some(taker);
        st.players[taker.index()].action_cooldown = secs(st, 0.5);
        st.restart = Some(Restart { kind: RestartKind::KickOff, team, spot: Vec2::ZERO });
    }

    /// Advance one fixed timestep.
    pub fn step(&mut self) {
        let plans = [plan_team(&self.state, TeamSide::Home), plan_team(&self.state, TeamSide::Away)];
        let decisions = decide_all(&self.state, &plans);
        for side in TeamSide::BOTH {
            self.state.teams[side.index()].phase = plans[side.index()].phase;
        }
        self.apply_actions(&decisions);
        self.move_players(&decisions);
        self.move_ball();
        self.resolve_boundaries();
        self.resolve_control();
        self.tick_cooldowns();
        self.state.tick += 1;
        self.plans = Some(plans);
        self.decisions = decisions;
    }

    /// Run `n` steps.
    pub fn run(&mut self, n: u32) {
        for _ in 0..n {
            self.step();
        }
    }

    // ------------------------------------------------------------ actions --

    fn apply_actions(&mut self, decisions: &[Decision]) {
        for d in decisions {
            match d.action {
                Action::None => {}
                Action::Pass(c) => self.do_pass(d.player, c),
                Action::Shoot(s) => self.do_shot(d.player, s),
                Action::Clear { target, speed } => self.do_clear(d.player, target, speed),
                Action::Tackle(on) => self.do_tackle(d.player, on),
            }
        }
    }

    fn release(&mut self, by: PlayerId) {
        let dt = self.state.dt;
        let st = &mut self.state;
        st.ball.owner = None;
        st.ball.last_touch = Some(by);
        st.restart = None;
        let p = &mut st.players[by.index()];
        p.action_cooldown = (0.35 / dt).round() as u16;
    }

    fn do_pass(&mut self, by: PlayerId, c: crate::passing::PassCandidate) {
        if self.state.ball.owner != Some(by) || self.state.player(by).action_cooldown > 0 {
            return;
        }
        let p = self.state.player(by).clone();
        let lofted = c.kind.is_lofted();
        let acc = if lofted { p.attrs.long_passing } else { p.attrs.passing };
        let spread = (1.0 - acc) * c.distance * 0.07 + 0.15;
        let along = (c.target - p.pos).normalize_or_zero();
        let err = along.perp() * self.rng.range(-spread, spread) + along * self.rng.range(-spread, spread) * 0.5;
        let target = c.target + err;
        let dir = (target - p.pos).normalize_or_zero();
        let st = &mut self.state;
        st.ball.vel = dir * c.speed;
        st.ball.pos = p.pos + dir * 0.4;
        let offside_applies = !st.restart.is_some_and(|r| r.kind.exempt_from_offside());
        let receiver_offside = offside_applies && st.in_offside_position(c.receiver);
        st.ball.pass =
            Some(PassInFlight { from: by, to: c.receiver, origin: p.pos, target, kicked_tick: st.tick, lofted, receiver_offside });
        st.ball.air_distance = if lofted { p.pos.distance(target) * 0.97 } else { 0.0 };
        st.ball.shot = None;
        self.release(by);
        self.event(MatchEvent::Pass { from: by, to: c.receiver, kind: c.kind });
    }

    fn do_shot(&mut self, by: PlayerId, s: crate::decision::ShotOption) {
        if self.state.ball.owner != Some(by) || self.state.player(by).action_cooldown > 0 {
            return;
        }
        let p = self.state.player(by).clone();
        let dist = p.pos.distance(s.target);
        let pressure = crate::eval::pressure_at(&self.state, p.team, p.pos);
        let spread = (1.0 - p.attrs.finishing) * dist * 0.06 + 0.25 + pressure * 1.5;
        let target = s.target + Vec2::new(0.0, self.rng.range(-spread, spread));
        let dir = (target - p.pos).normalize_or_zero();
        let speed = SHOT_SPEED * (0.85 + 0.15 * p.attrs.kicking_power);
        let st = &mut self.state;
        st.ball.vel = dir * speed;
        st.ball.pos = p.pos + dir * 0.4;
        st.ball.pass = None;
        st.ball.air_distance = 0.0;
        st.ball.shot = Some(ShotInFlight { shooter: by, target, kicked_tick: st.tick, xg: s.xg, keeper_attempted: false });
        self.release(by);
        self.event(MatchEvent::Shot { by, xg: s.xg });
    }

    fn do_clear(&mut self, by: PlayerId, target: Vec2, speed: f32) {
        if self.state.ball.owner != Some(by) || self.state.player(by).action_cooldown > 0 {
            return;
        }
        let p = self.state.player(by).clone();
        let target = target + Vec2::new(self.rng.range(-6.0, 6.0), self.rng.range(-6.0, 6.0));
        let dir = (target - p.pos).normalize_or_zero();
        let st = &mut self.state;
        st.ball.vel = dir * speed;
        st.ball.pos = p.pos + dir * 0.4;
        st.ball.pass = None;
        st.ball.shot = None;
        st.ball.air_distance = p.pos.distance(target) * 0.9;
        self.release(by);
        self.event(MatchEvent::Clearance { by });
    }

    fn do_tackle(&mut self, by: PlayerId, on: PlayerId) {
        let dt = self.state.dt;
        let (t, c) = (self.state.player(by).clone(), self.state.player(on).clone());
        if self.state.ball.owner != Some(on) || t.tackle_cooldown > 0 || t.pos.distance(c.pos) > TACKLE_RANGE {
            return;
        }
        let p_win = (0.22 + 0.5 * (t.attrs.tackling - 0.9 * c.attrs.dribbling)).clamp(0.05, 0.6);
        let won = self.rng.chance(p_win);
        let keep = self.rng.chance(0.45);
        let angle = self.rng.range(-0.9, 0.9);
        let kick = self.rng.range(3.0, 7.0);
        let st = &mut self.state;
        st.players[by.index()].tackle_cooldown = (1.8 / dt) as u16;
        if won {
            st.players[on.index()].stun = (0.6 / dt) as u16;
            st.ball.last_touch = Some(by);
            if keep {
                Self::give_ball(st, by);
            } else {
                st.ball.owner = None;
                let away = (c.pos - t.pos).normalize_or_zero();
                let dir = away + away.perp() * angle;
                st.ball.vel = dir.normalize_or_zero() * kick;
            }
        } else {
            // Beaten: the tackler is off balance.
            st.players[by.index()].stun = (0.45 / dt) as u16;
        }
        self.event(MatchEvent::Tackle { by, on, won });
        if won && keep {
            self.after_control(by);
        }
    }

    // ----------------------------------------------------------- movement --

    fn move_players(&mut self, decisions: &[Decision]) {
        let dt = self.state.dt;
        let positions: Vec<Vec2> = self.state.players.iter().map(|p| p.pos).collect();
        let owner = self.state.ball.owner;
        let restart_spot = self.state.restart.map(|r| r.spot);
        for (i, d) in decisions.iter().enumerate() {
            let p = &mut self.state.players[i];
            let mut cap = p.attrs.top_speed * d.speed.factor();
            if owner == Some(p.id) {
                cap *= 0.72 + 0.15 * p.attrs.dribbling;
                if restart_spot.is_some() {
                    cap = 0.0;
                }
            }
            if p.stun > 0 {
                cap *= 0.25;
            }
            let to = d.target - p.pos;
            let dist = to.length();
            let desired_speed = cap.min(dist * 2.0);
            let desired = to.normalize_or_zero() * desired_speed;
            // Light separation so players don't stack on the same spot.
            let mut sep = Vec2::ZERO;
            for (j, &q) in positions.iter().enumerate() {
                if j == i {
                    continue;
                }
                let off = p.pos - q;
                let d2 = off.length_squared();
                if d2 < 1.0 && d2 > 1e-6 {
                    sep += off.normalize_or_zero() * (1.0 - d2.sqrt()) * 3.0;
                }
            }
            let dv = (desired + sep - p.vel).clamp_length(p.attrs.acceleration * dt);
            p.vel = (p.vel + dv).clamp_length(p.attrs.top_speed);
            p.pos = clamp_to_pitch(p.pos + p.vel * dt, if owner == Some(p.id) { 0.6 } else { -1.5 });
            if p.vel.length() > 0.4 {
                p.facing = p.vel.normalize_or_zero();
            }
        }
    }

    fn move_ball(&mut self) {
        let dt = self.state.dt;
        let st = &mut self.state;
        if let Some(o) = st.ball.owner {
            let p = &st.players[o.index()];
            st.ball.pos = p.pos + p.facing * 0.55;
            st.ball.vel = p.vel;
            return;
        }
        let speed = st.ball.vel.length();
        if st.ball.air_distance > 0.0 {
            // In flight: constant horizontal speed, damped on landing.
            let step = speed * dt;
            if step >= st.ball.air_distance {
                st.ball.pos += st.ball.vel.normalize_or_zero() * st.ball.air_distance;
                st.ball.air_distance = 0.0;
                st.ball.vel *= crate::ball::LANDING_DAMPING;
            } else {
                st.ball.air_distance -= step;
                st.ball.pos += st.ball.vel * dt;
            }
        } else {
            st.ball.vel = apply_friction(st.ball.vel, dt);
            st.ball.pos += st.ball.vel * dt;
        }
        if st.ball.vel.length() < 0.3 {
            st.ball.pass = None;
            st.ball.air_distance = 0.0;
        }
        if st.ball.shot.is_some() && st.ball.vel.length() < 6.0 {
            st.ball.shot = None;
        }
    }

    // --------------------------------------------------------- boundaries --

    fn resolve_boundaries(&mut self) {
        let b = self.state.ball.pos;
        if self.state.ball.owner.is_some() || (b.x.abs() <= HALF_LENGTH && b.y.abs() <= HALF_WIDTH) {
            return;
        }
        let last = self.state.ball.last_touch.map(|p| p.team()).unwrap_or(TeamSide::Home);
        if b.x.abs() > HALF_LENGTH {
            // Which team defends this end?
            let defending = if b.x > 0.0 { TeamSide::Away } else { TeamSide::Home };
            let attacking = defending.opponent();
            // Interpolate the crossing point on the goal line.
            let v = self.state.ball.vel;
            let back = if v.x.abs() > 1e-4 { (b.x.abs() - HALF_LENGTH) / v.x.abs() } else { 0.0 };
            let cross_y = b.y - v.y * back;
            if cross_y.abs() < GOAL_HALF_WIDTH {
                let scorer = self.state.ball.last_touch;
                self.state.score[attacking.index()] += 1;
                self.event(MatchEvent::Goal { team: attacking, scorer });
                self.kick_off(defending);
                return;
            }
            let (kind, team, spot) = if last == defending {
                let sy = b.y.signum() * (HALF_WIDTH - 0.5);
                (RestartKind::Corner, attacking, Vec2::new(b.x.signum() * (HALF_LENGTH - 0.5), sy))
            } else {
                (RestartKind::GoalKick, defending, Vec2::new(b.x.signum() * (HALF_LENGTH - 5.5), 0.0))
            };
            self.set_restart(kind, team, spot);
        } else {
            let team = last.opponent();
            let spot = Vec2::new(b.x.clamp(-HALF_LENGTH + 1.0, HALF_LENGTH - 1.0), b.y.signum() * (HALF_WIDTH - 0.3));
            self.set_restart(RestartKind::ThrowIn, team, spot);
        }
    }

    fn set_restart(&mut self, kind: RestartKind, team: TeamSide, spot: Vec2) {
        let st = &mut self.state;
        st.ball.pos = spot;
        st.ball.vel = Vec2::ZERO;
        st.ball.owner = None;
        st.ball.pass = None;
        st.ball.shot = None;
        st.ball.air_distance = 0.0;
        st.restart = Some(Restart { kind, team, spot });
        if st.possession != team {
            st.possession = team;
            st.possession_since = st.tick;
        }
        self.event(MatchEvent::OutOfPlay { restart: kind, team });
    }

    // ------------------------------------------------------------ control --

    fn give_ball(st: &mut MatchState, id: PlayerId) {
        st.ball.owner = Some(id);
        st.ball.owner_since = st.tick;
        st.ball.last_touch = Some(id);
        st.ball.air_distance = 0.0;
        if st.possession != id.team() {
            st.possession = id.team();
            st.possession_since = st.tick;
        }
    }

    /// Bookkeeping after `id` gains control: first-touch delay, events.
    fn after_control(&mut self, id: PlayerId) {
        let st = &mut self.state;
        let p = &st.players[id.index()];
        let tactics = &st.teams[p.team.index()].tactics;
        let tempo = 0.5 * tactics.tempo + 0.5 * p.behaviour.tempo;
        let delay = 0.65 - 0.5 * tempo + (1.0 - p.attrs.control) * 0.2;
        st.players[id.index()].action_cooldown = (delay / st.dt).round() as u16;
    }

    fn resolve_control(&mut self) {
        if self.state.ball.owner.is_some() || self.state.ball.air_distance > 0.0 {
            return;
        }
        let ball_pos = self.state.ball.pos;
        let speed = self.state.ball.vel.length();
        let pass = self.state.ball.pass;
        let shot = self.state.ball.shot;
        let restart_team = self.state.restart.map(|r| r.team);

        // Candidates: within reach, ordered intended receiver first, then by distance and id.
        let mut cands: Vec<(u8, f32, PlayerId)> = Vec::new();
        for p in &self.state.players {
            if p.stun > 0 || (p.action_cooldown > 0 && self.state.ball.last_touch == Some(p.id)) {
                continue;
            }
            if restart_team.is_some_and(|t| t != p.team) {
                continue;
            }
            let keeper_reach = if p.is_goalkeeper() && crate::pitch::in_own_box_local(p.team.to_local(p.pos)) {
                if shot.is_some_and(|s| s.shooter.team() != p.team && !s.keeper_attempted) {
                    1.6 + 1.7 * p.attrs.goalkeeping
                } else {
                    1.8
                }
            } else {
                CONTROL_RADIUS
            };
            let d = p.pos.distance(ball_pos);
            if d > keeper_reach {
                continue;
            }
            // Team-mates let a pass run through to the intended receiver.
            if let Some(ps) = pass {
                if ps.to != p.id && ps.from.team() == p.team && speed > 4.0 {
                    continue;
                }
            }
            let priority = if pass.is_some_and(|ps| ps.to == p.id) { 0 } else { 1 };
            cands.push((priority, d, p.id));
        }
        cands.sort_by(|a, b| a.0.cmp(&b.0).then(cmp_f32(a.1, b.1)).then(a.2.cmp(&b.2)));

        for (_, _, id) in cands {
            let p = self.state.player(id).clone();
            // Goalkeeper facing a shot.
            if let Some(s) = shot.filter(|s| s.shooter.team() != p.team) {
                if p.is_goalkeeper() {
                    let save = (0.15 + 0.75 * p.attrs.goalkeeping - 0.5 * s.xg).clamp(0.05, 0.95);
                    let saved = self.rng.chance(save);
                    let held = self.rng.chance(0.55);
                    let deflect = self.rng.range(-1.0, 1.0);
                    if let Some(sh) = self.state.ball.shot.as_mut() {
                        sh.keeper_attempted = true;
                    }
                    if !saved {
                        continue;
                    }
                    if held {
                        Self::give_ball(&mut self.state, id);
                        self.state.ball.shot = None;
                        self.state.ball.pass = None;
                        self.after_control(id);
                    } else {
                        let v = self.state.ball.vel;
                        let parry = Vec2::new(-v.x, v.y + deflect * 10.0).normalize_or_zero() * 9.0;
                        self.state.ball.vel = parry;
                        self.state.ball.last_touch = Some(id);
                        self.state.ball.shot = None;
                    }
                    self.event(MatchEvent::Save { by: id, held });
                    return;
                }
                // Outfield block.
                if self.rng.chance(0.5) {
                    let v = self.state.ball.vel;
                    self.state.ball.vel = (v.perp() * self.rng.range(-0.5, 0.5) - v * 0.3).clamp_length(10.0);
                    self.state.ball.last_touch = Some(id);
                    self.state.ball.shot = None;
                    return;
                }
                continue;
            }

            let intended = pass.is_some_and(|ps| ps.to == id);
            let opponent_of_passer = pass.is_some_and(|ps| ps.from.team() != p.team);
            let p_control = if intended {
                (1.25 - speed / 28.0 + 0.3 * p.attrs.control).clamp(0.55, 0.99)
            } else if opponent_of_passer {
                (0.95 - speed / 30.0 + 0.15 * p.attrs.tackling).clamp(0.2, 0.9)
            } else {
                (1.15 - speed / 25.0 + 0.2 * p.attrs.control).clamp(0.3, 0.99)
            };
            if self.rng.chance(p_control) {
                if pass.is_some_and(|ps| ps.to == id && ps.receiver_offside) {
                    self.event(MatchEvent::Offside { player: id });
                    let spot = clamp_to_pitch(p.pos, 1.0);
                    self.set_restart(RestartKind::FreeKick, p.team.opponent(), spot);
                    return;
                }
                let from_team = self.state.possession;
                Self::give_ball(&mut self.state, id);
                if let Some(ps) = pass {
                    if ps.to == id {
                        self.state.last_completed_pass = Some(PassRecord {
                            team: p.team,
                            from: ps.from,
                            to: id,
                            origin: ps.origin,
                            received_at: ball_pos,
                            completed_tick: self.state.tick,
                        });
                        self.event(MatchEvent::PassCompleted { from: ps.from, to: id });
                    } else if ps.from.team() != p.team {
                        self.event(MatchEvent::Interception { by: id, from: ps.from });
                    }
                }
                self.state.ball.pass = None;
                self.state.ball.shot = None;
                if from_team != p.team {
                    self.event(MatchEvent::PossessionChange { team: p.team });
                }
                self.after_control(id);
                return;
            }
            // Failed control: the ball deflects off the player.
            let v = self.state.ball.vel;
            let dev = self.rng.range(-0.6, 0.6);
            self.state.ball.vel = (v + v.perp() * dev) * 0.35;
            self.state.ball.last_touch = Some(id);
            self.state.ball.pass = None;
            self.state.players[id.index()].action_cooldown = (0.25 / self.state.dt) as u16;
            return;
        }
    }

    fn tick_cooldowns(&mut self) {
        for p in &mut self.state.players {
            p.tackle_cooldown = p.tackle_cooldown.saturating_sub(1);
            p.action_cooldown = p.action_cooldown.saturating_sub(1);
            p.stun = p.stun.saturating_sub(1);
        }
    }
}

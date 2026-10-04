//! Deterministic fixed-timestep match simulation.
//!
//! Each [`Simulation::step`]:
//!
//! 1. **Plan** – [`plan_team`] for both teams from the current snapshot.
//! 2. **Decide** – [`decide`] for all 22 players from the same snapshot.
//! 3. **Act** – apply on-ball actions and tackles in player-id order.
//! 4. **Move** – steer players towards their targets, integrate the ball.
//! 5. **Resolve** – goals, out of play, restarts, ball control, advantage,
//!    the match clock.
//!
//! Fouls, cards, free kicks, penalties and time follow [`crate::rules`].
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
use crate::pitch::{
    clamp_to_pitch, TeamSide, CENTRE_CIRCLE_RADIUS, GOAL_HALF_WIDTH, HALF_LENGTH, HALF_WIDTH,
};
use crate::plan::{plan_team, TeamPlan};
use crate::rng::SimRng;
use crate::role::Role;
use crate::rules::{self, CardDecision, Challenge, MatchClock, Referee};
use crate::state::*;

/// Match configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct SimConfig {
    pub seed: u64,
    /// Fixed timestep (s).
    pub dt: f32,
    pub home: Squad,
    pub away: Squad,
    pub referee: Referee,
    pub clock: MatchClock,
}

impl SimConfig {
    /// 30 Hz, default referee and 2 × 45 minutes.
    pub fn new(seed: u64, home: Squad, away: Squad) -> Self {
        Self {
            seed,
            dt: 1.0 / 30.0,
            home,
            away,
            referee: Referee::default(),
            clock: MatchClock::default(),
        }
    }
}

impl Default for SimConfig {
    fn default() -> Self {
        Self::new(
            1,
            Squad::generated(presets::gegenpress(), 78),
            Squad::generated(presets::long_ball(), 78),
        )
    }
}

/// Something notable that happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatchEvent {
    Pass {
        from: PlayerId,
        to: PlayerId,
        kind: crate::passing::PassKind,
    },
    PassCompleted {
        from: PlayerId,
        to: PlayerId,
    },
    Interception {
        by: PlayerId,
        from: PlayerId,
    },
    Tackle {
        by: PlayerId,
        on: PlayerId,
        won: bool,
    },
    Shot {
        by: PlayerId,
        xg: f32,
        kind: ShotKind,
    },
    Save {
        by: PlayerId,
        held: bool,
    },
    Goal {
        team: TeamSide,
        scorer: Option<PlayerId>,
    },
    Clearance {
        by: PlayerId,
    },
    Offside {
        player: PlayerId,
    },
    OutOfPlay {
        restart: RestartKind,
        team: TeamSide,
    },
    PossessionChange {
        team: TeamSide,
    },
    Foul {
        by: PlayerId,
        on: PlayerId,
        spot: Vec2,
    },
    /// The referee lets play continue after a foul.
    Advantage {
        team: TeamSide,
    },
    /// Advantage did not accrue: play is brought back for the free kick.
    AdvantageCalledBack {
        team: TeamSide,
    },
    YellowCard {
        player: PlayerId,
    },
    RedCard {
        player: PlayerId,
        second_yellow: bool,
    },
    PenaltyAwarded {
        team: TeamSide,
    },
    /// `slot` now holds the substitute; the substituted player went to the
    /// bench at `bench_index`.
    Substitution {
        team: TeamSide,
        slot: u8,
        bench_index: u8,
    },
    HalfTime,
    FullTime,
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
    pub referee: Referee,
    pub clock: MatchClock,
    /// Bench players already used (in or out), by team.
    pub bench_used: [Vec<bool>; 2],
}

/// Maximum substitutions per team (PES 2018 era rules).
pub const MAX_SUBSTITUTIONS: u8 = 3;

fn secs(state: &MatchState, s: f32) -> u16 {
    (s / state.dt).round().max(0.0) as u16
}

impl Simulation {
    pub fn new(config: SimConfig) -> Self {
        let SimConfig {
            seed,
            dt,
            home,
            away,
            referee,
            clock,
        } = config;
        let mut players = Vec::with_capacity(PLAYER_COUNT);
        let mut teams = Vec::with_capacity(2);
        for (side, squad) in [(TeamSide::Home, &home), (TeamSide::Away, &away)] {
            let formation = Formation::new(squad.style.formation);
            for (slot, profile) in squad.players.iter().enumerate().take(TEAM_SIZE) {
                let role = formation.role(slot);
                players.push(Player::new(
                    PlayerId::new(side, slot),
                    role,
                    profile.attributes(),
                    profile.behaviour_in(role),
                ));
            }
            teams.push(TeamState {
                side,
                formation,
                tactics: squad.style.compile(),
                phase: TeamPhase::MidBlock,
            });
        }
        assert_eq!(
            players.len(),
            PLAYER_COUNT,
            "each squad needs eleven players"
        );
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
            period: Period::FirstHalf,
            subs_used: [0, 0],
            period_start_tick: 0,
            stoppage_secs: 0.0,
            first_kick_off: TeamSide::Home,
            advantage: None,
        };
        let mut sim = Simulation {
            state,
            rng: SimRng::new(seed),
            squads: [home, away],
            plans: None,
            decisions: Vec::new(),
            events: Vec::new(),
            referee,
            clock,
            bench_used: [Vec::new(), Vec::new()],
        };
        for side in TeamSide::BOTH {
            sim.bench_used[side.index()] = vec![false; sim.squads[side.index()].bench.len()];
        }
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
        st.advantage = None;
        for side in TeamSide::BOTH {
            let phase = if side == team {
                TeamPhase::BuildUp
            } else {
                TeamPhase::MidBlock
            };
            st.teams[side.index()].phase = phase;
            let shape = team_shape(st, side, phase);
            for (slot, &target) in shape.iter().enumerate() {
                let mut local = side.to_local(target);
                local.x = local.x.min(-1.0);
                if side != team && local.length() < CENTRE_CIRCLE_RADIUS + 0.5 {
                    local = local.normalize_or_zero() * (CENTRE_CIRCLE_RADIUS + 0.5);
                }
                let p = &mut st.players[PlayerId::new(side, slot).index()];
                if p.sent_off {
                    continue;
                }
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
            .on_pitch(team)
            .filter(|p| p.role != Role::Goalkeeper)
            .max_by(|a, b| {
                cmp_f32(team.to_local(a.pos).x, team.to_local(b.pos).x).then(b.id.cmp(&a.id))
            })
            .map(|p| p.id)
            .unwrap();
        st.players[taker.index()].pos = team.to_world(Vec2::new(-0.4, 0.0));
        st.ball.owner = Some(taker);
        st.ball.owner_since = st.tick;
        st.ball.last_touch = Some(taker);
        st.players[taker.index()].action_cooldown = secs(st, 0.5);
        st.restart = Some(Restart {
            kind: RestartKind::KickOff,
            team,
            spot: Vec2::ZERO,
        });
    }

    /// Advance one fixed timestep.
    pub fn step(&mut self) {
        if self.state.period == Period::FullTime {
            return;
        }
        let plans = [
            plan_team(&self.state, TeamSide::Home),
            plan_team(&self.state, TeamSide::Away),
        ];
        let decisions = decide_all(&self.state, &plans);
        for side in TeamSide::BOTH {
            self.state.teams[side.index()].phase = plans[side.index()].phase;
        }
        self.apply_actions(&decisions);
        self.move_players(&decisions);
        self.update_fatigue();
        self.resolve_duels();
        self.move_ball();
        self.resolve_boundaries();
        self.resolve_control();
        self.resolve_advantage();
        self.tick_cooldowns();
        self.state.tick += 1;
        self.plans = Some(plans);
        self.decisions = decisions;
        self.update_clock();
    }

    /// Has the final whistle gone?
    pub fn finished(&self) -> bool {
        self.state.period == Period::FullTime
    }

    /// Match clock in seconds.
    pub fn match_seconds(&self) -> f32 {
        self.state.match_seconds(self.clock.half_seconds)
    }

    fn update_clock(&mut self) {
        let length = self.clock.period_length(self.state.stoppage_secs);
        if self.state.period_seconds() < length {
            return;
        }
        match self.state.period {
            Period::FirstHalf => {
                self.event(MatchEvent::HalfTime);
                self.state.period = Period::SecondHalf;
                self.state.period_start_tick = self.state.tick;
                self.state.stoppage_secs = 0.0;
                self.state.restart = None;
                let team = self.state.first_kick_off.opponent();
                self.kick_off(team);
                self.consider_substitutions();
            }
            Period::SecondHalf => {
                self.event(MatchEvent::FullTime);
                let st = &mut self.state;
                st.period = Period::FullTime;
                st.ball.owner = None;
                st.ball.vel = Vec2::ZERO;
                st.ball.pass = None;
                st.ball.shot = None;
                st.restart = None;
                st.advantage = None;
            }
            Period::FullTime => {}
        }
    }

    /// Play until the final whistle (or `max_ticks`).
    pub fn run_to_full_time(&mut self, max_ticks: u32) {
        for _ in 0..max_ticks {
            if self.finished() {
                break;
            }
            self.step();
        }
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
        let acc = if lofted {
            p.attrs.long_passing
        } else {
            p.attrs.passing
        };
        let spread = (1.0 - acc) * c.distance * 0.07 + 0.15;
        let along = (c.target - p.pos).normalize_or_zero();
        let err = along.perp() * self.rng.range(-spread, spread)
            + along * self.rng.range(-spread, spread) * 0.5;
        let target = c.target + err;
        let dir = (target - p.pos).normalize_or_zero();
        let st = &mut self.state;
        st.ball.vel = dir * c.speed;
        st.ball.pos = p.pos + dir * 0.4;
        let offside_applies = !st.restart.is_some_and(|r| r.kind.exempt_from_offside());
        let receiver_offside = offside_applies && st.in_offside_position(c.receiver);
        st.ball.pass = Some(PassInFlight {
            from: by,
            to: c.receiver,
            origin: p.pos,
            target,
            kicked_tick: st.tick,
            lofted,
            receiver_offside,
        });
        st.ball.air_distance = if lofted {
            p.pos.distance(target) * 0.97
        } else {
            0.0
        };
        st.ball.shot = None;
        self.release(by);
        self.event(MatchEvent::Pass {
            from: by,
            to: c.receiver,
            kind: c.kind,
        });
    }

    fn do_shot(&mut self, by: PlayerId, s: crate::decision::ShotOption) {
        if self.state.ball.owner != Some(by) || self.state.player(by).action_cooldown > 0 {
            return;
        }
        let p = self.state.player(by).clone();
        let kind = match self.state.restart.map(|r| r.kind) {
            Some(RestartKind::Penalty) => ShotKind::Penalty,
            Some(RestartKind::FreeKick) => ShotKind::FreeKick,
            _ => ShotKind::OpenPlay,
        };
        let dist = p.pos.distance(s.target);
        let spread = match kind {
            ShotKind::Penalty => 0.2 + (1.0 - p.attrs.finishing) * 0.9,
            ShotKind::FreeKick => 0.4 + (1.0 - p.attrs.set_pieces) * dist * 0.05,
            ShotKind::OpenPlay => {
                let pressure = crate::eval::pressure_at(&self.state, p.team, p.pos);
                (1.0 - p.attrs.finishing) * dist * 0.06 + 0.25 + pressure * 1.5
            }
        };
        let target = s.target + Vec2::new(0.0, self.rng.range(-spread, spread));
        let dir = (target - p.pos).normalize_or_zero();
        let speed = SHOT_SPEED * (0.85 + 0.15 * p.attrs.kicking_power);
        let st = &mut self.state;
        st.ball.vel = dir * speed;
        st.ball.pos = p.pos + dir * 0.4;
        st.ball.pass = None;
        // A free kick is curled over the wall.
        st.ball.air_distance = if kind == ShotKind::FreeKick {
            10.0
        } else {
            0.0
        };
        st.ball.shot = Some(ShotInFlight {
            shooter: by,
            target,
            kicked_tick: st.tick,
            xg: s.xg,
            keeper_attempted: false,
            kind,
        });
        self.release(by);
        self.event(MatchEvent::Shot { by, xg: s.xg, kind });
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
        if self.state.ball.owner != Some(on)
            || t.tackle_cooldown > 0
            || t.sent_off
            || t.pos.distance(c.pos) > TACKLE_RANGE
        {
            return;
        }
        let challenge = Challenge::new(&t, &c);
        // Draw every random number up front so the stream does not depend on the outcome.
        let foul = self
            .rng
            .chance(rules::foul_probability(&challenge, &self.referee));
        let settling =
            c.action_cooldown > 0 || self.state.seconds_since(self.state.ball.owner_since) < 0.5;
        let p_win = rules::tackle_success(&t, &c, settling);
        let won = self.rng.chance(p_win);
        let keep = self.rng.chance(0.45);
        let angle = self.rng.range(-0.9, 0.9);
        let kick = self.rng.range(3.0, 7.0);
        let card_roll = self.rng.next_f32();
        self.state.players[by.index()].tackle_cooldown = (3.0 / dt) as u16;

        if foul {
            self.handle_foul(by, on, &challenge, !won, card_roll);
            return;
        }
        let st = &mut self.state;
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

    /// Referee's whistle: penalise a challenge by `by` on `on` right now
    /// (scenarios, tests and the sandbox use this to set up set pieces).
    /// `carrier_kept_ball` allows the referee to play advantage.
    pub fn whistle_foul(&mut self, by: PlayerId, on: PlayerId, carrier_kept_ball: bool) {
        let challenge = Challenge::new(self.state.player(by), self.state.player(on));
        let roll = self.rng.next_f32();
        self.handle_foul(by, on, &challenge, carrier_kept_ball, roll);
    }

    /// Show `player` a card directly (tests, sandbox).
    pub fn caution(&mut self, player: PlayerId, card: CardDecision) {
        self.apply_card(player, card);
    }

    /// A challenge was penalised. `carrier_kept_ball` tells whether the
    /// fouled player still has the ball (advantage is possible).
    fn handle_foul(
        &mut self,
        by: PlayerId,
        on: PlayerId,
        challenge: &Challenge,
        carrier_kept_ball: bool,
        card_roll: f32,
    ) {
        let spot = self.state.player(on).pos;
        let fouled = on.team();
        self.event(MatchEvent::Foul { by, on, spot });
        self.state.stoppage_secs += self.clock.stoppage_per_foul;
        let card = rules::card_for_foul(&self.state, by, on, challenge, &self.referee, card_roll);

        if rules::should_play_advantage(&self.state, on, carrier_kept_ball, &self.referee) {
            self.state.advantage = Some(Advantage {
                team: fouled,
                spot,
                tick: self.state.tick,
            });
            self.event(MatchEvent::Advantage { team: fouled });
        } else {
            let dt = self.state.dt;
            self.state.players[on.index()].stun = (0.8 / dt) as u16;
            self.award_free_kick(fouled, spot);
        }
        self.apply_card(by, card);
    }

    /// Free kick (or penalty, inside the box) to `team` for a foul at `spot`.
    fn award_free_kick(&mut self, team: TeamSide, spot: Vec2) {
        if rules::in_penalty_area(team.opponent(), spot) {
            self.event(MatchEvent::PenaltyAwarded { team });
            self.set_restart(
                RestartKind::Penalty,
                team,
                rules::penalty_spot(team.opponent()),
            );
        } else {
            self.set_restart(RestartKind::FreeKick, team, clamp_to_pitch(spot, 0.5));
        }
    }

    fn apply_card(&mut self, player: PlayerId, card: CardDecision) {
        let p = &self.state.players[player.index()];
        // No substitutions are modelled yet, so a goalkeeper cannot be
        // replaced: dismissals of keepers are downgraded to a caution.
        let card = if p.is_goalkeeper() && card == CardDecision::Red {
            CardDecision::Yellow
        } else {
            card
        };
        match card {
            CardDecision::None => return,
            CardDecision::Yellow => {
                self.state.players[player.index()].yellow_cards += 1;
                self.event(MatchEvent::YellowCard { player });
                let p = &self.state.players[player.index()];
                if p.yellow_cards >= 2 && !p.is_goalkeeper() {
                    self.event(MatchEvent::RedCard {
                        player,
                        second_yellow: true,
                    });
                    self.send_off(player);
                }
            }
            CardDecision::Red => {
                self.event(MatchEvent::RedCard {
                    player,
                    second_yellow: false,
                });
                self.send_off(player);
            }
        }
        self.state.stoppage_secs += self.clock.stoppage_per_card;
    }

    fn send_off(&mut self, player: PlayerId) {
        let p = &mut self.state.players[player.index()];
        p.sent_off = true;
        p.vel = Vec2::ZERO;
        // Off to the touchline, away from play.
        p.pos = Vec2::new(
            -6.0 + player.slot() as f32 * 1.2 * player.team().direction(),
            -(HALF_WIDTH + 4.0),
        );
        if self.state.ball.owner == Some(player) {
            self.state.ball.owner = None;
        }
    }

    /// Advantage: if the fouled team loses the ball within the window, bring
    /// play back for the free kick; once the window passes, it has accrued.
    fn resolve_advantage(&mut self) {
        let Some(a) = self.state.advantage else {
            return;
        };
        if self.state.restart.is_some() {
            self.state.advantage = None;
            return;
        }
        if self.state.seconds_since(a.tick) > self.referee.advantage_window {
            self.state.advantage = None;
        } else if self.state.possession != a.team {
            self.state.advantage = None;
            self.event(MatchEvent::AdvantageCalledBack { team: a.team });
            self.award_free_kick(a.team, a.spot);
        }
    }

    /// A carrier who runs into a defender standing in front of him has to get
    /// the ball past him: that contact is a duel whether or not the defender
    /// chose to dive in.
    fn resolve_duels(&mut self) {
        let Some(cid) = self.state.ball.owner else {
            return;
        };
        if self.state.restart.is_some() {
            return;
        }
        let c = self.state.player(cid).clone();
        let speed = c.vel.length();
        if speed < 2.0 {
            return;
        }
        let dir = c.vel / speed;
        let defender = self
            .state
            .on_pitch(c.team.opponent())
            .filter(|d| d.tackle_cooldown == 0 && d.stun == 0)
            .filter(|d| {
                let off = d.pos - c.pos;
                off.length() < 1.0 && off.normalize_or_zero().dot(dir) > 0.6
            })
            .map(|d| d.id)
            .min();
        if let Some(d) = defender {
            self.do_tackle(d, cid);
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
            if p.sent_off {
                continue;
            }
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
            p.pos = clamp_to_pitch(
                p.pos + p.vel * dt,
                if owner == Some(p.id) { 0.6 } else { -1.5 },
            );
            if p.vel.length() > 0.4 {
                p.facing = p.vel.normalize_or_zero();
            }
        }
    }

    /// Energy drains with running (faster for low-stamina players) and
    /// recovers slowly at walking pace; tired players lose speed.
    fn update_fatigue(&mut self) {
        let dt = self.state.dt;
        for p in &mut self.state.players {
            if p.sent_off {
                continue;
            }
            let frac = (p.vel.length() / p.base_top_speed).min(1.0);
            let drain = (0.00003 + 0.0007 * frac * frac * frac) * (1.4 - p.attrs.stamina);
            let recover = if frac < 0.25 { 0.0002 } else { 0.0 };
            p.energy = (p.energy - (drain - recover) * dt).clamp(0.0, 1.0);
            let factor = 0.8 + 0.2 * p.energy;
            p.attrs.top_speed = p.base_top_speed * factor;
            p.attrs.acceleration = p.base_acceleration * factor;
        }
    }

    // ------------------------------------------------------- substitutions --

    /// Bring on bench player `bench_index` for the player in `slot`.
    /// Returns `false` (and does nothing) if the substitution is not allowed.
    pub fn substitute(&mut self, team: TeamSide, slot: usize, bench_index: usize) -> bool {
        let t = team.index();
        let id = PlayerId::new(team, slot);
        if self.state.subs_used[t] >= MAX_SUBSTITUTIONS
            || bench_index >= self.squads[t].bench.len()
            || self.bench_used[t][bench_index]
            || self.state.player(id).sent_off
            || self.state.ball.owner == Some(id)
        {
            return false;
        }
        let incoming = self.squads[t].bench[bench_index].clone();
        let outgoing = std::mem::replace(&mut self.squads[t].players[slot], incoming.clone());
        self.squads[t].bench[bench_index] = outgoing;
        self.bench_used[t][bench_index] = true;
        self.state.subs_used[t] += 1;
        let old = self.state.player(id).clone();
        let role = old.role;
        let mut p = Player::new(id, role, incoming.attributes(), incoming.behaviour_in(role));
        p.pos = old.pos;
        p.facing = old.facing;
        self.state.players[id.index()] = p;
        self.event(MatchEvent::Substitution {
            team,
            slot: slot as u8,
            bench_index: bench_index as u8,
        });
        true
    }

    /// The manager's substitution logic, run at dead balls: from the 55th
    /// minute, replace the most tired outfield player (energy below 0.6) with
    /// the best available bench player for his position.
    fn consider_substitutions(&mut self) {
        if self.match_seconds() < 55.0 * 60.0 {
            return;
        }
        for team in TeamSide::BOTH {
            let t = team.index();
            if self.state.subs_used[t] >= MAX_SUBSTITUTIONS {
                continue;
            }
            let tired = self
                .state
                .on_pitch(team)
                .filter(|p| {
                    !p.is_goalkeeper() && p.energy < 0.6 && self.state.ball.owner != Some(p.id)
                })
                .min_by(|a, b| cmp_f32(a.energy, b.energy).then(a.id.cmp(&b.id)))
                .map(|p| (p.id.slot(), p.role));
            let Some((slot, role)) = tired else { continue };
            let line = |r: Role| match r {
                Role::Goalkeeper => 0,
                Role::CentreBack | Role::FullBack => 1,
                Role::DefensiveMid | Role::CentralMid | Role::AttackingMid => 2,
                Role::Winger | Role::Striker => 3,
            };
            let bench = &self.squads[t].bench;
            let pick = (0..bench.len())
                .filter(|&i| !self.bench_used[t][i] && bench[i].position != Role::Goalkeeper)
                .max_by_key(|&i| {
                    let fit = if bench[i].position == role {
                        2
                    } else if line(bench[i].position) == line(role) {
                        1
                    } else {
                        0
                    };
                    (
                        fit,
                        bench[i].ratings.overall_outfield(),
                        std::cmp::Reverse(i),
                    )
                });
            if let Some(i) = pick {
                self.substitute(team, slot, i);
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
        if self.state.ball.owner.is_some() || (b.x.abs() <= HALF_LENGTH && b.y.abs() <= HALF_WIDTH)
        {
            return;
        }
        let last = self
            .state
            .ball
            .last_touch
            .map(|p| p.team())
            .unwrap_or(TeamSide::Home);
        if b.x.abs() > HALF_LENGTH {
            // Which team defends this end?
            let defending = if b.x > 0.0 {
                TeamSide::Away
            } else {
                TeamSide::Home
            };
            let attacking = defending.opponent();
            // Interpolate the crossing point on the goal line.
            let v = self.state.ball.vel;
            let back = if v.x.abs() > 1e-4 {
                (b.x.abs() - HALF_LENGTH) / v.x.abs()
            } else {
                0.0
            };
            let cross_y = b.y - v.y * back;
            if cross_y.abs() < GOAL_HALF_WIDTH {
                let scorer = self.state.ball.last_touch;
                self.state.score[attacking.index()] += 1;
                self.state.stoppage_secs += self.clock.stoppage_per_goal;
                self.event(MatchEvent::Goal {
                    team: attacking,
                    scorer,
                });
                self.kick_off(defending);
                self.consider_substitutions();
                return;
            }
            let (kind, team, spot) = if last == defending {
                let sy = b.y.signum() * (HALF_WIDTH - 0.5);
                (
                    RestartKind::Corner,
                    attacking,
                    Vec2::new(b.x.signum() * (HALF_LENGTH - 0.5), sy),
                )
            } else {
                (
                    RestartKind::GoalKick,
                    defending,
                    Vec2::new(b.x.signum() * (HALF_LENGTH - 5.5), 0.0),
                )
            };
            self.set_restart(kind, team, spot);
        } else {
            let team = last.opponent();
            let spot = Vec2::new(
                b.x.clamp(-HALF_LENGTH + 1.0, HALF_LENGTH - 1.0),
                b.y.signum() * (HALF_WIDTH - 0.3),
            );
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
        st.advantage = None;
        if st.possession != team {
            st.possession = team;
            st.possession_since = st.tick;
        }
        self.event(MatchEvent::OutOfPlay {
            restart: kind,
            team,
        });
        if kind != RestartKind::Penalty {
            self.consider_substitutions();
        }
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
            if p.sent_off
                || p.stun > 0
                || (p.action_cooldown > 0 && self.state.ball.last_touch == Some(p.id))
            {
                continue;
            }
            if restart_team.is_some_and(|t| t != p.team) {
                continue;
            }
            let keeper_reach =
                if p.is_goalkeeper() && crate::pitch::in_own_box_local(p.team.to_local(p.pos)) {
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
            let priority = if pass.is_some_and(|ps| ps.to == p.id) {
                0
            } else {
                1
            };
            cands.push((priority, d, p.id));
        }
        cands.sort_by(|a, b| a.0.cmp(&b.0).then(cmp_f32(a.1, b.1)).then(a.2.cmp(&b.2)));

        for (_, _, id) in cands {
            let p = self.state.player(id).clone();
            // Goalkeeper facing a shot.
            if let Some(s) = shot.filter(|s| s.shooter.team() != p.team) {
                if p.is_goalkeeper() {
                    let save = match s.kind {
                        ShotKind::Penalty => 0.1 + 0.25 * p.attrs.goalkeeping,
                        _ => (0.15 + 0.75 * p.attrs.goalkeeping - 0.5 * s.xg).clamp(0.05, 0.95),
                    };
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
                    self.state.ball.vel =
                        (v.perp() * self.rng.range(-0.5, 0.5) - v * 0.3).clamp_length(10.0);
                    self.state.ball.last_touch = Some(id);
                    self.state.ball.shot = None;
                    return;
                }
                continue;
            }

            let intended = pass.is_some_and(|ps| ps.to == id);
            let opponent_of_passer = pass.is_some_and(|ps| ps.from.team() != p.team);
            // At the edge of his reach a player only gets a touch on the ball.
            let reach_factor = 1.0 - 0.6 * (p.pos.distance(ball_pos) / CONTROL_RADIUS).min(1.0);
            let p_control = if intended {
                (1.25 - speed / 28.0 + 0.3 * p.attrs.control).clamp(0.55, 0.99)
            } else if opponent_of_passer {
                ((0.95 - speed / 30.0 + 0.15 * p.attrs.tackling) * reach_factor).clamp(0.1, 0.9)
            } else {
                (1.15 - speed / 25.0 + 0.2 * p.attrs.control).clamp(0.3, 0.99)
            };
            if self.rng.chance(p_control) {
                if pass.is_some_and(|ps| ps.to == id && ps.receiver_offside) {
                    self.event(MatchEvent::Offside { player: id });
                    let spot = clamp_to_pitch(p.pos, 1.0);
                    self.set_restart(RestartKind::IndirectFreeKick, p.team.opponent(), spot);
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
                        self.event(MatchEvent::PassCompleted {
                            from: ps.from,
                            to: id,
                        });
                    } else if ps.from.team() != p.team {
                        self.event(MatchEvent::Interception {
                            by: id,
                            from: ps.from,
                        });
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

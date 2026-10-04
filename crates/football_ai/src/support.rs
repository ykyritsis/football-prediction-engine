//! Off-ball support in possession.
//!
//! While the team has the ball, players who are not the carrier are given a
//! [`SupportRun`] for this tick, in priority order:
//!
//! 1. **Rest defence** – designated players stay behind the ball.
//! 2. **Box runs** – with the ball wide in the final third, players attack the
//!    near post, far post, penalty spot and cut-back zone.
//! 3. **Run in behind** – forwards spring the offside line when the carrier
//!    has time to play the pass.
//! 4. **Overlap / underlap** – full-backs and midfielders go round or inside a
//!    wide carrier.
//! 5. **Show for the ball** – the nearest players move into free pockets with
//!    open passing lanes at a distance set by the team's support range.
//! 6. **Hold width** – wingers stretch the pitch.
//!
//! Everyone else keeps their formation-relative position.

use crate::math::{closest_point_on_segment, cmp_f32, Vec2};
use crate::phase::TeamPhase;
use crate::pitch::{clamp_to_pitch, TeamSide, HALF_LENGTH, HALF_WIDTH};
use crate::role::Role;
use crate::state::{MatchState, PlayerId, TEAM_SIZE};

/// Kind of off-ball run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupportRun {
    RestDefence,
    NearPostRun,
    FarPostRun,
    PenaltySpotRun,
    CutBackZone,
    RunInBehind,
    Overlap,
    Underlap,
    ShowForBall,
    HoldWidth,
}

impl SupportRun {
    pub fn name(self) -> &'static str {
        match self {
            SupportRun::RestDefence => "rest defence",
            SupportRun::NearPostRun => "near post",
            SupportRun::FarPostRun => "far post",
            SupportRun::PenaltySpotRun => "penalty spot",
            SupportRun::CutBackZone => "cut-back zone",
            SupportRun::RunInBehind => "run in behind",
            SupportRun::Overlap => "overlap",
            SupportRun::Underlap => "underlap",
            SupportRun::ShowForBall => "show for ball",
            SupportRun::HoldWidth => "hold width",
        }
    }

    /// Runs that should be made at full speed.
    pub fn is_sprint(self) -> bool {
        matches!(
            self,
            SupportRun::RunInBehind
                | SupportRun::Overlap
                | SupportRun::Underlap
                | SupportRun::NearPostRun
                | SupportRun::FarPostRun
                | SupportRun::PenaltySpotRun
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupportAssignment {
    pub player: PlayerId,
    pub run: SupportRun,
    /// World-space target.
    pub target: Vec2,
}

/// Where the ball effectively is for support purposes: the carrier, or the
/// target of a pass in flight to a team-mate.
fn ball_holder(state: &MatchState, side: TeamSide) -> Option<(PlayerId, Vec2)> {
    if let Some(c) = state.carrier() {
        return (c.team == side).then_some((c.id, c.pos));
    }
    state.ball.pass.filter(|p| p.to.team() == side).map(|p| (p.to, p.target))
}

/// Plan support runs for `side`. `shape` are its formation targets (world).
pub fn plan_support(state: &MatchState, side: TeamSide, phase: TeamPhase, shape: &[Vec2; TEAM_SIZE]) -> Vec<SupportAssignment> {
    if !phase.in_possession() || state.restart.is_some() {
        return Vec::new();
    }
    let Some((holder, ball_w)) = ball_holder(state, side) else { return Vec::new() };
    let tactics = &state.team(side).tactics;
    let ball = side.to_local(ball_w);
    let players = state.team_players(side);
    let mut taken = [false; TEAM_SIZE];
    taken[holder.slot()] = true;
    taken[0] = true; // goalkeeper never makes support runs
    let mut out = Vec::new();
    let local_shape: Vec<Vec2> = shape.iter().map(|&p| side.to_local(p)).collect();
    let push = |out: &mut Vec<SupportAssignment>, taken: &mut [bool; TEAM_SIZE], slot: usize, run, local: Vec2| {
        taken[slot] = true;
        out.push(SupportAssignment { player: players[slot].id, run, target: side.to_world(clamp_to_pitch(local, 1.0)) });
    };

    // 1. Rest defence.
    if ball.x > -25.0 {
        for (slot, p) in players.iter().enumerate() {
            if taken[slot] || !p.behaviour.rest_defence {
                continue;
            }
            let s = local_shape[slot];
            let behind = ball.x - if p.role == Role::DefensiveMid { 10.0 } else { 18.0 };
            push(&mut out, &mut taken, slot, SupportRun::RestDefence, Vec2::new(s.x.min(behind), s.y));
        }
    }

    let carrier_pressure = crate::eval::pressure_at(state, side, ball_w);
    let wide_final_third = ball.x > HALF_LENGTH - 32.0 && ball.y.abs() > 15.0;

    // 2. Box runs.
    if wide_final_third {
        let sign = ball.y.signum();
        let spots = [
            (SupportRun::NearPostRun, Vec2::new(HALF_LENGTH - 6.0, sign * 3.0)),
            (SupportRun::FarPostRun, Vec2::new(HALF_LENGTH - 8.0, -sign * 5.0)),
            (SupportRun::PenaltySpotRun, Vec2::new(HALF_LENGTH - 11.5, -sign * 1.0)),
            (SupportRun::CutBackZone, Vec2::new(HALF_LENGTH - 17.0, sign * 5.0)),
        ];
        let runners = 2 + (tactics.box_crowding * 2.0).round() as usize;
        for &(run, spot) in spots.iter().take(runners.min(spots.len())) {
            let best = (1..TEAM_SIZE)
                .filter(|&s| !taken[s] && players[s].behaviour.box_presence > 0.25)
                .map(|s| {
                    let p = &players[s];
                    let score = p.behaviour.box_presence - side.to_local(p.pos).distance(spot) / 60.0;
                    (score, s)
                })
                .max_by(|a, b| cmp_f32(a.0, b.0).then(b.1.cmp(&a.1)));
            if let Some((_, s)) = best {
                push(&mut out, &mut taken, s, run, spot);
            }
        }
    }

    // 3. Runs in behind.
    let offside = state.offside_line_local(side);
    let runnable = matches!(phase, TeamPhase::Progression | TeamPhase::FinalThird | TeamPhase::AttackingTransition);
    if runnable && carrier_pressure < 0.75 && offside < HALF_LENGTH - 10.0 {
        let counter = phase == TeamPhase::AttackingTransition;
        let max_runs = if counter && tactics.counter_attack > 0.7 { 2 } else { 1 };
        let mut cands: Vec<(f32, usize)> = (1..TEAM_SIZE)
            .filter(|&s| !taken[s] && players[s].behaviour.run_in_behind >= 0.35)
            .map(|s| {
                let p = &players[s];
                let pl = side.to_local(p.pos);
                // On the shoulder of the last defender is the best starting point.
                let shoulder = 1.0 - ((offside - pl.x).abs() / 25.0).min(1.0);
                let bonus = if counter { tactics.counter_attack * 0.3 } else { 0.0 };
                (p.behaviour.run_in_behind + 0.4 * shoulder + bonus, s)
            })
            .collect();
        cands.sort_by(|a, b| cmp_f32(b.0, a.0).then(a.1.cmp(&b.1)));
        // Time the run: hold the shoulder of the last defender until the
        // carrier is ready to release, then go.
        let ready = state.carrier().is_some_and(|c| c.team == side && c.action_cooldown <= 2);
        for &(_, s) in cands.iter().take(max_runs) {
            let y = local_shape[s].y * 0.75;
            let x = if ready { (offside + 10.0).min(HALF_LENGTH - 5.0) } else { offside - 1.0 };
            push(&mut out, &mut taken, s, SupportRun::RunInBehind, Vec2::new(x, y));
        }
    }

    // 4. Overlap / underlap around a wide carrier.
    let carrier_wide = ball.y.abs() > 12.0;
    if carrier_wide && matches!(phase, TeamPhase::Progression | TeamPhase::FinalThird) {
        let sign = ball.y.signum();
        for s in 1..TEAM_SIZE {
            let p = &players[s];
            if taken[s] || local_shape[s].y.signum() != sign {
                continue;
            }
            let pl = side.to_local(p.pos);
            let urge = p.behaviour.overlap * (0.6 + 0.4 * tactics.fluidity);
            if p.role == Role::FullBack && urge > 0.45 && pl.x < ball.x + 2.0 && ball.x < HALF_LENGTH - 8.0 {
                push(&mut out, &mut taken, s, SupportRun::Overlap, Vec2::new(ball.x + 9.0, sign * (HALF_WIDTH - 3.0)));
            } else if matches!(p.role, Role::CentralMid | Role::AttackingMid) && urge > 0.35 && pl.x < ball.x + 4.0 {
                push(&mut out, &mut taken, s, SupportRun::Underlap, Vec2::new(ball.x + 7.0, sign * 13.0));
            }
        }
    }

    // 5. Show for the ball.
    let shows = if carrier_pressure > 0.5 || tactics.support_closeness > 0.7 { 2 } else { 1 };
    let radius = 18.0 - 9.0 * tactics.support_closeness;
    let mut pockets: Vec<Vec2> = Vec::new();
    for _ in 0..shows {
        let nearest = (1..TEAM_SIZE)
            .filter(|&s| !taken[s])
            .min_by(|&a, &b| {
                cmp_f32(side.to_local(players[a].pos).distance(ball), side.to_local(players[b].pos).distance(ball))
                    .then(a.cmp(&b))
            });
        let Some(s) = nearest else { break };
        let pocket = best_pocket(state, side, ball, radius, local_shape[s], &pockets);
        pockets.push(pocket);
        push(&mut out, &mut taken, s, SupportRun::ShowForBall, pocket);
    }

    // 6. Hold width.
    for s in 1..TEAM_SIZE {
        let p = &players[s];
        if taken[s] || p.role != Role::Winger {
            continue;
        }
        let sign = if local_shape[s].y == 0.0 { 1.0 } else { local_shape[s].y.signum() };
        push(&mut out, &mut taken, s, SupportRun::HoldWidth, Vec2::new(local_shape[s].x, sign * (HALF_WIDTH - 3.0)));
    }

    out.sort_by_key(|a| a.player);
    out
}

/// Sixteen unit directions from straight back round to straight back,
/// precomputed so the search does not depend on platform trigonometry.
const POCKET_DIRS: [(f32, f32); 16] = [
    (-1.0000000, 0.0000000),
    (-0.9238795, -0.3826834),
    (-0.7071068, -0.7071068),
    (-0.3826834, -0.9238795),
    (0.0000000, -1.0000000),
    (0.3826834, -0.9238795),
    (0.7071068, -0.7071068),
    (0.9238795, -0.3826834),
    (1.0000000, 0.0000000),
    (0.9238795, 0.3826834),
    (0.7071068, 0.7071068),
    (0.3826834, 0.9238795),
    (0.0000000, 1.0000000),
    (-0.3826834, 0.9238795),
    (-0.7071068, 0.7071068),
    (-0.9238795, 0.3826834),
];

/// Find a free pocket around the ball (team-local) at roughly `radius`.
fn best_pocket(state: &MatchState, side: TeamSide, ball: Vec2, radius: f32, home: Vec2, avoid: &[Vec2]) -> Vec2 {
    let opps: Vec<Vec2> = state.team_players(side.opponent()).iter().map(|p| side.to_local(p.pos)).collect();
    let mut best = (f32::NEG_INFINITY, home);
    for &(dx, dy) in &POCKET_DIRS {
        let dir = Vec2::new(dx, dy);
        let p = clamp_to_pitch(ball + dir * radius, 2.0);
        if avoid.iter().any(|q| q.distance(p) < 7.0) {
            continue;
        }
        let space = opps.iter().map(|o| o.distance(p)).fold(f32::INFINITY, f32::min).min(10.0);
        let lane = opps
            .iter()
            .map(|o| o.distance(closest_point_on_segment(ball, p, *o).0))
            .fold(f32::INFINITY, f32::min)
            .min(6.0);
        let forward = (p.x - ball.x) / radius;
        let score = space + 0.9 * lane - 0.12 * p.distance(home) + 1.5 * forward;
        if score > best.0 {
            best = (score, p);
        }
    }
    best.1
}

//! Pressing triggers and press assignment.
//!
//! Out of possession, a team decides each tick whether to *press* the ball
//! carrier (close down at full speed and try to win the ball), *contain* him
//! (one player jockeys goal-side while the block holds shape) or do nothing
//! (loose ball, dead ball).
//!
//! The decision comes from a set of classic pressing triggers, each with a
//! weight. Their sum, scaled by the team's press aggression and the current
//! phase's press zone, is the press *intensity*. Above a threshold the nearest
//! willing player presses; at higher intensity a second player cuts off the
//! carrier's most dangerous passing lane (cover shadow).

use crate::math::{cmp_f32, Vec2};
use crate::phase::TeamPhase;
use crate::pitch::{TeamSide, HALF_LENGTH, HALF_WIDTH};
use crate::state::{MatchState, PlayerId};

/// A pressing trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PressTrigger {
    /// The ball has just been played backwards: the opponent is retreating.
    BackwardPass,
    /// The carrier has only just received and has not settled the ball.
    ReceivingTouch,
    /// The carrier is facing his own goal.
    FacingOwnGoal,
    /// The ball is near the touchline: the line acts as an extra defender.
    NearTouchline,
    /// No team-mate is close enough to support the carrier.
    IsolatedCarrier,
    /// The ball is in the zone this phase presses in.
    InPressZone,
    /// Possession was just lost: counter-press.
    CounterPress,
    /// A goalkeeper or centre-back has the ball under no pressure in build-up.
    SlowBuildUp,
    /// The ball is close to our goal: engage whatever the phase.
    DangerZone,
}

impl PressTrigger {
    pub const ALL: [PressTrigger; 9] = [
        PressTrigger::BackwardPass,
        PressTrigger::ReceivingTouch,
        PressTrigger::FacingOwnGoal,
        PressTrigger::NearTouchline,
        PressTrigger::IsolatedCarrier,
        PressTrigger::InPressZone,
        PressTrigger::CounterPress,
        PressTrigger::SlowBuildUp,
        PressTrigger::DangerZone,
    ];

    pub fn weight(self) -> f32 {
        match self {
            PressTrigger::BackwardPass => 0.3,
            PressTrigger::ReceivingTouch => 0.25,
            PressTrigger::FacingOwnGoal => 0.25,
            PressTrigger::NearTouchline => 0.2,
            PressTrigger::IsolatedCarrier => 0.2,
            PressTrigger::InPressZone => 0.35,
            PressTrigger::CounterPress => 0.0, // scaled by tactics.counter_press
            PressTrigger::SlowBuildUp => 0.15,
            PressTrigger::DangerZone => 0.4,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PressTrigger::BackwardPass => "backward pass",
            PressTrigger::ReceivingTouch => "receiving touch",
            PressTrigger::FacingOwnGoal => "facing own goal",
            PressTrigger::NearTouchline => "near touchline",
            PressTrigger::IsolatedCarrier => "isolated carrier",
            PressTrigger::InPressZone => "in press zone",
            PressTrigger::CounterPress => "counter-press",
            PressTrigger::SlowBuildUp => "slow build-up",
            PressTrigger::DangerZone => "danger zone",
        }
    }
}

/// Bit set of triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct TriggerSet(u16);

impl TriggerSet {
    pub fn insert(&mut self, t: PressTrigger) {
        self.0 |= 1 << (t as u8);
    }
    pub fn contains(&self, t: PressTrigger) -> bool {
        self.0 & (1 << (t as u8)) != 0
    }
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
    pub fn iter(&self) -> impl Iterator<Item = PressTrigger> + '_ {
        PressTrigger::ALL
            .iter()
            .copied()
            .filter(move |&t| self.contains(t))
    }
}

/// What the defending team does about the ball carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PressMode {
    /// Nobody engages the ball (loose/dead ball or own possession).
    None,
    /// One player engages goal-side without diving in.
    Contain,
    /// Close down aggressively and try to win the ball.
    Press,
}

/// Team-level pressing plan for one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressPlan {
    pub mode: PressMode,
    pub triggers: TriggerSet,
    pub intensity: f32,
    /// Player engaging the carrier.
    pub primary: Option<PlayerId>,
    /// Player blocking a passing lane.
    pub cover: Option<PlayerId>,
    /// The opponent whose lane `cover` blocks.
    pub cover_target: Option<PlayerId>,
    /// Extra players swarming the ball during a counter-press.
    pub swarm: [Option<PlayerId>; 2],
    /// Near goal: a second defender who blocks the line from ball to goal.
    pub blocker: Option<PlayerId>,
}

impl PressPlan {
    pub const NONE: PressPlan = PressPlan {
        mode: PressMode::None,
        triggers: TriggerSet(0),
        intensity: 0.0,
        primary: None,
        cover: None,
        cover_target: None,
        swarm: [None, None],
        blocker: None,
    };

    pub fn involves(&self, id: PlayerId) -> bool {
        self.primary == Some(id)
            || self.cover == Some(id)
            || self.blocker == Some(id)
            || self.swarm.contains(&Some(id))
    }
}

/// Intensity at or above which the team presses instead of containing.
pub const PRESS_THRESHOLD: f32 = 0.5;
/// Intensity at or above which a second player covers a passing lane.
pub const COVER_THRESHOLD: f32 = 0.8;

/// Evaluate pressing triggers for `defending` against the current carrier.
pub fn evaluate_triggers(
    state: &MatchState,
    defending: TeamSide,
    phase: TeamPhase,
) -> (TriggerSet, f32) {
    let mut set = TriggerSet::default();
    let Some(carrier) = state.carrier() else {
        return (set, 0.0);
    };
    if carrier.team == defending || state.restart.is_some() {
        return (set, 0.0);
    }
    let attacking = carrier.team;
    let tactics = &state.team(defending).tactics;

    if let Some(pass) = state.last_completed_pass {
        if pass.team == attacking
            && pass.to == carrier.id
            && state.seconds_since(pass.completed_tick) < 1.5
            && attacking.to_local(pass.received_at).x < attacking.to_local(pass.origin).x - 3.0
        {
            set.insert(PressTrigger::BackwardPass);
        }
    }
    if state.seconds_since(state.ball.owner_since) < 0.6 {
        set.insert(PressTrigger::ReceivingTouch);
    }
    if attacking.to_local(carrier.facing).x < -0.3 {
        set.insert(PressTrigger::FacingOwnGoal);
    }
    if carrier.pos.y.abs() > HALF_WIDTH - 8.0 {
        set.insert(PressTrigger::NearTouchline);
    }
    let support = state
        .on_pitch(attacking)
        .filter(|p| p.id != carrier.id && p.pos.distance(carrier.pos) < 12.0)
        .count();
    if support == 0 {
        set.insert(PressTrigger::IsolatedCarrier);
    }
    // Press zone: where (in the defending team's frame) this phase engages.
    let bx = defending.to_local(state.ball.pos).x;
    let in_zone = match phase {
        TeamPhase::HighPress => true,
        TeamPhase::MidBlock => bx > -25.0,
        TeamPhase::LowBlock => bx < -HALF_LENGTH + 22.0,
        TeamPhase::DefensiveTransition => true,
        _ => false,
    };
    if in_zone {
        set.insert(PressTrigger::InPressZone);
    }
    if bx < -HALF_LENGTH + 25.0 {
        set.insert(PressTrigger::DangerZone);
    }
    let mut counter = 0.0;
    if phase == TeamPhase::DefensiveTransition && tactics.counter_press > 0.0 {
        set.insert(PressTrigger::CounterPress);
        counter = tactics.counter_press * 0.8;
    }
    if (carrier.is_goalkeeper() || carrier.role == crate::role::Role::CentreBack)
        && phase == TeamPhase::HighPress
    {
        set.insert(PressTrigger::SlowBuildUp);
    }

    let sum: f32 = set.iter().map(|t| t.weight()).sum::<f32>() + counter;
    // Height of the team's press scales how readily triggers fire away from goal.
    let height_factor = match phase {
        TeamPhase::HighPress => 0.6 + 0.6 * tactics.press_height,
        TeamPhase::LowBlock => 1.0,
        _ => 0.8 + 0.3 * tactics.press_height,
    };
    (set, sum * tactics.press_aggression * height_factor)
}

/// Build the pressing plan. `excluded` players (e.g. the goalkeeper) are never selected.
pub fn plan_press(state: &MatchState, defending: TeamSide, phase: TeamPhase) -> PressPlan {
    let Some(carrier) = state.carrier() else {
        return PressPlan::NONE;
    };
    if carrier.team == defending || state.restart.is_some() {
        return PressPlan::NONE;
    }
    let (triggers, intensity) = evaluate_triggers(state, defending, phase);
    let mode = if intensity >= PRESS_THRESHOLD {
        PressMode::Press
    } else {
        PressMode::Contain
    };

    // Rank candidates by time to reach the carrier, discounted by willingness.
    let mut ranked: Vec<(f32, PlayerId)> = state
        .on_pitch(defending)
        .filter(|p| !p.is_goalkeeper() && p.stun == 0)
        .map(|p| {
            let t = p.pos.distance(carrier.pos) / p.attrs.top_speed;
            // Tired players are less keen to press.
            let reluctance =
                1.0 + (1.0 - p.behaviour.press_willingness) * 1.5 + (1.0 - p.energy) * 1.2;
            (t * reluctance, p.id)
        })
        .collect();
    ranked.sort_by(|a, b| cmp_f32(a.0, b.0).then(a.1.cmp(&b.1)));

    let primary = ranked.first().map(|r| r.1);
    let mut plan = PressPlan {
        mode,
        triggers,
        intensity,
        primary,
        ..PressPlan::NONE
    };

    if mode == PressMode::Press && intensity >= COVER_THRESHOLD && ranked.len() > 1 {
        // Block the lane to the carrier's most valuable short option.
        let target = state
            .on_pitch(carrier.team)
            .filter(|p| {
                p.id != carrier.id && !p.is_goalkeeper() && p.pos.distance(carrier.pos) < 28.0
            })
            .max_by(|a, b| {
                let va = crate::pitch::zone_value_local(carrier.team.to_local(a.pos));
                let vb = crate::pitch::zone_value_local(carrier.team.to_local(b.pos));
                cmp_f32(va, vb).then(b.id.cmp(&a.id))
            })
            .map(|p| p.id);
        if let Some(t) = target {
            // The cover player is the one (other than the primary) closest to that lane.
            let lane_mid = carrier.pos.lerp(state.player(t).pos, 0.4);
            let cover = state
                .on_pitch(defending)
                .filter(|p| !p.is_goalkeeper() && Some(p.id) != primary && p.stun == 0)
                .min_by(|a, b| {
                    cmp_f32(a.pos.distance(lane_mid), b.pos.distance(lane_mid))
                        .then(a.id.cmp(&b.id))
                })
                .map(|p| p.id);
            plan.cover = cover;
            plan.cover_target = Some(t);
        }
    }

    // Counter-press swarm: extra bodies close the ball down immediately.
    // Not in the own third, though: there the shape protects the box.
    let carrier_local = defending.to_local(carrier.pos);
    if phase == TeamPhase::DefensiveTransition
        && mode == PressMode::Press
        && carrier_local.x > -HALF_LENGTH / 3.0
    {
        let tactics = &state.team(defending).tactics;
        let extra = if tactics.counter_press >= 0.85 {
            2
        } else if tactics.counter_press >= 0.6 {
            1
        } else {
            0
        };
        let mut n = 0;
        for &(_, id) in ranked.iter().skip(1) {
            if n >= extra {
                break;
            }
            if Some(id) == plan.cover || state.player(id).pos.distance(carrier.pos) > 15.0 {
                continue;
            }
            plan.swarm[n] = Some(id);
            n += 1;
        }
    }
    // Double up on an elite dribbler in the final two thirds: nobody lets a
    // Messi go one against one.
    if plan.swarm[0].is_none()
        && carrier.attrs.dribbling >= ELITE_DRIBBLER
        && carrier_local.x < HALF_LENGTH / 3.0
    {
        plan.swarm[0] = ranked.iter().skip(1).map(|&(_, id)| id).find(|&id| {
            Some(id) != plan.cover && state.player(id).pos.distance(carrier.pos) < 15.0
        });
    }
    // Danger zone: double up, the second man protecting the goal.
    // Only when the carrier can actually shoot: a blocker pulled out to a wide
    // carrier leaves the centre of the box empty.
    if triggers.contains(PressTrigger::DangerZone) && carrier_local.y.abs() < 14.0 {
        let spot = blocking_position(defending, carrier.pos);
        plan.blocker = state
            .on_pitch(defending)
            .filter(|p| !p.is_goalkeeper() && p.stun == 0 && !plan.involves(p.id))
            .min_by(|a, b| {
                cmp_f32(a.pos.distance(spot), b.pos.distance(spot)).then(a.id.cmp(&b.id))
            })
            .map(|p| p.id);
    }
    plan
}

/// Dribbling (0..1) at which a carrier is double-marked.
pub const ELITE_DRIBBLER: f32 = 0.85;

/// Where a shot-blocking defender stands: on the line from the ball to the
/// centre of the goal, a couple of metres from the carrier.
pub fn blocking_position(defending: TeamSide, carrier_pos: Vec2) -> Vec2 {
    let goal = defending.own_goal();
    carrier_pos + (goal - carrier_pos).normalize_or_zero() * 2.2
}

/// Where the containing player stands: goal-side of the carrier, angled to
/// show him wide or inside according to the team's containment instruction.
pub fn contain_position(state: &MatchState, defending: TeamSide, carrier_pos: Vec2) -> Vec2 {
    let goal = defending.own_goal();
    let to_goal = (goal - carrier_pos).normalize_or_zero();
    let show_wide = state.team(defending).tactics.show_wide;
    // Lateral offset towards the centre forces the carrier wide, and vice versa.
    let centre_dir = Vec2::new(0.0, -carrier_pos.y.signum());
    let lateral = centre_dir * ((show_wide - 0.5) * 2.0);
    carrier_pos + (to_goal * 3.0 + lateral * 1.2)
}

/// Cover-shadow position: on the lane from carrier to `target`, nearer the carrier.
pub fn cover_position(carrier_pos: Vec2, target_pos: Vec2) -> Vec2 {
    carrier_pos.lerp(target_pos, 0.4)
}

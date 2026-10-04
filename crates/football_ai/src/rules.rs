//! Laws of the game: fouls, advantage, cards, free kicks, penalties, time.
//!
//! The functions here are pure and take explicit inputs so the referee's
//! judgement can be unit-tested; [`crate::sim`] calls them and applies the
//! outcome with its RNG. A [`Referee`] carries the officiating style, so
//! referees can have identities just like teams and players.

use crate::math::Vec2;
use crate::pitch::{
    TeamSide, GOAL_HALF_WIDTH, HALF_LENGTH, PENALTY_AREA_DEPTH, PENALTY_AREA_HALF_WIDTH,
};
use crate::state::{MatchState, Player, PlayerId};

/// Officiating style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Referee {
    /// Multiplier on the chance that a contact is given as a foul.
    pub foul_strictness: f32,
    /// Multiplier on the chance that a foul is punished with a card.
    pub card_strictness: f32,
    /// Plays advantage when the fouled team keeps the ball.
    pub plays_advantage: bool,
    /// Seconds the referee waits to see whether advantage accrues.
    pub advantage_window: f32,
}

impl Default for Referee {
    fn default() -> Self {
        Self {
            foul_strictness: 1.0,
            card_strictness: 1.0,
            plays_advantage: true,
            advantage_window: 3.0,
        }
    }
}

impl Referee {
    /// A referee who lets the game flow.
    pub fn lenient() -> Self {
        Self {
            foul_strictness: 0.7,
            card_strictness: 0.6,
            ..Self::default()
        }
    }

    /// A referee who blows for everything.
    pub fn strict() -> Self {
        Self {
            foul_strictness: 1.4,
            card_strictness: 1.5,
            plays_advantage: false,
            ..Self::default()
        }
    }
}

/// Match length configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchClock {
    /// Length of each half in seconds (45 minutes by default).
    pub half_seconds: f32,
    /// Stoppage time added per goal (s).
    pub stoppage_per_goal: f32,
    /// Stoppage time added per card (s).
    pub stoppage_per_card: f32,
    /// Stoppage time added per foul (s).
    pub stoppage_per_foul: f32,
}

impl Default for MatchClock {
    fn default() -> Self {
        Self {
            half_seconds: 45.0 * 60.0,
            stoppage_per_goal: 40.0,
            stoppage_per_card: 25.0,
            stoppage_per_foul: 8.0,
        }
    }
}

impl MatchClock {
    /// Seconds the current period lasts including stoppage time, rounded up
    /// to whole minutes as a fourth official would show them (minimum one).
    pub fn period_length(&self, stoppage_secs: f32) -> f32 {
        let added = (stoppage_secs / 60.0).ceil().max(1.0) * 60.0;
        self.half_seconds + added
    }
}

/// The geometry of a challenge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Challenge {
    /// Cosine of the angle between the carrier's facing and the direction
    /// from the carrier to the tackler: `1` = from the front, `-1` = from behind.
    pub frontness: f32,
    /// Tackler's tackling skill `0..1`.
    pub tackling: f32,
    /// Tackler's aggression `0..1` (press willingness, destroyer traits).
    pub aggression: f32,
    /// Carrier's dribbling `0..1`: skilful carriers draw more fouls.
    pub carrier_dribbling: f32,
    /// The tackler is already on a yellow card and stays on his feet.
    pub booked: bool,
    /// The challenge is inside the tackler's own penalty area.
    pub in_own_box: bool,
}

impl Challenge {
    pub fn new(tackler: &Player, carrier: &Player) -> Self {
        let to_tackler = (tackler.pos - carrier.pos).normalize_or_zero();
        Challenge {
            frontness: carrier.facing.dot(to_tackler),
            tackling: tackler.attrs.tackling,
            aggression: tackler
                .behaviour
                .press_willingness
                .max(tackler.behaviour.marking_tightness),
            carrier_dribbling: carrier.attrs.dribbling,
            booked: tackler.yellow_cards > 0,
            in_own_box: in_penalty_area(tackler.team, carrier.pos),
        }
    }

    /// From behind: the tackler cannot see or reach the ball cleanly.
    pub fn from_behind(&self) -> bool {
        self.frontness < -0.35
    }
}

/// Probability that a clean challenge wins the ball. A carrier who is still
/// settling the ball or has his back to the tackler is easier to dispossess.
pub fn tackle_success(tackler: &Player, carrier: &Player, settling: bool) -> f32 {
    let to_tackler = (tackler.pos - carrier.pos).normalize_or_zero();
    let back_turned = carrier.facing.dot(to_tackler) < -0.2;
    // Keepers smother with their hands: goalkeeping, not tackling, decides it.
    let skill = if tackler.is_goalkeeper() {
        0.35 + 0.6 * tackler.attrs.goalkeeping
    } else {
        tackler.attrs.tackling
    };
    let mut p = 0.22 + 0.5 * (skill - 0.9 * carrier.attrs.dribbling);
    if settling {
        p += 0.08;
    }
    if back_turned {
        p += 0.05;
    }
    p.clamp(0.12, 0.65)
}

/// Minimum expected success at which a player commits to a tackle: aggressive
/// players dive in sooner, cautious ones jockey.
pub fn tackle_commit_threshold(tackler: &Player) -> f32 {
    let aggression = tackler
        .behaviour
        .press_willingness
        .max(tackler.behaviour.marking_tightness);
    let booked = if tackler.yellow_cards > 0 { 0.08 } else { 0.0 };
    0.42 - 0.12 * aggression + booked
}

/// Probability that a challenge is penalised as a foul.
pub fn foul_probability(c: &Challenge, referee: &Referee) -> f32 {
    // Calibrated so a typical match has ~20–30 fouls from several hundred challenges.
    let base =
        0.002 + 0.009 * (1.0 - c.tackling) + 0.004 * c.aggression + 0.004 * c.carrier_dribbling;
    let angle = if c.from_behind() {
        0.025
    } else if c.frontness < 0.2 {
        0.006
    } else {
        0.0
    };
    let mut p = base + angle;
    if c.booked {
        p *= 0.4;
    }
    if c.in_own_box {
        // Defenders stay on their feet in the box: a foul there costs a penalty.
        p *= 0.05;
    }
    (p * referee.foul_strictness).clamp(0.0, 0.9)
}

/// Card decision for a foul.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardDecision {
    None,
    Yellow,
    Red,
}

/// Is `pos` inside the penalty area that `defending` protects?
pub fn in_penalty_area(defending: TeamSide, pos: Vec2) -> bool {
    let l = defending.to_local(pos);
    l.x <= -HALF_LENGTH + PENALTY_AREA_DEPTH
        && l.x >= -HALF_LENGTH
        && l.y.abs() <= PENALTY_AREA_HALF_WIDTH
}

/// Penalty spot in front of the goal `defending` protects.
pub fn penalty_spot(defending: TeamSide) -> Vec2 {
    defending.to_world(Vec2::new(-HALF_LENGTH + 11.0, 0.0))
}

/// Denying an obvious goal-scoring opportunity: the carrier was heading for
/// goal in a central area within 25 m with at most the goalkeeper (besides
/// the fouler) between him and the goal.
pub fn denies_goal_scoring_opportunity(
    state: &MatchState,
    fouler: PlayerId,
    victim: PlayerId,
) -> bool {
    let v = state.player(victim);
    let attacking = v.team;
    let local = attacking.to_local(v.pos);
    let goal = Vec2::new(HALF_LENGTH, 0.0);
    let heading_for_goal = attacking.to_local(v.vel).x > 2.0;
    if local.distance(goal) > 25.0 || local.y.abs() > 16.0 || !heading_for_goal {
        return false;
    }
    let goal_side = state
        .on_pitch(attacking.opponent())
        .filter(|p| p.id != fouler && !p.is_goalkeeper())
        .filter(|p| {
            let pl = attacking.to_local(p.pos);
            pl.x > local.x && pl.y.abs() < (GOAL_HALF_WIDTH + 12.0)
        })
        .count();
    goal_side == 0
}

/// How the referee punishes a foul. `roll` is a uniform `[0, 1)` draw.
pub fn card_for_foul(
    state: &MatchState,
    fouler: PlayerId,
    victim: PlayerId,
    challenge: &Challenge,
    referee: &Referee,
    roll: f32,
) -> CardDecision {
    let in_box = in_penalty_area(state.player(fouler).team, state.player(victim).pos);
    if denies_goal_scoring_opportunity(state, fouler, victim) {
        // In the box, a penalty already restores the chance: caution only
        // (the IFAB "double punishment" change from 2016).
        return if in_box {
            CardDecision::Yellow
        } else {
            CardDecision::Red
        };
    }
    let mut yellow = 0.02 + 0.05 * challenge.aggression;
    if challenge.from_behind() {
        yellow += 0.06;
    }
    // Tactical foul stopping a promising attack in the opponent's half.
    let v = state.player(victim);
    if v.team.to_local(v.pos).x > 0.0 && v.vel.length() > 4.0 {
        yellow += 0.06;
    }
    yellow *= referee.card_strictness;
    let red = if challenge.from_behind() {
        0.01 * referee.card_strictness
    } else {
        0.002 * referee.card_strictness
    };
    if roll < red {
        CardDecision::Red
    } else if roll < red + yellow.min(0.9) {
        CardDecision::Yellow
    } else {
        CardDecision::None
    }
}

/// Should the referee play advantage instead of stopping play?
pub fn should_play_advantage(
    state: &MatchState,
    victim: PlayerId,
    carrier_kept_ball: bool,
    referee: &Referee,
) -> bool {
    if !referee.plays_advantage || !carrier_kept_ball {
        return false;
    }
    let v = state.player(victim);
    // Only worth it going forward with some space.
    v.team.to_local(v.pos).x > -10.0 && state.nearest_distance(v.team.opponent(), v.pos) > 1.5
}

/// Where players stand for a free kick defending team's wall: `n` slots on
/// the line from the ball to the goal, 9.15 m from the ball.
pub fn wall_positions(spot: Vec2, defending: TeamSide, n: usize) -> Vec<Vec2> {
    let goal = defending.own_goal();
    let dir = (goal - spot).normalize_or_zero();
    let centre = spot + dir * 9.4;
    let across = dir.perp();
    (0..n)
        .map(|i| centre + across * ((i as f32 - (n as f32 - 1.0) / 2.0) * 0.7))
        .collect()
}

/// Size of the wall for a free kick at `spot` (0 when too far to shoot).
pub fn wall_size(spot: Vec2, defending: TeamSide) -> usize {
    let l = defending.to_local(spot);
    let d = l.distance(Vec2::new(-HALF_LENGTH, 0.0));
    if d > 32.0 {
        0
    } else if l.y.abs() > 20.0 {
        2
    } else if d > 25.0 {
        3
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(frontness: f32, tackling: f32) -> Challenge {
        Challenge {
            frontness,
            tackling,
            aggression: 0.5,
            carrier_dribbling: 0.6,
            booked: false,
            in_own_box: false,
        }
    }

    #[test]
    fn tackles_from_behind_are_fouls_more_often() {
        let r = Referee::default();
        assert!(foul_probability(&ch(-0.9, 0.7), &r) > foul_probability(&ch(0.9, 0.7), &r) * 2.0);
    }

    #[test]
    fn good_tacklers_foul_less() {
        let r = Referee::default();
        assert!(foul_probability(&ch(0.5, 0.9), &r) < foul_probability(&ch(0.5, 0.3), &r));
    }

    #[test]
    fn booked_players_and_box_challenges_are_more_careful() {
        let r = Referee::default();
        let c = ch(0.0, 0.6);
        assert!(foul_probability(&Challenge { booked: true, ..c }, &r) < foul_probability(&c, &r));
        assert!(
            foul_probability(
                &Challenge {
                    in_own_box: true,
                    ..c
                },
                &r
            ) < foul_probability(&c, &r)
        );
    }

    #[test]
    fn strict_referees_give_more() {
        let c = ch(0.0, 0.6);
        assert!(
            foul_probability(&c, &Referee::strict()) > foul_probability(&c, &Referee::lenient())
        );
    }

    #[test]
    fn penalty_area_geometry() {
        // Home defends -x.
        assert!(in_penalty_area(TeamSide::Home, Vec2::new(-45.0, 5.0)));
        assert!(!in_penalty_area(TeamSide::Home, Vec2::new(-30.0, 5.0)));
        assert!(!in_penalty_area(TeamSide::Home, Vec2::new(45.0, 5.0)));
        assert!(in_penalty_area(TeamSide::Away, Vec2::new(45.0, -5.0)));
        assert_eq!(
            penalty_spot(TeamSide::Home),
            Vec2::new(-HALF_LENGTH + 11.0, 0.0)
        );
    }

    #[test]
    fn wall_stands_between_ball_and_goal() {
        let spot = Vec2::new(-30.0, 5.0);
        let wall = wall_positions(spot, TeamSide::Home, 4);
        assert_eq!(wall.len(), 4);
        for w in wall {
            assert!((w.distance(spot) - 9.4).abs() < 1.5);
            assert!(w.x < spot.x);
        }
        assert_eq!(wall_size(Vec2::new(10.0, 0.0), TeamSide::Home), 0);
        assert_eq!(wall_size(Vec2::new(-32.0, 0.0), TeamSide::Home), 4);
    }

    #[test]
    fn stoppage_time_rounds_up_to_minutes() {
        let c = MatchClock::default();
        assert_eq!(c.period_length(0.0), c.half_seconds + 60.0);
        assert_eq!(c.period_length(61.0), c.half_seconds + 120.0);
    }
}

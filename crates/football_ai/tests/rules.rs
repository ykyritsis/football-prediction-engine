//! Laws of the game: time, fouls, set pieces, cards.

use football_ai::decision::Activity;
use football_ai::identity::presets::{self, Squad};
use football_ai::math::Vec2;
use football_ai::pitch::TeamSide;
use football_ai::plan::plan_team;
use football_ai::role::Role;
use football_ai::rules::{in_penalty_area, penalty_spot, CardDecision, Referee};
use football_ai::scenario::Preset;
use football_ai::state::{Period, RestartKind, ShotKind};
use football_ai::{MatchEvent, PlayerId, SimConfig, Simulation};

const HOME: TeamSide = TeamSide::Home;
const AWAY: TeamSide = TeamSide::Away;

fn count(sim: &Simulation, f: impl Fn(&MatchEvent) -> bool) -> usize {
    sim.events.iter().filter(|(_, e)| f(e)).count()
}

#[test]
fn match_has_two_halves_and_ends() {
    let mut sim = Simulation::new(SimConfig::new(
        4,
        Squad::generated(presets::wing_play(), 80),
        Squad::generated(presets::counter_attack(), 80),
    ));
    sim.run_to_full_time(30 * 60 * 120);
    assert!(sim.finished());
    assert_eq!(sim.state.period, Period::FullTime);
    let ht = sim
        .events
        .iter()
        .find(|(_, e)| *e == MatchEvent::HalfTime)
        .map(|e| e.0)
        .unwrap();
    let ft = sim
        .events
        .iter()
        .find(|(_, e)| *e == MatchEvent::FullTime)
        .map(|e| e.0)
        .unwrap();
    let dt = sim.state.dt;
    // Each half lasts 45 minutes plus at least a minute of stoppage time.
    assert!(
        ht as f32 * dt >= 46.0 * 60.0 - 1.0,
        "first half {}",
        ht as f32 * dt / 60.0
    );
    assert!((ft - ht) as f32 * dt >= 46.0 * 60.0 - 1.0);
    // After the whistle nothing moves.
    let snapshot = sim.state.clone();
    sim.run(100);
    assert_eq!(snapshot, sim.state);
}

#[test]
fn second_half_is_kicked_off_by_the_other_team() {
    let mut cfg = SimConfig::default();
    cfg.clock.half_seconds = 60.0;
    let mut sim = Simulation::new(cfg);
    while sim.state.period == Period::FirstHalf {
        sim.step();
    }
    let r = sim.state.restart.expect("kick-off");
    assert_eq!(r.kind, RestartKind::KickOff);
    assert_eq!(r.team, sim.state.first_kick_off.opponent());
}

fn box_foul() -> (Simulation, PlayerId, PlayerId) {
    let mut sim = Preset::LowBlockDefending.build(3, None, None);
    let attacker = sim.state.carrier().unwrap().id;
    // Carrier inside the home box, defender beside him.
    sim.place(attacker, Vec2::new(-44.0, 6.0));
    sim.set_carrier(attacker);
    let defender = sim
        .state
        .team_players(HOME)
        .iter()
        .find(|p| p.role == Role::CentreBack)
        .unwrap()
        .id;
    sim.place(defender, Vec2::new(-44.5, 7.0));
    (sim, attacker, defender)
}

#[test]
fn foul_in_the_box_gives_a_penalty_taken_by_the_best_finisher() {
    let (mut sim, attacker, defender) = box_foul();
    assert!(in_penalty_area(HOME, sim.state.player(attacker).pos));
    sim.whistle_foul(defender, attacker, false);
    let r = sim.state.restart.expect("restart");
    assert_eq!(r.kind, RestartKind::Penalty);
    assert_eq!(r.team, AWAY);
    assert_eq!(r.spot, penalty_spot(HOME));
    assert!(
        count(
            &sim,
            |e| matches!(e, MatchEvent::PenaltyAwarded { team } if *team == AWAY)
        ) == 1
    );

    let plan = plan_team(&sim.state, AWAY);
    let taker = plan.restart_taker.unwrap();
    let best = sim
        .state
        .on_pitch(AWAY)
        .filter(|p| !p.is_goalkeeper())
        .map(|p| p.attrs.finishing)
        .fold(0.0, f32::max);
    assert!((sim.state.player(taker).attrs.finishing - best).abs() < 0.05);

    // Run until the kick is taken: everyone else waits outside the area.
    let mut taken = false;
    for _ in 0..30 * 30 {
        sim.step();
        if let Some((_, MatchEvent::Shot { kind, by, .. })) = sim.events.last() {
            assert_eq!(*kind, ShotKind::Penalty);
            assert_eq!(*by, taker);
            taken = true;
            break;
        }
        for d in &sim.decisions {
            let p = sim.state.player(d.player);
            if d.player == taker || (p.is_goalkeeper() && p.team == HOME) || p.sent_off {
                continue;
            }
            assert!(
                !in_penalty_area(HOME, d.target),
                "{:?} targets {:?} inside the area",
                d.player,
                d.target
            );
        }
    }
    assert!(taken, "penalty never taken");
}

#[test]
fn direct_free_kick_near_goal_gets_a_wall() {
    let mut sim = Preset::LowBlockDefending.build(3, None, None);
    let attacker = sim.state.carrier().unwrap().id;
    sim.place(attacker, Vec2::new(-30.0, 2.0));
    sim.set_carrier(attacker);
    let defender = sim
        .state
        .team_players(HOME)
        .iter()
        .find(|p| p.role == Role::DefensiveMid || p.role == Role::CentralMid)
        .unwrap()
        .id;
    sim.place(defender, Vec2::new(-29.0, 2.5));
    sim.whistle_foul(defender, attacker, false);
    assert_eq!(sim.state.restart.unwrap().kind, RestartKind::FreeKick);
    let plan = plan_team(&sim.state, HOME);
    assert!(plan.wall.len() >= 3, "{:?}", plan.wall);
    sim.run(30 * 3);
    let in_wall = sim
        .decisions
        .iter()
        .filter(|d| d.activity == Activity::Wall)
        .count();
    assert!(in_wall >= 3 || sim.state.restart.is_none());
}

#[test]
fn second_yellow_sends_a_player_off_and_he_takes_no_part() {
    let mut sim = Preset::MidfieldProgression.build(1, None, None);
    let victim = sim
        .state
        .team_players(AWAY)
        .iter()
        .find(|p| p.role == Role::CentreBack)
        .unwrap()
        .id;
    sim.caution(victim, CardDecision::Yellow);
    assert!(!sim.state.player(victim).sent_off);
    sim.caution(victim, CardDecision::Yellow);
    assert!(sim.state.player(victim).sent_off);
    assert_eq!(
        count(
            &sim,
            |e| matches!(e, MatchEvent::RedCard { player, second_yellow: true } if *player == victim)
        ),
        1
    );
    for _ in 0..30 * 60 {
        sim.step();
        assert_eq!(sim.decisions[victim.index()].activity, Activity::SentOff);
        assert_ne!(sim.state.ball.owner, Some(victim));
        let plans = sim.plans.as_ref().unwrap();
        assert!(plans.iter().all(|p| p
            .marking
            .iter()
            .all(|m| m.marker != victim && m.target != victim)));
        assert!(plans.iter().all(|p| !p.press.involves(victim)));
    }
    assert_eq!(sim.state.on_pitch(AWAY).count(), 10);
}

#[test]
fn goalkeepers_are_not_sent_off_without_substitutes() {
    let mut sim = Simulation::new(SimConfig::default());
    let gk = sim.state.goalkeeper(HOME).id;
    sim.caution(gk, CardDecision::Red);
    assert!(!sim.state.player(gk).sent_off);
}

#[test]
fn advantage_is_played_and_called_back_if_the_ball_is_lost() {
    let mut sim = Preset::MidfieldProgression.build(1, None, None);
    let carrier = sim.state.carrier().unwrap().id;
    sim.place(carrier, Vec2::new(10.0, 0.0));
    sim.set_carrier(carrier);
    let fouler = sim
        .state
        .team_players(AWAY)
        .iter()
        .find(|p| p.role == Role::DefensiveMid || p.role == Role::CentralMid)
        .unwrap()
        .id;
    sim.place(fouler, Vec2::new(9.0, 1.5));
    // Make sure the carrier has some space so advantage is worth playing.
    for p in sim.state.team_players(AWAY).to_vec() {
        if p.id != fouler && p.pos.distance(Vec2::new(10.0, 0.0)) < 4.0 {
            sim.place(p.id, p.pos + Vec2::new(10.0, 10.0));
        }
    }
    sim.whistle_foul(fouler, carrier, true);
    assert!(sim.state.advantage.is_some());
    assert!(sim.state.restart.is_none());
    assert_eq!(
        count(&sim, |e| matches!(e, MatchEvent::Advantage { .. })),
        1
    );
    // Hand the ball to the defending team: advantage did not accrue.
    let thief = sim
        .state
        .team_players(AWAY)
        .iter()
        .find(|p| p.role == Role::Striker)
        .unwrap()
        .id;
    sim.place(thief, Vec2::new(12.0, -3.0));
    sim.set_carrier(thief);
    sim.state.advantage.as_mut().unwrap().tick = sim.state.tick;
    sim.step();
    assert_eq!(
        count(&sim, |e| matches!(
            e,
            MatchEvent::AdvantageCalledBack { .. }
        )),
        1
    );
    let r = sim.state.restart.expect("free kick");
    assert_eq!(r.team, HOME);
}

#[test]
fn fouls_and_cards_happen_at_football_rates() {
    let styles = presets::team_archetypes();
    let (mut fouls, mut yellows, mut reds, mut pens) = (0.0, 0.0, 0.0, 0.0);
    let n = 6;
    for seed in 0..n {
        let mut sim = Simulation::new(SimConfig::new(
            seed,
            Squad::generated(styles[seed as usize % 6].clone(), 80),
            Squad::generated(styles[(seed as usize + 3) % 6].clone(), 80),
        ));
        sim.run_to_full_time(30 * 60 * 120);
        fouls += count(&sim, |e| matches!(e, MatchEvent::Foul { .. })) as f32 / n as f32;
        yellows += count(&sim, |e| matches!(e, MatchEvent::YellowCard { .. })) as f32 / n as f32;
        reds += count(&sim, |e| matches!(e, MatchEvent::RedCard { .. })) as f32 / n as f32;
        pens += count(&sim, |e| matches!(e, MatchEvent::PenaltyAwarded { .. })) as f32 / n as f32;
    }
    assert!((8.0..45.0).contains(&fouls), "fouls {fouls}");
    assert!((0.5..8.0).contains(&yellows), "yellows {yellows}");
    assert!(reds < 1.5, "reds {reds}");
    assert!(pens < 2.0, "penalties {pens}");
}

#[test]
fn strict_referee_gives_more_fouls_than_lenient() {
    // Summed over several matches: a single match has too few fouls to compare.
    let run = |referee: Referee| {
        (0..4)
            .map(|seed| {
                let mut cfg = SimConfig::new(
                    seed,
                    Squad::generated(presets::gegenpress(), 80),
                    Squad::generated(presets::gegenpress(), 80),
                );
                cfg.referee = referee;
                let mut sim = Simulation::new(cfg);
                sim.run_to_full_time(30 * 60 * 120);
                count(&sim, |e| matches!(e, MatchEvent::Foul { .. }))
            })
            .sum::<usize>()
    };
    let (strict, lenient) = (run(Referee::strict()), run(Referee::lenient()));
    assert!(strict > lenient, "strict {strict} lenient {lenient}");
}

#[test]
fn teams_change_ends_at_half_time() {
    let mut cfg = SimConfig::default();
    cfg.clock.half_seconds = 60.0;
    let mut sim = Simulation::new(cfg);
    assert!(!sim.state.ends_swapped);
    let p = Vec2::new(30.0, -10.0);
    assert_eq!(sim.state.display_pos(p), p);
    while sim.state.period == Period::FirstHalf {
        sim.step();
    }
    assert!(sim.state.ends_swapped);
    assert_eq!(sim.state.display_pos(p), Vec2::new(-30.0, 10.0));
}

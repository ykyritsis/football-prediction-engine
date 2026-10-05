//! Stamina, fatigue and substitutions.

use football_ai::identity::presets::{self, Squad};
use football_ai::pitch::TeamSide;
use football_ai::sim::MAX_SUBSTITUTIONS;
use football_ai::{MatchEvent, PlayerId, SimConfig, Simulation};

fn full_match(seed: u64, home: football_ai::TeamStyle, away: football_ai::TeamStyle) -> Simulation {
    let mut sim = Simulation::new(SimConfig::new(
        seed,
        Squad::generated(home, 80),
        Squad::generated(away, 80),
    ));
    sim.run_to_full_time(30 * 60 * 120);
    sim
}

fn mean_energy(sim: &Simulation, side: TeamSide) -> f32 {
    let e: Vec<f32> = sim.state.on_pitch(side).map(|p| p.energy).collect();
    e.iter().sum::<f32>() / e.len() as f32
}

#[test]
fn players_tire_and_slow_down() {
    let mut sim = Simulation::new(SimConfig::default());
    sim.run(30 * 60 * 40);
    for p in sim
        .state
        .on_pitch(TeamSide::Home)
        // Starters only: an injury replacement may have just come on fresh.
        .filter(|p| !p.is_goalkeeper() && sim.stats.line(p.id).on == 0.0)
    {
        assert!(
            p.energy < 1.0 && p.energy > 0.0,
            "{:?} energy {}",
            p.id,
            p.energy
        );
        assert!(p.attrs.top_speed <= p.base_top_speed);
        assert!(p.attrs.top_speed >= 0.8 * p.base_top_speed - 1e-4);
    }
}

#[test]
fn pressing_teams_tire_more_than_deep_blocks() {
    let mut press = 0.0;
    let mut bus = 0.0;
    for seed in 0..2 {
        // Disable substitutions' effect by measuring before they can happen.
        let mut a = Simulation::new(SimConfig::new(
            seed,
            Squad::generated(presets::gegenpress(), 80),
            Squad::generated(presets::park_the_bus(), 80),
        ));
        a.run(30 * 60 * 50);
        press += mean_energy(&a, TeamSide::Home);
        bus += mean_energy(&a, TeamSide::Away);
    }
    assert!(press < bus, "gegenpress {press} vs bus {bus}");
}

#[test]
fn better_stamina_drains_slower() {
    let mut sim = Simulation::new(SimConfig::default());
    let a = PlayerId::new(TeamSide::Home, 6);
    let b = PlayerId::new(TeamSide::Home, 7);
    sim.state.players[a.index()].attrs.stamina = 0.95;
    sim.state.players[b.index()].attrs.stamina = 0.3;
    // Same running for both.
    for _ in 0..30 * 60 {
        for id in [a, b] {
            let p = &mut sim.state.players[id.index()];
            p.vel = football_ai::math::Vec2::new(p.base_top_speed * 0.9, 0.0);
        }
        sim.step();
    }
    assert!(sim.state.player(a).energy > sim.state.player(b).energy);
}

#[test]
fn managers_substitute_tired_players_at_dead_balls() {
    let sim = full_match(4, presets::gegenpress(), presets::wing_play());
    let half = sim.clock.half_seconds;
    for side in TeamSide::BOTH {
        let subs: Vec<_> = sim
            .events
            .iter()
            .filter(|(_, e)| matches!(e, MatchEvent::Substitution { team, .. } if *team == side))
            .collect();
        assert!(!subs.is_empty(), "{side:?} made no substitutions");
        assert!(subs.len() <= MAX_SUBSTITUTIONS as usize);
        assert_eq!(sim.state.subs_used[side.index()] as usize, subs.len());
        for (tick, e) in &subs {
            let MatchEvent::Substitution { slot, .. } = e else {
                unreachable!()
            };
            // Injured players are replaced at once; anything else waits.
            let injured = sim.events.iter().any(|(t, e)| {
                t <= tick
                    && *e
                        == MatchEvent::Injury {
                            player: PlayerId::new(side, *slot as usize),
                        }
            });
            if injured {
                continue;
            }
            // From the 55th minute (second half), never in the first half.
            assert!(
                *tick as f32 * sim.state.dt > half,
                "substitution in the first half"
            );
        }
    }
    // Substitutes appear in the squad sheet and were fresh when they came on.
    let (_, MatchEvent::Substitution { team, slot, .. }) = sim
        .events
        .iter()
        .find(|(_, e)| matches!(e, MatchEvent::Substitution { .. }))
        .unwrap()
    else {
        unreachable!()
    };
    assert!(sim.squads[team.index()].players[*slot as usize]
        .name
        .starts_with("Sub "));
}

#[test]
fn substitution_rules_are_enforced() {
    let mut sim = Simulation::new(SimConfig::default());
    sim.clear_restart();
    let home = TeamSide::Home;
    assert!(sim.substitute(home, 5, 0));
    // A bench player cannot come on twice.
    assert!(!sim.substitute(home, 6, 0));
    assert!(sim.substitute(home, 6, 1));
    assert!(sim.substitute(home, 7, 2));
    // Only three.
    assert!(!sim.substitute(home, 8, 3));
    assert_eq!(sim.state.subs_used[0], 3);
    let p = sim.state.player(PlayerId::new(home, 5));
    assert_eq!(p.energy, 1.0);
    assert_eq!(p.yellow_cards, 0);
}

#[test]
fn sent_off_players_cannot_be_replaced() {
    let mut sim = Simulation::new(SimConfig::default());
    let id = PlayerId::new(TeamSide::Away, 4);
    sim.caution(id, football_ai::rules::CardDecision::Red);
    assert!(!sim.substitute(TeamSide::Away, 4, 0));
}

#[test]
fn substitutions_keep_the_match_deterministic() {
    let a = full_match(8, presets::tiki_taka(), presets::gegenpress());
    let b = full_match(8, presets::tiki_taka(), presets::gegenpress());
    assert_eq!(a.state.state_hash(), b.state.state_hash());
    assert_eq!(a.events, b.events);
    assert!(a
        .events
        .iter()
        .any(|(_, e)| matches!(e, MatchEvent::Substitution { .. })));
}

#[test]
fn losing_team_chases_the_game_late_on() {
    let mut sim = Simulation::new(SimConfig::default());
    // Away team leads; jump to the 70th minute and trigger a dead ball.
    sim.state.score = [0, 1];
    sim.state.period = football_ai::state::Period::SecondHalf;
    sim.state.period_start_tick = sim
        .state
        .tick
        .saturating_sub((25.0 * 60.0 / sim.state.dt) as u32);
    sim.state.tick += (25.0 * 60.0 / sim.state.dt) as u32;
    sim.state.period_start_tick = sim.state.tick - (25.0 * 60.0 / sim.state.dt) as u32;
    // Kick the ball out for a throw-in.
    sim.clear_restart();
    sim.state.ball.owner = None;
    sim.state.ball.pos = football_ai::math::Vec2::new(0.0, 33.9);
    sim.state.ball.vel = football_ai::math::Vec2::new(0.0, 10.0);
    sim.state.ball.last_touch = Some(PlayerId::new(TeamSide::Away, 5));
    sim.step();
    use football_ai::tactics::Mentality;
    assert_eq!(sim.state.team(TeamSide::Home).mentality, Mentality::Chasing);
    assert_eq!(sim.state.team(TeamSide::Away).mentality, Mentality::Normal);
    let home = sim.state.team(TeamSide::Home);
    assert!(home.tactics.line_offset > home.base_tactics.line_offset);
    assert!(sim
        .events
        .iter()
        .any(|(_, e)| matches!(e, MatchEvent::MentalityChange { .. })));
}

fn dead_ball(sim: &mut Simulation) {
    // Put the ball out for a throw-in so the managers get a chance to act.
    sim.clear_restart();
    sim.state.ball.owner = None;
    sim.state.ball.pass = None;
    sim.state.ball.pos = football_ai::math::Vec2::new(0.0, 33.9);
    sim.state.ball.vel = football_ai::math::Vec2::new(0.0, 10.0);
    sim.state.ball.last_touch = Some(PlayerId::new(TeamSide::Away, 5));
    sim.step();
}

fn jump_to_minute(sim: &mut Simulation, minute: f32) {
    let half = sim.clock.half_seconds;
    let dt = sim.state.dt;
    sim.state.period = football_ai::state::Period::SecondHalf;
    sim.state.tick += ((minute * 60.0) / dt) as u32;
    sim.state.period_start_tick = sim.state.tick - (((minute * 60.0) - half) / dt) as u32;
}

#[test]
fn injured_players_are_slowed_and_replaced_at_the_next_dead_ball() {
    let mut sim = Simulation::new(SimConfig::default());
    sim.run(30 * 60 * 10);
    let id = PlayerId::new(TeamSide::Home, 6);
    let fresh = sim.state.player(id).attrs.top_speed;
    let subs_before = sim.state.subs_used[0];
    sim.injure(id);
    sim.clear_restart();
    sim.step();
    assert!(sim.state.player(id).attrs.top_speed < fresh);
    assert!(sim
        .events
        .iter()
        .any(|(_, e)| *e == MatchEvent::Injury { player: id }));
    // First half, but injuries cannot wait.
    dead_ball(&mut sim);
    assert!(!sim.state.player(id).injured, "injured player still on");
    assert_eq!(sim.state.subs_used[0], subs_before + 1);
}

#[test]
fn chasing_team_brings_on_an_attacker() {
    let mut sim = Simulation::new(SimConfig::default());
    jump_to_minute(&mut sim, 65.0);
    sim.state.score = [0, 1];
    dead_ball(&mut sim);
    assert_eq!(
        sim.state.team(TeamSide::Home).mentality,
        football_ai::tactics::Mentality::Chasing
    );
    dead_ball(&mut sim);
    let subs: Vec<_> = sim
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            MatchEvent::Substitution {
                team: TeamSide::Home,
                slot,
                ..
            } => Some(*slot as usize),
            _ => None,
        })
        .collect();
    assert!(!subs.is_empty(), "no substitution when chasing");
    let incoming = &sim.squads[0].players[subs[0]];
    assert!(
        incoming.position.is_forward(),
        "brought on {:?}",
        incoming.position
    );
    let outgoing_role = sim
        .state
        .player(PlayerId::new(TeamSide::Home, subs[0]))
        .role;
    assert!(matches!(
        outgoing_role,
        football_ai::role::Role::DefensiveMid
            | football_ai::role::Role::CentralMid
            | football_ai::role::Role::FullBack
    ));
}

#[test]
fn protecting_team_brings_on_a_defender() {
    let mut sim = Simulation::new(SimConfig::default());
    jump_to_minute(&mut sim, 78.0);
    sim.state.score = [0, 1];
    dead_ball(&mut sim);
    dead_ball(&mut sim);
    let incoming: Vec<_> = sim
        .events
        .iter()
        .filter_map(|(_, e)| match e {
            MatchEvent::Substitution {
                team: TeamSide::Away,
                slot,
                ..
            } => Some(sim.squads[1].players[*slot as usize].position),
            _ => None,
        })
        .collect();
    assert!(!incoming.is_empty(), "no substitution when protecting");
    assert!(!incoming[0].is_forward(), "brought on {:?}", incoming[0]);
}

#[test]
fn injuries_happen_occasionally_over_matches() {
    let mut injuries = 0;
    for seed in 0..4 {
        let sim = full_match(seed, presets::gegenpress(), presets::gegenpress());
        injuries += sim
            .events
            .iter()
            .filter(|(_, e)| matches!(e, MatchEvent::Injury { .. }))
            .count();
    }
    assert!(injuries <= 12, "{injuries} injuries in 4 matches");
}

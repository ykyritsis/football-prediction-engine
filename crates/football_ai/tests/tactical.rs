//! Tactical behaviour in controlled scenarios.

use football_ai::decision::{decide, Activity};
use football_ai::formation::team_shape;
use football_ai::identity::presets::{self, Squad};
use football_ai::marking::assign_marking;
use football_ai::math::Vec2;
use football_ai::passing::{evaluate_passes, interception_risk, lines_broken, PassKind};
use football_ai::phase::{phase_for, TeamPhase};
use football_ai::pitch::{TeamSide, HALF_LENGTH};
use football_ai::plan::plan_team;
use football_ai::pressing::{evaluate_triggers, PressMode, PressTrigger};
use football_ai::role::Role;
use football_ai::scenario::Preset;
use football_ai::support::SupportRun;
use football_ai::{PlayerId, SimConfig, Simulation};

const HOME: TeamSide = TeamSide::Home;
const AWAY: TeamSide = TeamSide::Away;

fn sim(home: football_ai::TeamStyle, away: football_ai::TeamStyle) -> Simulation {
    Simulation::new(SimConfig::new(
        1,
        Squad::generated(home, 80),
        Squad::generated(away, 80),
    ))
}

fn first(sim: &Simulation, side: TeamSide, role: Role) -> PlayerId {
    sim.state
        .team_players(side)
        .iter()
        .find(|p| p.role == role)
        .unwrap()
        .id
}

fn spread(points: &[Vec2]) -> (f32, f32) {
    let xs = points.iter().map(|p| p.x);
    let ys = points.iter().map(|p| p.y);
    let w = ys.clone().fold(f32::MIN, f32::max) - ys.fold(f32::MAX, f32::min);
    let d = xs.clone().fold(f32::MIN, f32::max) - xs.fold(f32::MAX, f32::min);
    (d, w)
}

// ------------------------------------------------------------ positioning --

#[test]
fn team_is_more_compact_out_of_possession() {
    let s = sim(presets::gegenpress(), presets::gegenpress());
    let st = &s.state;
    let outfield = |shape: [Vec2; 11]| shape[1..].to_vec();
    let with = outfield(team_shape(st, HOME, TeamPhase::Progression));
    let without = outfield(team_shape(st, HOME, TeamPhase::MidBlock));
    let (dw, ww) = spread(&with);
    let (dwo, wwo) = spread(&without);
    assert!(wwo < ww, "width out {wwo} vs in {ww}");
    assert!(dwo < dw, "depth out {dwo} vs in {dw}");
}

#[test]
fn block_slides_towards_the_ball() {
    let mut s = sim(presets::wing_play(), presets::wing_play());
    let carrier = first(&s, AWAY, Role::Winger);
    s.arrange(carrier, Vec2::new(0.0, 25.0));
    let left = team_shape(&s.state, HOME, TeamPhase::MidBlock);
    s.arrange(carrier, Vec2::new(0.0, -25.0));
    let right = team_shape(&s.state, HOME, TeamPhase::MidBlock);
    let mean_y = |v: &[Vec2; 11]| v[1..].iter().map(|p| p.y).sum::<f32>() / 10.0;
    assert!(
        mean_y(&left) > mean_y(&right) + 10.0,
        "{} vs {}",
        mean_y(&left),
        mean_y(&right)
    );
}

#[test]
fn high_press_team_defends_higher_than_low_block_team() {
    let s = sim(presets::gegenpress(), presets::park_the_bus());
    let st = &s.state;
    let back_line = |side: TeamSide, phase| {
        let shape = team_shape(st, side, phase);
        let f = &st.team(side).formation;
        let xs: Vec<f32> = (1..11)
            .filter(|&i| f.slots[i].role == Role::CentreBack)
            .map(|i| side.to_local(shape[i]).x)
            .collect();
        xs.iter().sum::<f32>() / xs.len() as f32
    };
    assert!(back_line(HOME, TeamPhase::MidBlock) > back_line(AWAY, TeamPhase::MidBlock) + 8.0);
}

#[test]
fn home_and_away_shapes_mirror() {
    let mut s = sim(presets::tiki_taka(), presets::tiki_taka());
    let carrier = PlayerId::new(HOME, 6);
    s.arrange(carrier, Vec2::new(10.0, 7.0));
    let st = s.state.clone();
    // Build the mirrored situation: away has the ball at the rotated spot.
    let mut m = sim(presets::tiki_taka(), presets::tiki_taka());
    for p in &st.players {
        let mirror = PlayerId::new(p.team.opponent(), p.id.slot());
        m.place(mirror, -p.pos);
    }
    m.place(PlayerId::new(AWAY, 6), Vec2::new(-10.0, -7.0));
    m.set_carrier(PlayerId::new(AWAY, 6));
    let a = team_shape(&st, HOME, TeamPhase::Progression);
    let b = team_shape(&m.state, AWAY, TeamPhase::Progression);
    for i in 0..11 {
        assert!(
            (a[i] + b[i]).length() < 1e-3,
            "slot {i}: {:?} vs {:?}",
            a[i],
            b[i]
        );
    }
}

#[test]
fn goalkeepers_stay_near_their_goal() {
    let mut s = Preset::MidfieldProgression.build(4, None, None);
    for _ in 0..900 {
        s.step();
        for side in TeamSide::BOTH {
            let gk = s.state.goalkeeper(side);
            if s.state.ball.owner == Some(gk.id) {
                continue;
            }
            assert!(
                side.to_local(gk.pos).x < -HALF_LENGTH + 30.0,
                "{side:?} keeper wandered to {:?}",
                gk.pos
            );
        }
    }
}

#[test]
fn attackers_hold_onside_positions_in_shape() {
    let mut s = Preset::MidfieldProgression.build(1, None, None);
    s.run(1);
    let line = s.state.offside_line_local(HOME);
    let shape = team_shape(&s.state, HOME, TeamPhase::Progression);
    for p in shape {
        assert!(HOME.to_local(p).x <= line + 0.01);
    }
}

// ----------------------------------------------------------------- phases --

#[test]
fn phases_follow_scenarios() {
    let s = Preset::BuildUpUnderPress.build(1, None, None);
    assert_eq!(phase_for(&s.state, HOME), TeamPhase::BuildUp);
    assert_eq!(phase_for(&s.state, AWAY), TeamPhase::HighPress);

    let s = Preset::WideFinalThird.build(1, None, None);
    assert_eq!(phase_for(&s.state, HOME), TeamPhase::FinalThird);
    assert_eq!(phase_for(&s.state, AWAY), TeamPhase::LowBlock);

    let s = Preset::CounterAttack.build(1, None, None);
    assert_eq!(phase_for(&s.state, HOME), TeamPhase::AttackingTransition);
    assert_eq!(phase_for(&s.state, AWAY), TeamPhase::DefensiveTransition);
}

// --------------------------------------------------------------- pressing --

#[test]
fn touchline_and_isolation_trigger_pressing() {
    let mut s = sim(presets::gegenpress(), presets::gegenpress());
    let w = first(&s, AWAY, Role::Winger);
    s.arrange(w, Vec2::new(5.0, 31.0));
    // Remove nearby support.
    for p in s.state.team_players(AWAY).to_vec() {
        if p.id != w && p.pos.distance(s.state.player(w).pos) < 13.0 {
            s.place(p.id, p.pos + Vec2::new(-15.0, -15.0));
        }
    }
    let phase = phase_for(&s.state, HOME);
    let (set, intensity) = evaluate_triggers(&s.state, HOME, phase);
    assert!(set.contains(PressTrigger::NearTouchline));
    assert!(set.contains(PressTrigger::IsolatedCarrier));
    assert!(intensity > 0.5, "{intensity}");
}

#[test]
fn backward_pass_and_facing_own_goal_trigger_pressing() {
    let mut s = sim(presets::gegenpress(), presets::gegenpress());
    let cm = first(&s, AWAY, Role::DefensiveMid);
    s.arrange(cm, Vec2::new(10.0, 0.0));
    // Away attacks -x: a backward pass travels towards +x.
    let from = first(&s, AWAY, Role::AttackingMid);
    s.state.last_completed_pass = Some(football_ai::state::PassRecord {
        team: AWAY,
        from,
        to: cm,
        origin: Vec2::new(0.0, 0.0),
        received_at: Vec2::new(10.0, 0.0),
        completed_tick: s.state.tick,
    });
    s.state.players[cm.index()].facing = Vec2::new(1.0, 0.0); // towards his own goal
    let (set, _) = evaluate_triggers(&s.state, HOME, phase_for(&s.state, HOME));
    assert!(set.contains(PressTrigger::BackwardPass));
    assert!(set.contains(PressTrigger::FacingOwnGoal));
}

#[test]
fn aggressive_teams_press_where_passive_teams_contain() {
    // Identical situation, different team identities.
    let situation = |home: football_ai::TeamStyle| {
        let mut s = sim(home, presets::tiki_taka());
        let cb = first(&s, AWAY, Role::CentreBack);
        s.arrange(cb, Vec2::new(30.0, 5.0));
        plan_team(&s.state, HOME).press
    };
    let gp = situation(presets::gegenpress());
    let bus = situation(presets::park_the_bus());
    assert!(gp.intensity > bus.intensity);
    assert_eq!(gp.mode, PressMode::Press);
    assert_eq!(bus.mode, PressMode::Contain);
}

#[test]
fn presser_is_a_nearby_outfield_player_and_tackles_in_range() {
    let mut s = Preset::BuildUpUnderPress.build(
        2,
        Some(Squad::generated(presets::tiki_taka(), 80)),
        Some(Squad::generated(presets::gegenpress(), 80)),
    );
    s.run(1);
    let plan = plan_team(&s.state, AWAY);
    let carrier = s.state.carrier().unwrap().clone();
    let primary = plan.press.primary.expect("someone engages the ball");
    let pp = s.state.player(primary);
    assert!(!pp.is_goalkeeper());
    let nearest = s
        .state
        .team_players(AWAY)
        .iter()
        .filter(|p| !p.is_goalkeeper())
        .map(|p| p.pos.distance(carrier.pos))
        .fold(f32::MAX, f32::min);
    assert!(pp.pos.distance(carrier.pos) < nearest + 12.0);
    // Right next to a carrier who faces him and is on the move, a weak
    // tackler jockeys instead of diving in.
    s.place(primary, carrier.pos + Vec2::new(1.0, 0.0));
    s.state.players[primary.index()].attrs.tackling = 0.3;
    s.state.players[carrier.id.index()].facing = Vec2::new(1.0, 0.0);
    s.set_velocity(carrier.id, Vec2::new(4.0, 0.0));
    s.state.players[carrier.id.index()].action_cooldown = 0;
    let plan = plan_team(&s.state, AWAY);
    let d = decide(&s.state, &plan, primary);
    assert!(
        matches!(d.activity, Activity::Press(_) | Activity::Contain(_)),
        "{d:?}"
    );
    assert_eq!(d.action, football_ai::Action::None, "{d:?}");
    // On the receiving touch even a good tackler only closes down...
    s.state.players[primary.index()].attrs.tackling = 0.9;
    s.state.players[carrier.id.index()].action_cooldown = 5;
    let plan = plan_team(&s.state, AWAY);
    let d = decide(&s.state, &plan, primary);
    assert_eq!(d.action, football_ai::Action::None, "{d:?}");
    // ...and commits once the carrier has it and has stopped.
    s.state.players[carrier.id.index()].action_cooldown = 0;
    s.set_velocity(carrier.id, Vec2::ZERO);
    s.state.ball.owner_since = s.state.tick.saturating_sub(30);
    s.state.tick = s.state.tick.max(30);
    let plan = plan_team(&s.state, AWAY);
    let d = decide(&s.state, &plan, primary);
    assert!(
        matches!(d.action, football_ai::Action::Tackle(t) if t == carrier.id),
        "{d:?}"
    );
}

#[test]
fn gegenpress_swarms_in_defensive_transition() {
    let mut s = Preset::CounterAttack.build(
        1,
        Some(Squad::generated(presets::tiki_taka(), 80)),
        Some(Squad::generated(presets::gegenpress(), 80)),
    );
    // Bring two away players close to the ball so they can swarm.
    let c = s.state.carrier().unwrap().pos;
    s.place(PlayerId::new(AWAY, 6), c + Vec2::new(4.0, 2.0));
    s.place(PlayerId::new(AWAY, 7), c + Vec2::new(5.0, -3.0));
    s.place(PlayerId::new(AWAY, 8), c + Vec2::new(7.0, 1.0));
    let plan = plan_team(&s.state, AWAY);
    assert_eq!(plan.phase, TeamPhase::DefensiveTransition);
    assert_eq!(plan.press.mode, PressMode::Press);
    assert!(
        plan.press.swarm.iter().flatten().count() >= 1,
        "{:?}",
        plan.press
    );
}

// ---------------------------------------------------------------- marking --

#[test]
fn marking_assignments_are_unique_and_goal_side() {
    let s = Preset::LowBlockDefending.build(3, None, None);
    let plan = plan_team(&s.state, HOME);
    assert!(!plan.marking.is_empty());
    let mut markers: Vec<_> = plan.marking.iter().map(|m| m.marker).collect();
    let mut targets: Vec<_> = plan.marking.iter().map(|m| m.target).collect();
    markers.sort();
    markers.dedup();
    targets.sort();
    targets.dedup();
    assert_eq!(markers.len(), plan.marking.len());
    assert_eq!(targets.len(), plan.marking.len());
    for m in &plan.marking {
        assert_eq!(m.marker.team(), HOME);
        assert_eq!(m.target.team(), AWAY);
        assert!(!plan.press.involves(m.marker));
        let pos = football_ai::marking::marking_position(&s.state, HOME, m.target, m.tightness);
        let att = s.state.player(m.target);
        // Goal-side: closer to the home goal than the (anticipated) attacker.
        assert!(
            pos.distance(HOME.own_goal()) < (att.pos + att.vel * 0.5).distance(HOME.own_goal())
        );
    }
}

#[test]
fn centre_backs_do_not_follow_wingers_to_the_touchline() {
    let s = Preset::LowBlockDefending.build(3, None, None);
    let plan = plan_team(&s.state, HOME);
    for m in &plan.marking {
        if s.state.player(m.marker).role == Role::CentreBack {
            assert!(HOME.to_local(s.state.player(m.target).pos).y.abs() <= 20.0);
        }
    }
}

#[test]
fn tight_marking_instruction_tightens_marking() {
    let tightness = |style: football_ai::TeamStyle| {
        let s = Preset::LowBlockDefending.build(3, Some(Squad::generated(style, 80)), None);
        let phase = phase_for(&s.state, HOME);
        let shape = team_shape(&s.state, HOME, phase);
        let m = assign_marking(&s.state, HOME, phase, &shape, &[]);
        m.iter().map(|a| a.tightness).sum::<f32>() / m.len().max(1) as f32
    };
    let mut tight = presets::wing_play();
    tight.instructions = vec![football_ai::identity::Instruction::TightMarking];
    assert!(tightness(tight) > tightness(presets::wing_play()) + 0.1);
}

// ---------------------------------------------------------------- support --

fn support_runs(s: &Simulation, side: TeamSide) -> Vec<(Role, SupportRun)> {
    plan_team(&s.state, side)
        .support
        .iter()
        .map(|a| (s.state.player(a.player).role, a.run))
        .collect()
}

#[test]
fn forward_runs_in_behind_when_carrier_is_free() {
    let mut s = Preset::MidfieldProgression.build(1, None, None);
    let c = s.state.carrier().unwrap().id;
    // Clear space around the carrier.
    for p in s.state.team_players(AWAY).to_vec() {
        if p.pos.distance(s.state.player(c).pos) < 8.0 {
            s.place(p.id, p.pos + Vec2::new(10.0, 0.0));
        }
    }
    let runs = support_runs(&s, HOME);
    assert!(
        runs.iter()
            .any(|&(r, k)| k == SupportRun::RunInBehind && r.is_forward()),
        "{runs:?}"
    );
}

#[test]
fn full_back_overlaps_a_wide_carrier() {
    let mut s = sim(presets::wing_play(), presets::park_the_bus());
    let w = PlayerId::new(HOME, 5); // left winger in 4-4-2
    assert_eq!(s.state.player(w).role, Role::Winger);
    s.arrange(w, Vec2::new(5.0, 25.0));
    s.place(PlayerId::new(HOME, 1), Vec2::new(-5.0, 28.0)); // left back behind him
    let plan = plan_team(&s.state, HOME);
    let fb = plan.support_for(PlayerId::new(HOME, 1)).map(|a| a.run);
    assert_eq!(fb, Some(SupportRun::Overlap), "{:?}", plan.support);
}

#[test]
fn box_is_attacked_when_ball_is_wide_in_final_third() {
    let s = Preset::WideFinalThird.build(1, None, None);
    let runs = support_runs(&s, HOME);
    let box_runs = runs
        .iter()
        .filter(|(_, k)| {
            matches!(
                k,
                SupportRun::NearPostRun
                    | SupportRun::FarPostRun
                    | SupportRun::PenaltySpotRun
                    | SupportRun::CutBackZone
            )
        })
        .count();
    assert!(box_runs >= 2, "{runs:?}");
}

#[test]
fn rest_defence_stays_behind_the_ball() {
    let s = Preset::WideFinalThird.build(1, None, None);
    let plan = plan_team(&s.state, HOME);
    let ball_x = HOME.to_local(s.state.ball.pos).x;
    let rest: Vec<_> = plan
        .support
        .iter()
        .filter(|a| a.run == SupportRun::RestDefence)
        .collect();
    assert!(rest.len() >= 2);
    for a in rest {
        assert!(HOME.to_local(a.target).x < ball_x - 5.0);
    }
}

#[test]
fn short_passing_teams_support_closer_to_the_ball() {
    let pocket_distance = |style: football_ai::TeamStyle| {
        let s = Preset::MidfieldProgression.build(1, Some(Squad::generated(style, 80)), None);
        let ball = s.state.ball.pos;
        let plan = plan_team(&s.state, HOME);
        let shows: Vec<f32> = plan
            .support
            .iter()
            .filter(|a| a.run == SupportRun::ShowForBall)
            .map(|a| a.target.distance(ball))
            .collect();
        assert!(!shows.is_empty());
        shows.iter().sum::<f32>() / shows.len() as f32
    };
    assert!(pocket_distance(presets::tiki_taka()) + 4.0 < pocket_distance(presets::long_ball()));
}

// ---------------------------------------------------------------- passing --

#[test]
fn blocked_lane_is_riskier_than_open_lane() {
    let mut s = sim(presets::tiki_taka(), presets::tiki_taka());
    // Move every away player far away, then put one on the lane.
    for slot in 0..11 {
        s.place(
            PlayerId::new(AWAY, slot),
            Vec2::new(40.0, -30.0 + slot as f32),
        );
    }
    let open = interception_risk(
        &s.state,
        HOME,
        Vec2::new(0.0, 0.0),
        Vec2::new(20.0, 0.0),
        15.0,
        false,
        false,
    );
    s.place(PlayerId::new(AWAY, 5), Vec2::new(10.0, 0.5));
    let blocked = interception_risk(
        &s.state,
        HOME,
        Vec2::new(0.0, 0.0),
        Vec2::new(20.0, 0.0),
        15.0,
        false,
        false,
    );
    assert!(open < 0.1, "{open}");
    assert!(blocked > 0.6, "{blocked}");
    // A lofted ball goes over him.
    let lofted = interception_risk(
        &s.state,
        HOME,
        Vec2::new(0.0, 0.0),
        Vec2::new(20.0, 0.0),
        15.0,
        true,
        false,
    );
    assert!(lofted < blocked);
}

#[test]
fn lines_broken_counts_bypassed_opponents() {
    let mut s = sim(presets::tiki_taka(), presets::tiki_taka());
    for slot in 1..11 {
        s.place(
            PlayerId::new(AWAY, slot),
            Vec2::new(-50.0 + slot as f32 * 9.0, 20.0),
        );
    }
    // Opponents at x = -41, -32, ..., 40: between -20 and 20 lie -14,-5,4,13 → 4.
    assert_eq!(
        lines_broken(&s.state, HOME, Vec2::new(-20.0, 0.0), Vec2::new(20.0, 0.0)),
        4
    );
    assert_eq!(
        lines_broken(&s.state, HOME, Vec2::new(20.0, 0.0), Vec2::new(-20.0, 0.0)),
        0
    );
}

#[test]
fn offside_receivers_are_never_pass_options() {
    let mut s = Preset::MidfieldProgression.build(1, None, None);
    let st_id = first(&s, HOME, Role::Striker);
    s.place(st_id, Vec2::new(45.0, 0.0));
    assert!(s.state.in_offside_position(st_id));
    let c = s.state.carrier().unwrap().id;
    let passes = evaluate_passes(&s.state, c, TeamPhase::Progression);
    assert!(!passes.is_empty());
    assert!(passes.iter().all(|p| p.receiver != st_id));
}

#[test]
fn pass_candidates_are_sorted_and_finite() {
    for preset in Preset::ALL {
        let s = preset.build(1, None, None);
        let Some(c) = s.state.carrier() else { continue };
        let passes = evaluate_passes(&s.state, c.id, phase_for(&s.state, c.team));
        for w in passes.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
        for p in &passes {
            assert!(
                p.score.is_finite() && (0.0..=1.0).contains(&p.risk),
                "{p:?}"
            );
        }
    }
}

#[test]
fn long_ball_team_values_long_passes_more_than_tiki_taka() {
    let best_long_minus_best_short = |style: football_ai::TeamStyle| {
        let s = Preset::BuildUpUnderPress.build(1, Some(Squad::generated(style, 80)), None);
        let c = s.state.carrier().unwrap().id;
        let passes = evaluate_passes(&s.state, c, TeamPhase::BuildUp);
        let long = passes
            .iter()
            .filter(|p| p.kind == PassKind::Long)
            .map(|p| p.style)
            .fold(f32::MIN, f32::max);
        let short = passes
            .iter()
            .filter(|p| p.kind == PassKind::Short)
            .map(|p| p.style)
            .fold(f32::MIN, f32::max);
        long - short
    };
    assert!(
        best_long_minus_best_short(presets::long_ball())
            > best_long_minus_best_short(presets::tiki_taka()) + 0.02
    );
}

// ------------------------------------------------------- role behaviour --

#[test]
fn every_role_gets_a_sensible_activity() {
    for preset in Preset::ALL {
        let mut s = preset.build(1, None, None);
        s.run(2);
        for d in &s.decisions {
            let p = s.state.player(d.player);
            if p.is_goalkeeper() && s.state.ball.owner != Some(p.id) {
                assert!(
                    matches!(
                        d.activity,
                        Activity::Goalkeeping
                            | Activity::KeepDistance
                            | Activity::ChaseBall
                            | Activity::TakeRestart
                    ),
                    "{:?} {:?}",
                    preset,
                    d
                );
            }
        }
    }
}

#[test]
fn second_defender_blocks_the_line_to_goal_near_the_box() {
    let mut s = Preset::LowBlockDefending.build(3, None, None);
    let c = s.state.carrier().unwrap().id;
    s.place(c, Vec2::new(-34.0, 4.0));
    s.set_carrier(c);
    let plan = plan_team(&s.state, HOME);
    let blocker = plan.press.blocker.expect("a blocker near goal");
    assert_ne!(Some(blocker), plan.press.primary);
    let d = decide(&s.state, &plan, blocker);
    let carrier = s.state.player(c).pos;
    // Target lies between the carrier and the goal.
    assert!(
        d.target.x < carrier.x && d.target.distance(carrier) < 3.0,
        "{d:?}"
    );
    // Far from goal there is no blocker.
    s.place(c, Vec2::new(10.0, 4.0));
    s.set_carrier(c);
    assert!(plan_team(&s.state, HOME).press.blocker.is_none());
}

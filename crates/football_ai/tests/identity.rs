//! Team and player identity must change behaviour, not just labels.

use football_ai::decision::{carrier_options, Activity};
use football_ai::identity::presets::{
    self, archetype_profile, example_messi, example_ronaldo, PlayerArchetype, Squad,
};
use football_ai::identity::{PlayingStyle, Skill};
use football_ai::math::Vec2;
use football_ai::passing::PassKind;
use football_ai::pitch::TeamSide;
use football_ai::plan::plan_team;
use football_ai::role::Role;
use football_ai::{MatchEvent, PlayerId, SimConfig, Simulation};

// ------------------------------------------------------------- profiles --

#[test]
fn messi_profile_reads_as_a_dribbler() {
    let m = example_messi();
    let tags = m.signature_traits();
    assert!(tags.contains(&"Takes defenders on"), "{tags:?}");
    assert!(m.skills.contains(Skill::DoubleTouch));
    let generic = archetype_profile(PlayerArchetype::CrossingWinger, "generic", 82);
    assert!(
        m.behaviour_in(Role::Winger).dribble_bias
            > generic.behaviour_in(Role::Winger).dribble_bias + 0.3
    );
    let d = m.describe();
    assert!(
        d.contains("Creative Playmaker") && d.contains("dribbling"),
        "{d}"
    );
}

#[test]
fn ronaldo_profile_reads_as_a_finisher() {
    let r = example_ronaldo();
    let tags = r.signature_traits();
    assert!(tags.contains(&"Clinical finisher"), "{tags:?}");
    let generic = archetype_profile(PlayerArchetype::TargetMan, "generic", 82);
    let rb = r.behaviour_in(Role::Striker);
    let gb = generic.behaviour_in(Role::Striker);
    assert!(rb.shot_threshold < gb.shot_threshold);
    assert!(rb.shot_range > gb.shot_range + 5.0);
    assert!(r.attributes().finishing > generic.attributes().finishing);
}

#[test]
fn identity_survives_a_change_of_role() {
    // A dribbler played at full-back still dribbles more than other full-backs.
    let messi_fb = example_messi().behaviour_in(Role::FullBack);
    let normal_fb = archetype_profile(PlayerArchetype::DefensiveFullBack, "fb", 80)
        .behaviour_in(Role::FullBack);
    assert!(messi_fb.dribble_bias > normal_fb.dribble_bias);
}

#[test]
fn every_playing_style_changes_behaviour() {
    for style in PlayingStyle::ALL {
        let mut p = archetype_profile(PlayerArchetype::BoxToBox, "x", 75);
        let role = p.position;
        p.playing_style = None;
        let base = p.behaviour_in(role);
        p.playing_style = Some(style);
        assert_ne!(base, p.behaviour_in(role), "{style:?} has no effect");
    }
}

#[test]
fn every_squad_member_has_a_description() {
    for style in presets::team_archetypes() {
        let squad = Squad::generated(style, 78);
        let text = squad.describe();
        assert!(text.lines().count() >= 12);
        for p in &squad.players {
            assert!(p.describe().starts_with(&p.name));
        }
    }
}

// ------------------------------------------------------- in-match effect --

/// Put `profile` on the ball as home slot `slot` in a fixed situation and
/// return (best dribble score - best pass score, best shot xg if any).
fn on_ball_tendency(
    profile: football_ai::PlayerProfile,
    slot: usize,
    ball: Vec2,
) -> (f32, Option<f32>, f32) {
    let home = Squad::generated(presets::gegenpress(), 80).with_player(slot, profile);
    let cfg = SimConfig::new(1, home, Squad::generated(presets::park_the_bus(), 80));
    let mut sim = Simulation::new(cfg);
    let id = PlayerId::new(TeamSide::Home, slot);
    sim.arrange(id, ball);
    let plan = plan_team(&sim.state, TeamSide::Home);
    let o = carrier_options(&sim.state, &plan, id);
    let best_pass = o.passes.first().map_or(-1.0, |p| p.score);
    let best_dribble = o.dribbles.first().map_or(-1.0, |d| d.score);
    (
        best_dribble - best_pass,
        o.shot.map(|s| s.xg),
        o.shot.map_or(-1.0, |s| s.score),
    )
}

#[test]
fn dribbler_prefers_carrying_in_the_same_situation() {
    // 4-2-3-1: slot 7 is the left winger.
    let ball = Vec2::new(20.0, 20.0);
    let (messi, _, _) = on_ball_tendency(example_messi(), 7, ball);
    let (crosser, _, _) = on_ball_tendency(
        archetype_profile(PlayerArchetype::CrossingWinger, "c", 80),
        7,
        ball,
    );
    assert!(messi > crosser, "messi {messi} crosser {crosser}");
}

#[test]
fn finisher_rates_the_same_chance_higher() {
    // 4-2-3-1: slot 10 is the striker.
    let ball = Vec2::new(38.0, 4.0);
    let (_, ronaldo, _) = on_ball_tendency(example_ronaldo(), 10, ball);
    let (_, plain, _) = on_ball_tendency(
        archetype_profile(PlayerArchetype::TargetMan, "t", 70),
        10,
        ball,
    );
    assert!(ronaldo.unwrap() > plain.unwrap());
}

fn count_activity(
    sim: &mut Simulation,
    id: PlayerId,
    ticks: u32,
    pred: fn(&Activity) -> bool,
) -> (u32, u32) {
    let (mut hits, mut on_ball) = (0, 0);
    for _ in 0..ticks {
        sim.step();
        // Open play only: a set-piece taker holds the ball through the
        // dead-ball time.
        if sim.state.ball.owner == Some(id) && sim.state.restart.is_none() {
            on_ball += 1;
            if pred(&sim.decisions[id.index()].activity) {
                hits += 1;
            }
        }
    }
    (hits, on_ball)
}

#[test]
fn dribbler_carries_the_ball_more_over_a_match() {
    let slot = 7;
    let run = |profile| {
        let home = Squad::generated(presets::gegenpress(), 80).with_player(slot, profile);
        let cfg = SimConfig::new(21, home, Squad::generated(presets::counter_attack(), 80));
        let mut sim = Simulation::new(cfg);
        let (d, n) = count_activity(
            &mut sim,
            PlayerId::new(TeamSide::Home, slot),
            30 * 60 * 30,
            |a| matches!(a, Activity::Dribble),
        );
        d as f32 / n.max(1) as f32
    };
    let messi = run(example_messi());
    let crosser = run(archetype_profile(PlayerArchetype::CrossingWinger, "c", 80));
    assert!(
        messi > crosser,
        "dribble share messi {messi:.2} vs crosser {crosser:.2}"
    );
}

// ------------------------------------------------------- team identity --

#[derive(Default, Debug, Clone, Copy)]
struct TeamStats {
    passes: f32,
    long: f32,
    pass_len: f32,
    possession: f32,
    high_regains: f32,
    /// Opponent passes in their own 60% of the pitch, and this team's
    /// defensive actions in that zone: PPDA = passes / actions.
    ppda_passes: f32,
    ppda_actions: f32,
}

fn play(
    home: football_ai::TeamStyle,
    away: football_ai::TeamStyle,
    seed: u64,
    minutes: f32,
) -> [TeamStats; 2] {
    let cfg = SimConfig::new(seed, Squad::generated(home, 80), Squad::generated(away, 80));
    let mut sim = Simulation::new(cfg);
    let mut out = [TeamStats::default(), TeamStats::default()];
    let ticks = (minutes * 60.0 * 30.0) as u32;
    for _ in 0..ticks {
        let before = sim.events.len();
        let ball = sim.state.ball.pos;
        sim.step();
        out[sim.state.possession.index()].possession += 1.0 / ticks as f32;
        for (_, e) in &sim.events[before..] {
            match *e {
                MatchEvent::Pass { from, kind, .. } => {
                    if from.team().to_local(ball).x < 10.0 {
                        out[from.team().opponent().index()].ppda_passes += 1.0;
                    }
                    let s = &mut out[from.team().index()];
                    s.passes += 1.0;
                    if kind == PassKind::Long {
                        s.long += 1.0;
                    }
                    if let Some(p) = sim.state.ball.pass {
                        s.pass_len += p.origin.distance(p.target);
                    }
                }
                MatchEvent::Tackle { by, .. }
                | MatchEvent::Interception { by, .. }
                | MatchEvent::Foul { by, .. }
                    if by.team().to_local(ball).x > -10.0 =>
                {
                    out[by.team().index()].ppda_actions += 1.0;
                }
                // Possession won in the attacking third, however it came:
                // a tackle, an interception, or a hurried pass under pressure.
                MatchEvent::PossessionChange { team } if team.to_local(ball).x > 17.5 => {
                    out[team.index()].high_regains += 1.0;
                }
                _ => {}
            }
        }
    }
    out
}

#[test]
fn long_ball_team_plays_longer_than_tiki_taka() {
    let mut lb = (0.0, 0.0);
    let mut tt = (0.0, 0.0);
    for seed in 0..2 {
        let [a, b] = play(presets::tiki_taka(), presets::long_ball(), 100 + seed, 30.0);
        tt.0 += a.long / a.passes;
        tt.1 += a.pass_len / a.passes;
        lb.0 += b.long / b.passes;
        lb.1 += b.pass_len / b.passes;
    }
    assert!(
        lb.0 > tt.0 * 1.5,
        "long share: route one {} tiki {}",
        lb.0 / 2.0,
        tt.0 / 2.0
    );
    assert!(
        lb.1 > tt.1 + 5.0,
        "avg length: route one {} tiki {}",
        lb.1 / 2.0,
        tt.1 / 2.0
    );
}

#[test]
fn possession_team_keeps_the_ball_more_than_a_bus() {
    let [tt, bus] = play(presets::tiki_taka(), presets::park_the_bus(), 7, 30.0);
    assert!(tt.possession > bus.possession + 0.1, "{tt:?} {bus:?}");
}

#[test]
fn pressing_team_allows_fewer_passes_per_defensive_action() {
    // PPDA, the standard pressing metric: a pressing side lets the opponent
    // string far fewer passes together before it challenges.
    let ppda = |style: fn() -> football_ai::TeamStyle| {
        let (passes, actions) = (3..5)
            .map(|seed| play(style(), presets::wing_play(), seed, 90.0)[0])
            .fold((0.0, 0.0), |(p, a), t| {
                (p + t.ppda_passes, a + t.ppda_actions)
            });
        passes / f32::max(actions, 1.0)
    };
    let (gp, bus) = (ppda(presets::gegenpress), ppda(presets::park_the_bus));
    assert!(gp * 2.0 < bus, "PPDA gegenpress {gp:.1} bus {bus:.1}");
}

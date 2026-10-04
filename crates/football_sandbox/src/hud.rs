//! Text HUD: match state, phases, pressing, carrier options, selected player.

use crate::Sandbox;
use bevy::prelude::*;
use football_ai::pitch::TeamSide;
use football_ai::pressing::PressMode;
use football_ai::MatchEvent;

#[derive(Component)]
pub struct InfoPanel;

#[derive(Component)]
pub struct HelpPanel;

pub fn setup(mut commands: Commands) {
    commands.spawn((
        InfoPanel,
        Text::new(""),
        TextFont::from_font_size(13.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            width: Val::Px(400.0),
            ..default()
        },
    ));
    commands.spawn((
        HelpPanel,
        Text::new(
            "Space pause | Right/. step | +/- speed | 1-6 presets | R reset | N new seed | H/A cycle home/away style | X star players | F foul on carrier\n\
             Click/Tab select player, Esc clear | Overlays: T targets, S shape, M marking, P pressing, U support, C pass options, O offside",
        ),
        TextFont::from_font_size(12.0),
        TextColor(Color::srgb(0.75, 0.75, 0.8)),
        Node { position_type: PositionType::Absolute, bottom: Val::Px(8.0), left: Val::Px(12.0), ..default() },
    ));
}

fn side_name(side: TeamSide) -> &'static str {
    match side {
        TeamSide::Home => "HOME (blue, attacks right)",
        TeamSide::Away => "AWAY (red, attacks left)",
    }
}

pub fn update(sb: Res<Sandbox>, mut q: Query<&mut Text, With<InfoPanel>>) {
    let Ok(mut text) = q.single_mut() else { return };
    let sim = &sb.sim;
    let st = &sim.state;
    let mut s = String::new();
    let secs = sim.match_seconds();
    let period = match st.period {
        football_ai::state::Period::FirstHalf => "1st half",
        football_ai::state::Period::SecondHalf => "2nd half",
        football_ai::state::Period::FullTime => "FULL TIME",
    };
    s += &format!(
        "{}  {} - {}  {}\n{:02}:{:02} {}   speed x{}{}   seed {}\nPreset: {}\n\n",
        sim.squads[0].style.name,
        st.score[0],
        st.score[1],
        sim.squads[1].style.name,
        (secs / 60.0) as u32,
        (secs % 60.0) as u32,
        period,
        sb.speed,
        if sb.paused { "  [PAUSED]" } else { "" },
        sb.seed,
        sb.preset.name()
    );

    if let Some(plans) = sim.plans.as_ref() {
        for plan in plans {
            s += &format!(
                "{}: {} ({})\n",
                side_name(plan.side),
                plan.phase.name(),
                sim.squads[plan.side.index()].style.formation.name()
            );
            let energy: Vec<f32> = st.on_pitch(plan.side).map(|p| p.energy).collect();
            s += &format!(
                "  energy {:.0}%  subs {}/{}\n",
                100.0 * energy.iter().sum::<f32>() / energy.len().max(1) as f32,
                st.subs_used[plan.side.index()],
                football_ai::sim::MAX_SUBSTITUTIONS
            );
            if plan.press.mode != PressMode::None {
                let triggers: Vec<&str> = plan.press.triggers.iter().map(|t| t.name()).collect();
                s += &format!(
                    "  {:?} intensity {:.2}  [{}]\n",
                    plan.press.mode,
                    plan.press.intensity,
                    triggers.join(", ")
                );
            }
            if !plan.support.is_empty() {
                let mut runs: Vec<&str> = plan.support.iter().map(|a| a.run.name()).collect();
                runs.sort_unstable();
                runs.dedup();
                s += &format!("  runs: {}\n", runs.join(", "));
            }
            if !plan.marking.is_empty() {
                s += &format!("  marking: {} pairs\n", plan.marking.len());
            }
        }
        s += "\n";

        if let Some(c) = st.carrier() {
            let plan = &plans[c.team.index()];
            let o = football_ai::decision::carrier_options(st, plan, c.id);
            s += &format!("Carrier: {} ({})\n", sim.name(c.id), c.role.short_name());
            s += &format!("  hold {:+.3}", o.hold_score);
            if let Some(d) = o.dribbles.first() {
                s += &format!("   dribble {:+.3} (risk {:.2})", d.score, d.risk);
            }
            if let Some(sh) = o.shot {
                s += &format!("\n  shot xG {:.2} score {:+.3}", sh.xg, sh.score);
            }
            s += "\n";
            for p in o.passes.iter().take(5) {
                s += &format!(
                    "  {:<7} -> {:<16} {:+.3} (risk {:.2}, style {:+.3})\n",
                    format!("{:?}", p.kind),
                    truncate(sim.name(p.receiver), 16),
                    p.score,
                    p.risk,
                    p.style
                );
            }
            s += "\n";
        }
    }

    if let Some(id) = sb.selected {
        let p = st.player(id);
        let profile = &sim.squads[id.team().index()].players[id.slot()];
        let d = sim.decisions.get(id.index());
        s += &format!(
            "Selected: {} ({}, {:?})\n",
            profile.name,
            p.role.short_name(),
            p.team
        );
        if let Some(style) = profile.playing_style {
            s += &format!("  {}\n", style.name());
        }
        if let Some(d) = d {
            s += &format!("  doing: {}  {:?}\n", d.activity.label(), d.speed);
        }
        s += &format!(
            "  energy {:.0}%  speed {:.1}/{:.1} m/s\n",
            100.0 * p.energy,
            p.attrs.top_speed,
            p.base_top_speed
        );
        let b = &p.behaviour;
        s += &format!(
            "  dribble {:.2} press {:.2} run-behind {:.2} shoot>={:.2} xG\n  range {:.0}m  tempo {:.2} track-back {:.2}\n",
            b.dribble_bias, b.press_willingness, b.run_in_behind, b.shot_threshold, b.shot_range, b.tempo, b.track_back
        );
        let tags = profile.signature_traits();
        if !tags.is_empty() {
            s += &format!("  {}\n", tags.join(" | "));
        }
        s += "\n";
    }

    s += "Recent events:\n";
    for (tick, e) in sim.events.iter().rev().take(8) {
        let t = *tick as f32 * st.dt;
        let line = match *e {
            MatchEvent::Pass { from, to, kind } => {
                format!("{:?} pass {} -> {}", kind, sim.name(from), sim.name(to))
            }
            MatchEvent::PassCompleted { .. } => continue,
            MatchEvent::Interception { by, .. } => format!("Interception by {}", sim.name(by)),
            MatchEvent::Tackle { by, won, .. } => format!(
                "Tackle by {} ({})",
                sim.name(by),
                if won { "won" } else { "beaten" }
            ),
            MatchEvent::Shot { by, xg, .. } => format!("SHOT {} (xG {:.2})", sim.name(by), xg),
            MatchEvent::Save { by, held } => format!(
                "Save by {}{}",
                sim.name(by),
                if held { "" } else { " (parried)" }
            ),
            MatchEvent::Goal { team, scorer } => {
                format!(
                    "GOAL {:?}{}",
                    team,
                    scorer
                        .map(|s| format!(" - {}", sim.name(s)))
                        .unwrap_or_default()
                )
            }
            MatchEvent::Clearance { by } => format!("Clearance by {}", sim.name(by)),
            MatchEvent::Offside { player } => format!("Offside: {}", sim.name(player)),
            MatchEvent::OutOfPlay { restart, team } => format!("{:?} to {:?}", restart, team),
            MatchEvent::PossessionChange { team } => format!("Possession -> {:?}", team),
            MatchEvent::Foul { by, on, .. } => {
                format!("Foul by {} on {}", sim.name(by), sim.name(on))
            }
            MatchEvent::Advantage { team } => format!("Advantage {:?}", team),
            MatchEvent::AdvantageCalledBack { team } => {
                format!("Advantage called back: free kick {:?}", team)
            }
            MatchEvent::YellowCard { player } => format!("YELLOW {}", sim.name(player)),
            MatchEvent::RedCard {
                player,
                second_yellow,
            } => {
                format!(
                    "RED {}{}",
                    sim.name(player),
                    if second_yellow { " (2nd yellow)" } else { "" }
                )
            }
            MatchEvent::PenaltyAwarded { team } => format!("PENALTY to {:?}", team),
            MatchEvent::HalfTime => "Half time".into(),
            MatchEvent::FullTime => "Full time".into(),
            MatchEvent::Substitution {
                team,
                slot,
                bench_index,
            } => format!(
                "SUB {:?}: {} on for {}",
                team,
                sim.squads[team.index()].players[slot as usize].name,
                sim.squads[team.index()].bench[bench_index as usize].name
            ),
        };
        s += &format!("  {:>5.1}s {}\n", t, line);
    }
    if text.0 != s {
        text.0 = s;
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

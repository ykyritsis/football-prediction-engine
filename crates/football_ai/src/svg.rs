//! Headless SVG snapshots of a match, for debugging, docs and CI artefacts.
//!
//! Draws the pitch, players (coloured by team, labelled with role), each
//! player's current movement target, marking links, the pressing player and
//! the ball (with a pass/shot line when in flight). No dependencies.

use crate::decision::{Activity, Decision};
use crate::math::Vec2;
use crate::pitch::*;
use crate::plan::TeamPlan;
use crate::state::MatchState;
use std::fmt::Write;

const SCALE: f32 = 8.0;
const MARGIN: f32 = 4.0;

fn px(p: Vec2) -> (f32, f32) {
    ((p.x + HALF_LENGTH + MARGIN) * SCALE, (HALF_WIDTH - p.y + MARGIN) * SCALE)
}

/// Render `state` (optionally with the plans/decisions that produced it).
pub fn render(state: &MatchState, plans: Option<&[TeamPlan; 2]>, decisions: &[Decision], caption: &str) -> String {
    let w = (PITCH_LENGTH + 2.0 * MARGIN) * SCALE;
    let h = (PITCH_WIDTH + 2.0 * MARGIN) * SCALE + 40.0;
    let mut s = String::new();
    let _ = write!(s, r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="sans-serif">"#);
    let _ = write!(s, r##"<rect width="{w}" height="{h}" fill="#2e7d32"/>"##);
    // Pitch markings.
    let line = |s: &mut String, a: Vec2, b: Vec2| {
        let (x1, y1) = px(a);
        let (x2, y2) = px(b);
        let _ = write!(s, r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="white" stroke-width="2"/>"#);
    };
    let rect = |s: &mut String, a: Vec2, b: Vec2| {
        let (x1, y1) = px(Vec2::new(a.x.min(b.x), a.y.max(b.y)));
        let (x2, y2) = px(Vec2::new(a.x.max(b.x), a.y.min(b.y)));
        let _ = write!(s, r#"<rect x="{x1}" y="{y1}" width="{}" height="{}" fill="none" stroke="white" stroke-width="2"/>"#, x2 - x1, y2 - y1);
    };
    rect(&mut s, Vec2::new(-HALF_LENGTH, -HALF_WIDTH), Vec2::new(HALF_LENGTH, HALF_WIDTH));
    line(&mut s, Vec2::new(0.0, -HALF_WIDTH), Vec2::new(0.0, HALF_WIDTH));
    for sx in [-1.0, 1.0] {
        rect(&mut s, Vec2::new(sx * HALF_LENGTH, -PENALTY_AREA_HALF_WIDTH), Vec2::new(sx * (HALF_LENGTH - PENALTY_AREA_DEPTH), PENALTY_AREA_HALF_WIDTH));
        rect(&mut s, Vec2::new(sx * HALF_LENGTH, -GOAL_HALF_WIDTH), Vec2::new(sx * (HALF_LENGTH + 1.5), GOAL_HALF_WIDTH));
    }
    let (cx, cy) = px(Vec2::ZERO);
    let _ = write!(s, r#"<circle cx="{cx}" cy="{cy}" r="{}" fill="none" stroke="white" stroke-width="2"/>"#, CENTRE_CIRCLE_RADIUS * SCALE);

    // Plan overlays: marking links and press lines.
    if let Some(plans) = plans {
        for plan in plans {
            for m in &plan.marking {
                let (x1, y1) = px(state.player(m.marker).pos);
                let (x2, y2) = px(state.player(m.target).pos);
                let _ = write!(s, r##"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#ffeb3b" stroke-width="1.5" stroke-dasharray="4 3" opacity="0.8"/>"##);
            }
        }
    }
    // Movement targets.
    for d in decisions {
        let p = state.player(d.player);
        let (x1, y1) = px(p.pos);
        let (x2, y2) = px(d.target);
        let colour = match d.activity {
            Activity::Press(_) | Activity::Contain(_) => "#ff1744",
            Activity::Support(_) => "#00e5ff",
            Activity::Cover(_) => "#ff9100",
            _ => "#ffffff",
        };
        let _ = write!(s, r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{colour}" stroke-width="1" opacity="0.6"/>"#);
    }
    // Ball path.
    if let Some(pass) = state.ball.pass {
        let (x1, y1) = px(state.ball.pos);
        let (x2, y2) = px(pass.target);
        let _ = write!(s, r##"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#e0e0e0" stroke-width="2" stroke-dasharray="6 4"/>"##);
    }
    if let Some(shot) = state.ball.shot {
        let (x1, y1) = px(state.ball.pos);
        let (x2, y2) = px(shot.target);
        let _ = write!(s, r##"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#ff5252" stroke-width="3"/>"##);
    }
    // Players.
    for p in &state.players {
        let (x, y) = px(p.pos);
        let fill = match (p.team, p.is_goalkeeper()) {
            (TeamSide::Home, false) => "#1565c0",
            (TeamSide::Home, true) => "#4fc3f7",
            (TeamSide::Away, false) => "#c62828",
            (TeamSide::Away, true) => "#ffb74d",
        };
        let stroke = if state.ball.owner == Some(p.id) { "#ffff00" } else { "#000000" };
        let _ = write!(s, r#"<circle cx="{x}" cy="{y}" r="9" fill="{fill}" stroke="{stroke}" stroke-width="2"/>"#);
        let (fx, fy) = px(p.pos + p.facing * 1.6);
        let _ = write!(s, r#"<line x1="{x}" y1="{y}" x2="{fx}" y2="{fy}" stroke="black" stroke-width="2"/>"#);
        let _ = write!(s, r#"<text x="{x}" y="{}" font-size="9" fill="white" text-anchor="middle">{}{}</text>"#, y - 12.0, p.role.short_name(), p.id.0);
    }
    let (bx, by) = px(state.ball.pos);
    let _ = write!(s, r#"<circle cx="{bx}" cy="{by}" r="5" fill="white" stroke="black" stroke-width="1.5"/>"#);
    let _ = write!(s, r#"<text x="10" y="{}" font-size="16" fill="white">{}</text>"#, h - 14.0, escape(caption));
    s.push_str("</svg>");
    s
}

fn escape(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

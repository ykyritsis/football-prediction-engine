//! Pitch, players, ball and tactical overlays.

use crate::{to_screen, Sandbox, SCALE};
use bevy::prelude::*;
use football_ai::decision::Activity;
use football_ai::math::Vec2 as FVec2;
use football_ai::pitch::*;
use football_ai::PlayerId;

#[derive(Component)]
pub struct PlayerMarker(pub PlayerId);

#[derive(Component)]
pub struct PlayerLabel(pub PlayerId);

#[derive(Component)]
pub struct BallMarker;

pub const HOME: Color = Color::srgb(0.15, 0.40, 0.85);
pub const HOME_GK: Color = Color::srgb(0.35, 0.80, 0.95);
pub const AWAY: Color = Color::srgb(0.85, 0.20, 0.20);
pub const AWAY_GK: Color = Color::srgb(1.0, 0.70, 0.30);

const GRASS: Color = Color::srgb(0.16, 0.45, 0.20);
const LINE: Color = Color::srgba(1.0, 1.0, 1.0, 0.85);
const MARK: Color = Color::srgb(1.0, 0.92, 0.25);
const PRESS: Color = Color::srgb(1.0, 0.15, 0.25);
const COVER: Color = Color::srgb(1.0, 0.57, 0.0);
const SUPPORT: Color = Color::srgb(0.0, 0.9, 1.0);

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    sb: Res<Sandbox>,
) {
    commands.spawn(Camera2d);
    // Grass.
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(
            (PITCH_LENGTH + 8.0) * SCALE,
            (PITCH_WIDTH + 8.0) * SCALE,
        ))),
        MeshMaterial2d(materials.add(GRASS)),
        Transform::from_translation(crate::PITCH_ORIGIN.extend(-10.0)),
    ));
    let body = meshes.add(Circle::new(0.85 * SCALE));
    for p in &sb.sim.state.players {
        let colour = match (p.team, p.is_goalkeeper()) {
            (TeamSide::Home, false) => HOME,
            (TeamSide::Home, true) => HOME_GK,
            (TeamSide::Away, false) => AWAY,
            (TeamSide::Away, true) => AWAY_GK,
        };
        commands.spawn((
            PlayerMarker(p.id),
            Mesh2d(body.clone()),
            MeshMaterial2d(materials.add(colour)),
            Transform::from_translation(to_screen(p.pos).extend(1.0)),
        ));
        commands.spawn((
            PlayerLabel(p.id),
            Text2d::new(""),
            TextFont::from_font_size(11.0),
            TextColor(Color::WHITE),
            Transform::from_translation(to_screen(p.pos).extend(3.0)),
        ));
    }
    commands.spawn((
        BallMarker,
        Mesh2d(meshes.add(Circle::new(0.45 * SCALE))),
        MeshMaterial2d(materials.add(Color::WHITE)),
        Transform::from_translation(to_screen(sb.sim.state.ball.pos).extend(2.0)),
    ));
}

type PlayerBodies<'w, 's> = Query<
    'w,
    's,
    (&'static PlayerMarker, &'static mut Transform),
    (Without<BallMarker>, Without<PlayerLabel>),
>;
type PlayerLabels<'w, 's> = Query<
    'w,
    's,
    (
        &'static PlayerLabel,
        &'static mut Transform,
        &'static mut Text2d,
    ),
    Without<PlayerMarker>,
>;
type BallBody<'w, 's> = Query<
    'w,
    's,
    &'static mut Transform,
    (
        With<BallMarker>,
        Without<PlayerMarker>,
        Without<PlayerLabel>,
    ),
>;

pub fn sync_entities(
    sb: Res<Sandbox>,
    mut players: PlayerBodies,
    mut labels: PlayerLabels,
    mut ball: BallBody,
) {
    let st = &sb.sim.state;
    for (m, mut tf) in &mut players {
        let p = st.player(m.0);
        tf.translation = to_screen(p.pos).extend(1.0);
    }
    for (l, mut tf, mut text) in &mut labels {
        let p = st.player(l.0);
        tf.translation = (to_screen(p.pos) + Vec2::new(0.0, 1.7 * SCALE)).extend(3.0);
        let label = format!("{}{}", p.role.short_name(), p.id.0);
        if text.0 != label {
            text.0 = label;
        }
    }
    if let Ok(mut tf) = ball.single_mut() {
        tf.translation = to_screen(st.ball.pos).extend(2.0);
    }
}

fn line(g: &mut Gizmos, a: FVec2, b: FVec2, c: Color) {
    g.line_2d(to_screen(a), to_screen(b), c);
}

fn pitch_markings(g: &mut Gizmos) {
    let o = crate::PITCH_ORIGIN;
    g.rect_2d(
        Isometry2d::from_translation(o),
        Vec2::new(PITCH_LENGTH, PITCH_WIDTH) * SCALE,
        LINE,
    );
    line(
        g,
        FVec2::new(0.0, -HALF_WIDTH),
        FVec2::new(0.0, HALF_WIDTH),
        LINE,
    );
    g.circle_2d(
        Isometry2d::from_translation(o),
        CENTRE_CIRCLE_RADIUS * SCALE,
        LINE,
    )
    .resolution(64);
    for sx in [-1.0f32, 1.0] {
        let box_centre = FVec2::new(sx * (HALF_LENGTH - PENALTY_AREA_DEPTH / 2.0), 0.0);
        g.rect_2d(
            Isometry2d::from_translation(to_screen(box_centre)),
            Vec2::new(PENALTY_AREA_DEPTH, PENALTY_AREA_HALF_WIDTH * 2.0) * SCALE,
            LINE,
        );
        let six = FVec2::new(sx * (HALF_LENGTH - 2.75), 0.0);
        g.rect_2d(
            Isometry2d::from_translation(to_screen(six)),
            Vec2::new(5.5, 18.32) * SCALE,
            LINE,
        );
        let goal = FVec2::new(sx * (HALF_LENGTH + 1.0), 0.0);
        g.rect_2d(
            Isometry2d::from_translation(to_screen(goal)),
            Vec2::new(2.0, GOAL_HALF_WIDTH * 2.0) * SCALE,
            LINE,
        );
        g.circle_2d(
            Isometry2d::from_translation(to_screen(FVec2::new(sx * (HALF_LENGTH - 11.0), 0.0))),
            0.2 * SCALE,
            LINE,
        );
    }
}

pub fn overlays(mut g: Gizmos, sb: Res<Sandbox>) {
    pitch_markings(&mut g);
    let st = &sb.sim.state;
    let o = sb.overlays;

    // Carrier and selection rings.
    if let Some(c) = st.ball.owner {
        g.circle_2d(
            Isometry2d::from_translation(to_screen(st.player(c).pos)),
            1.3 * SCALE,
            MARK,
        );
    }
    if let Some(sel) = sb.selected {
        g.circle_2d(
            Isometry2d::from_translation(to_screen(st.player(sel).pos)),
            1.7 * SCALE,
            Color::WHITE,
        );
    }
    // Facing ticks.
    for p in &st.players {
        g.line_2d(
            to_screen(p.pos),
            to_screen(p.pos + p.facing * 1.4),
            Color::BLACK,
        );
    }

    // Offside lines.
    if o.offside {
        for side in TeamSide::BOTH {
            let x = side
                .to_world(FVec2::new(st.offside_line_local(side), 0.0))
                .x;
            let c = if side == TeamSide::Home {
                HOME.with_alpha(0.5)
            } else {
                AWAY.with_alpha(0.5)
            };
            line(
                &mut g,
                FVec2::new(x, -HALF_WIDTH),
                FVec2::new(x, HALF_WIDTH),
                c,
            );
        }
    }

    let Some(plans) = sb.sim.plans.as_ref() else {
        return;
    };

    if o.shape {
        for plan in plans {
            let c = if plan.side == TeamSide::Home {
                HOME
            } else {
                AWAY
            };
            for (slot, &t) in plan.shape.iter().enumerate() {
                g.rect_2d(
                    Isometry2d::from_translation(to_screen(t)),
                    Vec2::splat(0.9 * SCALE),
                    c,
                );
                let pid = PlayerId::new(plan.side, slot);
                line(&mut g, st.player(pid).pos, t, c.with_alpha(0.35));
            }
        }
    }
    if o.marking {
        for plan in plans {
            for m in &plan.marking {
                let alpha = 0.35 + 0.6 * m.tightness;
                line(
                    &mut g,
                    st.player(m.marker).pos,
                    st.player(m.target).pos,
                    MARK.with_alpha(alpha),
                );
            }
        }
    }
    for d in &sb.sim.decisions {
        let p = st.player(d.player);
        let (show, colour) = match d.activity {
            Activity::Press(_) | Activity::Contain(_) => (o.pressing, PRESS),
            Activity::Cover(_) => (o.pressing, COVER),
            Activity::Support(_) => (o.support, SUPPORT),
            Activity::ReceivePass | Activity::ChaseBall => (o.support || o.pressing, Color::WHITE),
            _ => (o.targets, Color::srgba(1.0, 1.0, 1.0, 0.4)),
        };
        if show && p.pos.distance(d.target) > 0.5 {
            g.arrow_2d(to_screen(p.pos), to_screen(d.target), colour)
                .with_tip_length(0.8 * SCALE);
        }
        if o.pressing {
            if let Activity::Cover(t) = d.activity {
                if let Some(c) = st.carrier() {
                    line(&mut g, c.pos, st.player(t).pos, COVER.with_alpha(0.4));
                }
            }
        }
    }

    // Ball in flight.
    if let Some(pass) = st.ball.pass {
        line(
            &mut g,
            st.ball.pos,
            pass.target,
            Color::srgba(1.0, 1.0, 1.0, 0.7),
        );
        g.circle_2d(
            Isometry2d::from_translation(to_screen(pass.target)),
            0.6 * SCALE,
            Color::WHITE,
        );
    }
    if let Some(shot) = st.ball.shot {
        line(&mut g, st.ball.pos, shot.target, PRESS);
    }

    // Pass candidates for the carrier, best in green.
    if o.passes {
        if let Some(c) = st.carrier() {
            let plan = &plans[c.team.index()];
            let opts = football_ai::decision::carrier_options(st, plan, c.id);
            let n = opts.passes.len().min(6);
            for (i, p) in opts.passes.iter().take(n).enumerate() {
                let t = i as f32 / n.max(1) as f32;
                let colour = Color::srgba(0.2 + 0.8 * t, 1.0 - 0.7 * t, 0.3, 0.9 - 0.5 * t);
                line(&mut g, c.pos, p.target, colour);
                g.circle_2d(
                    Isometry2d::from_translation(to_screen(p.target)),
                    (0.3 + 0.5 * (1.0 - p.risk)) * SCALE,
                    colour,
                );
            }
            if let Some(d) = opts.dribbles.first() {
                g.arrow_2d(
                    to_screen(c.pos),
                    to_screen(d.target),
                    Color::srgb(0.8, 0.5, 1.0),
                )
                .with_tip_length(0.6 * SCALE);
            }
        }
    }
}

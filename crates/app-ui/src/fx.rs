//! Lightweight chrome effects: soft glow, auth dialog morph (suck / spit), ripples.
//!
//! Event-driven only — no always-on full FPS loop. Callers request repaint
//! while [`FxLayer::is_active`].

use egui::{Color32, Context, Id, LayerId, Order, Painter, Pos2, Rect, Vec2};
use std::time::{Duration, Instant};

const RIPPLE_LIFE: Duration = Duration::from_millis(560);
const MORPH_LIFE: Duration = Duration::from_millis(700);
/// Match host-tab Connected status light green.
const STATUS_GREEN: Color32 = Color32::from_rgb(40, 160, 90);
const RIPPLE_GREEN_SOFT: Color32 = Color32::from_rgb(70, 190, 120);
/// Match host-tab Disconnected status light gray.
const STATUS_GRAY: Color32 = Color32::from_rgb(150, 154, 162);
const RIPPLE_GRAY_SOFT: Color32 = Color32::from_rgb(180, 184, 190);

#[derive(Clone)]
struct Ripple {
    origin: Pos2,
    born: Instant,
    color: Color32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MorphKind {
    /// Dialog → status light (auth success).
    Suck,
    /// Status light → dialog (reconnect via gray light).
    Spit,
}

struct AuthMorph {
    from: Rect,
    to: Rect,
    born: Instant,
    kind: MorphKind,
}

/// App-owned motion/effects layer.
#[derive(Default)]
pub struct FxLayer {
    ripples: Vec<Ripple>,
    pending_suck_from: Option<Rect>,
    morph: Option<AuthMorph>,
    spit_just_finished: bool,
}

impl FxLayer {
    pub fn is_active(&self) -> bool {
        self.pending_suck_from.is_some()
            || self.morph.is_some()
            || !self.ripples.is_empty()
            || self.spit_just_finished
    }

    /// Begin dialog → status-light morph after auth success.
    pub fn begin_auth_suck(&mut self, from: Rect) {
        if from.width() < 8.0 || from.height() < 8.0 {
            return;
        }
        self.pending_suck_from = Some(from);
        self.spit_just_finished = false;
    }

    /// Resolve suck target as the active host status light.
    pub fn settle_auth_target(&mut self, target: Rect) {
        if target.width() < 2.0 || target.height() < 2.0 {
            return;
        }
        if let Some(from) = self.pending_suck_from.take() {
            self.morph = Some(AuthMorph {
                from,
                to: target,
                born: Instant::now(),
                kind: MorphKind::Suck,
            });
        }
    }

    /// Begin status-light → dialog morph (gray reconnect). Emits a gray ripple at the light.
    pub fn begin_auth_spit(&mut self, from_light: Rect, to_dialog: Rect) {
        if from_light.width() < 2.0 || from_light.height() < 2.0 {
            return;
        }
        if to_dialog.width() < 8.0 || to_dialog.height() < 8.0 {
            return;
        }
        self.pending_suck_from = None;
        self.spit_just_finished = false;
        let now = Instant::now();
        let at = from_light.center();
        self.ripples.push(Ripple {
            origin: at,
            born: now,
            color: STATUS_GRAY,
        });
        self.ripples.push(Ripple {
            origin: at,
            born: now,
            color: RIPPLE_GRAY_SOFT,
        });
        self.morph = Some(AuthMorph {
            from: from_light,
            to: to_dialog,
            born: now,
            kind: MorphKind::Spit,
        });
    }

    /// True once after a spit morph completes (consumed).
    pub fn take_spit_finished(&mut self) -> bool {
        std::mem::take(&mut self.spit_just_finished)
    }

    /// Tick + paint morph / ripples.
    pub fn paint_overlay(&mut self, ctx: &Context) {
        if self.pending_suck_from.is_some() {
            // Fallback: approximate status-light position in the host list column.
            let screen = ctx.screen_rect();
            let fallback = Rect::from_center_size(
                Pos2::new(screen.min.x + 255.0, screen.min.y + 96.0),
                Vec2::splat(8.0),
            );
            self.settle_auth_target(fallback);
        }

        let now = Instant::now();
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("vsterm_fx")));

        let finished = self.morph.as_ref().and_then(|m| {
            if now.duration_since(m.born) >= MORPH_LIFE {
                Some((m.kind, m.to.center(), m.from.center()))
            } else {
                None
            }
        });
        if let Some((kind, to_c, from_c)) = finished {
            self.morph = None;
            match kind {
                MorphKind::Suck => {
                    self.ripples.push(Ripple {
                        origin: to_c,
                        born: now,
                        color: STATUS_GREEN,
                    });
                    self.ripples.push(Ripple {
                        origin: to_c,
                        born: now,
                        color: RIPPLE_GREEN_SOFT,
                    });
                }
                MorphKind::Spit => {
                    let _ = from_c;
                    self.spit_just_finished = true;
                }
            }
        } else if let Some(morph) = &self.morph {
            paint_auth_morph(&painter, morph, now);
        }

        self.ripples.retain(|r| now.duration_since(r.born) < RIPPLE_LIFE);
        for r in &self.ripples {
            paint_ripple(&painter, r, now);
        }
    }
}

/// Soft bloom under a status/icon glyph (layered discs, no GPU blur).
pub fn soft_glow(painter: &Painter, center: Pos2, core_r: f32, color: Color32) {
    let (r, g, b) = (color.r(), color.g(), color.b());
    painter.circle_filled(
        center,
        core_r * 2.6,
        Color32::from_rgba_unmultiplied(r, g, b, 22),
    );
    painter.circle_filled(
        center,
        core_r * 1.7,
        Color32::from_rgba_unmultiplied(r, g, b, 48),
    );
    painter.circle_filled(center, core_r, color);
}

/// Soft glow with a gentle pulse (Connecting state).
pub fn soft_glow_pulse(painter: &Painter, center: Pos2, core_r: f32, color: Color32, t: f32) {
    let pulse = 0.85 + 0.15 * (t * std::f32::consts::TAU).sin();
    let (r, g, b) = (color.r(), color.g(), color.b());
    painter.circle_filled(
        center,
        core_r * 2.8 * pulse,
        Color32::from_rgba_unmultiplied(r, g, b, (28.0 * pulse) as u8),
    );
    painter.circle_filled(
        center,
        core_r * 1.8,
        Color32::from_rgba_unmultiplied(r, g, b, 55),
    );
    painter.circle_filled(center, core_r, color);
}

/// Typical auth dialog footprint when we have not painted one yet.
pub fn estimated_auth_dialog_rect(screen: Rect) -> Rect {
    Rect::from_center_size(screen.center(), Vec2::new(420.0, 268.0))
}

fn paint_auth_morph(painter: &Painter, morph: &AuthMorph, now: Instant) {
    let life = MORPH_LIFE.as_secs_f32();
    let t = (now.duration_since(morph.born).as_secs_f32() / life).clamp(0.0, 1.0);
    let ease = t * t * (3.0 - 2.0 * t); // smoothstep — natural spit / suck

    let min = morph.from.min.lerp(morph.to.min, ease);
    let max = morph.from.max.lerp(morph.to.max, ease);
    let r = Rect::from_min_max(min, max);

    let (alpha, stroke_rgb) = match morph.kind {
        MorphKind::Suck => {
            let a = ((1.0 - ease.powf(0.7)) * 230.0) as u8;
            (a, STATUS_GREEN)
        }
        MorphKind::Spit => {
            // Fade in as the card expands out of the light.
            let a = (ease.powf(0.55) * 235.0) as u8;
            (a, STATUS_GRAY)
        }
    };
    let rounding = 4.0 + (1.0 - ease) * 4.0;

    painter.rect_filled(
        r,
        rounding,
        Color32::from_rgba_unmultiplied(250, 251, 253, alpha),
    );
    painter.rect_stroke(
        r,
        rounding,
        egui::Stroke::new(
            (1.0 + (1.0 - ease) * 1.2).max(0.8),
            Color32::from_rgba_unmultiplied(
                stroke_rgb.r(),
                stroke_rgb.g(),
                stroke_rgb.b(),
                (alpha as f32 * 0.75) as u8,
            ),
        ),
        egui::StrokeKind::Outside,
    );
}

fn paint_ripple(painter: &Painter, r: &Ripple, now: Instant) {
    let age = now.duration_since(r.born).as_secs_f32();
    let life = RIPPLE_LIFE.as_secs_f32();
    let t = (age / life).clamp(0.0, 1.0);
    let (cr, cg, cb) = (r.color.r(), r.color.g(), r.color.b());

    for (i, (scale, alpha0, width)) in [
        (1.0_f32, 120_u8, 2.4_f32),
        (0.72, 75, 1.7),
        (0.5, 40, 1.2),
    ]
    .into_iter()
    .enumerate()
    {
        let ring_t = (t - i as f32 * 0.08).clamp(0.0, 1.0);
        let ring_ease = 1.0 - (1.0 - ring_t) * (1.0 - ring_t);
        let radius = 5.0 + 28.0 * ring_ease * scale;
        let fade = (1.0 - ring_t).powf(1.1);
        let alpha = (alpha0 as f32 * fade) as u8;
        if alpha < 4 {
            continue;
        }
        painter.circle_stroke(
            r.origin,
            radius,
            egui::Stroke::new(
                width,
                Color32::from_rgba_unmultiplied(cr, cg, cb, alpha),
            ),
        );
    }
}

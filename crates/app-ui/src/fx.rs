//! Lightweight chrome effects: soft glow, auth dialog morph (suck / spit),
//! and a short-lived spark ring around a freshly shown dialog.
//!
//! Event-driven only — no always-on full FPS loop. Callers request repaint
//! while [`FxLayer::is_active`].

use egui::{Color32, Context, Id, LayerId, Order, Painter, Pos2, Rect, Vec2};
use std::time::{Duration, Instant};

const MORPH_LIFE: Duration = Duration::from_millis(760);
/// Sparks linger around the dialog for this long once it appears.
const SPARK_LIFE: Duration = Duration::from_millis(2500);
/// Number of fading afterimages behind the flying card.
const TRAIL: usize = 7;
/// Swirling spark particles around the dialog.
const SPARKS: usize = 9;
/// Fallback when the host has no `color_tag` (matches session-list default accent).
pub const DEFAULT_ACCENT: Color32 = Color32::from_rgb(60, 120, 210);

/// User preference for connect / reconnect motion (persisted in `config.yaml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectFxMode {
    /// Current suck / spit trail morph + dialog sparks.
    #[default]
    Trail,
    /// Reserved — not implemented yet; treated as Off until wired.
    Shatter,
    /// No connect / reconnect motion.
    Off,
}

impl ConnectFxMode {
    pub fn code(self) -> &'static str {
        match self {
            Self::Trail => "trail",
            Self::Shatter => "shatter",
            Self::Off => "off",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code.trim().to_ascii_lowercase().as_str() {
            "shatter" => Self::Shatter,
            "off" | "none" | "disabled" => Self::Off,
            _ => Self::Trail,
        }
    }

    /// Whether suck / spit / dialog-spark connect motion should run.
    pub fn motion_enabled(self) -> bool {
        matches!(self, Self::Trail)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MorphKind {
    /// Dialog → status light (auth success).
    Suck,
    /// Status light → dialog (reconnect via status light).
    Spit,
}

struct AuthMorph {
    from: Rect,
    to: Rect,
    born: Instant,
    kind: MorphKind,
    accent: Color32,
}

struct PendingSuck {
    from: Rect,
    accent: Color32,
}

/// Twinkling sparks orbiting a dialog after it renders.
struct SparkField {
    rect: Rect,
    born: Instant,
    accent: Color32,
    seed: u64,
}

/// App-owned motion/effects layer.
#[derive(Default)]
pub struct FxLayer {
    pending_suck: Option<PendingSuck>,
    morph: Option<AuthMorph>,
    sparks: Option<SparkField>,
    spit_just_finished: bool,
}

impl FxLayer {
    pub fn is_active(&self) -> bool {
        self.pending_suck.is_some()
            || self.morph.is_some()
            || self.sparks.is_some()
            || self.spit_just_finished
    }

    /// Begin dialog → status-light morph after auth success.
    pub fn begin_auth_suck(&mut self, from: Rect, accent: Color32) {
        if from.width() < 8.0 || from.height() < 8.0 {
            return;
        }
        self.pending_suck = Some(PendingSuck { from, accent });
        self.spit_just_finished = false;
    }

    /// Resolve suck target as the active host status light.
    pub fn settle_auth_target(&mut self, target: Rect) {
        if target.width() < 2.0 || target.height() < 2.0 {
            return;
        }
        if let Some(PendingSuck { from, accent }) = self.pending_suck.take() {
            self.morph = Some(AuthMorph {
                from,
                to: target,
                born: Instant::now(),
                kind: MorphKind::Suck,
                accent,
            });
        }
    }

    /// Begin status-light → dialog morph.
    pub fn begin_auth_spit(&mut self, from_light: Rect, to_dialog: Rect, accent: Color32) {
        if from_light.width() < 2.0 || from_light.height() < 2.0 {
            return;
        }
        if to_dialog.width() < 8.0 || to_dialog.height() < 8.0 {
            return;
        }
        self.pending_suck = None;
        self.spit_just_finished = false;
        self.morph = Some(AuthMorph {
            from: from_light,
            to: to_dialog,
            born: Instant::now(),
            kind: MorphKind::Spit,
            accent,
        });
    }

    /// Spawn a 2-second spark ring around a dialog that just appeared.
    pub fn begin_dialog_sparks(&mut self, rect: Rect, accent: Color32) {
        if rect.width() < 8.0 || rect.height() < 8.0 {
            return;
        }
        let now = Instant::now();
        self.sparks = Some(SparkField {
            rect,
            born: now,
            accent,
            seed: seed_from(now),
        });
    }

    /// True once after a spit morph completes (consumed).
    pub fn take_spit_finished(&mut self) -> bool {
        std::mem::take(&mut self.spit_just_finished)
    }

    /// Tick + paint morph / sparks.
    pub fn paint_overlay(&mut self, ctx: &Context) {
        if self.pending_suck.is_some() {
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

        if let Some(m) = &self.morph {
            if now.duration_since(m.born) >= MORPH_LIFE {
                if m.kind == MorphKind::Spit {
                    self.spit_just_finished = true;
                }
                self.morph = None;
            } else {
                paint_auth_morph(&painter, m, now);
            }
        }

        if let Some(s) = &self.sparks {
            if now.duration_since(s.born) >= SPARK_LIFE {
                self.sparks = None;
            } else {
                paint_sparks(&painter, s, now);
            }
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
    Rect::from_center_size(screen.center(), Vec2::new(420.0, 232.0))
}

/// Resolve host accent from session `color_tag`, falling back to blue.
pub fn accent_from_tag(tag: Option<&str>) -> Color32 {
    tag.and_then(parse_hex_color).unwrap_or(DEFAULT_ACCENT)
}

fn parse_hex_color(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}

fn seed_from(now: Instant) -> u64 {
    let n = now.elapsed().as_nanos() as u64;
    n ^ 0x9E37_79B9_7F4A_7C15
}

/// Deterministic pseudo-random in `0.0..1.0` from a seed + index.
fn rnd(seed: u64, i: u64) -> f32 {
    let mut x = seed
        .wrapping_add(i.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(0x1234_5678_9ABC_DEF0);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

fn ease_out_cubic(t: f32) -> f32 {
    let u = 1.0 - t;
    1.0 - u * u * u
}

/// Ease-out with a gentle overshoot (spit "pop").
fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158_f32;
    let c3 = c1 + 1.0;
    let u = t - 1.0;
    1.0 + c3 * u * u * u + c1 * u * u
}

fn qbez(a: Pos2, ctrl: Pos2, b: Pos2, t: f32) -> Pos2 {
    let u = 1.0 - t;
    let x = u * u * a.x + 2.0 * u * t * ctrl.x + t * t * b.x;
    let y = u * u * a.y + 2.0 * u * t * ctrl.y + t * t * b.y;
    Pos2::new(x, y)
}

/// Curved center + half-size of the flying card at raw progress `t`.
fn morph_state(m: &AuthMorph, t: f32) -> (Pos2, Vec2) {
    let t = t.clamp(0.0, 1.0);
    let (pos_e, size_e) = match m.kind {
        MorphKind::Suck => (ease_in_cubic(t), t.powf(1.7)),
        MorphKind::Spit => (ease_out_back(t), ease_out_cubic(t)),
    };
    let a = m.from.center();
    let b = m.to.center();
    let d = b - a;
    let len = d.length().max(1.0);
    let n = Vec2::new(-d.y, d.x) / len;
    let bow = (len * 0.16).min(90.0);
    let ctrl = a.lerp(b, 0.5) + n * bow;
    let center = qbez(a, ctrl, b, pos_e);

    let fs = m.from.size();
    let ts = m.to.size();
    let half = Vec2::new(fs.x + (ts.x - fs.x) * size_e, fs.y + (ts.y - fs.y) * size_e) * 0.5;
    // Subtle squash-and-stretch that peaks mid-flight.
    let wobble = (std::f32::consts::PI * t).sin();
    let half = Vec2::new(half.x * (1.0 + 0.10 * wobble), half.y * (1.0 - 0.07 * wobble));
    (center, half)
}

fn rounded_card(painter: &Painter, center: Pos2, half: Vec2, rounding: f32, fill: Color32) {
    let rect = Rect::from_center_size(center, half * 2.0);
    painter.rect_filled(rect, rounding, fill);
}

fn paint_auth_morph(painter: &Painter, morph: &AuthMorph, now: Instant) {
    let life = MORPH_LIFE.as_secs_f32();
    let t = (now.duration_since(morph.born).as_secs_f32() / life).clamp(0.0, 1.0);
    let accent = morph.accent;

    // Overall card opacity envelope.
    let card_alpha = match morph.kind {
        MorphKind::Suck => (1.0 - t.powf(2.4)) * 235.0,
        MorphKind::Spit => (ease_out_cubic((t * 1.5).min(1.0))) * 235.0,
    };
    // Energy envelope for the trail: peaks mid-flight.
    let energy = (std::f32::consts::PI * t).sin().max(0.0);

    // 1) Fading afterimage trail — a colorful comet streak.
    for k in (1..=TRAIL).rev() {
        let tk = t - k as f32 * 0.045;
        if tk <= 0.0 {
            continue;
        }
        let (c, half) = morph_state(morph, tk);
        let shrink = 0.72 + 0.28 * (1.0 - k as f32 / TRAIL as f32);
        let rounding = 4.0 + half.x.min(half.y) * 0.12;
        let fade = (1.0 - k as f32 / (TRAIL as f32 + 1.0)) * energy;
        let a = (card_alpha * 0.16 * fade) as u8;
        if a < 3 {
            continue;
        }
        rounded_card(
            painter,
            c,
            half * shrink,
            rounding,
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), a),
        );
    }

    // 2) The main card: soft shadow, glass fill, accent sheen + stroke.
    let (center, half) = morph_state(morph, t);
    let rounding = 5.0 + half.x.min(half.y) * 0.10;
    let a_main = card_alpha.clamp(0.0, 255.0);

    let shadow = Rect::from_center_size(center + Vec2::new(0.0, 4.0), half * 2.0 + Vec2::splat(2.0));
    painter.rect_filled(
        shadow,
        rounding,
        Color32::from_rgba_unmultiplied(18, 24, 40, (a_main * 0.16) as u8),
    );
    rounded_card(
        painter,
        center,
        half,
        rounding,
        Color32::from_rgba_unmultiplied(250, 251, 253, a_main as u8),
    );
    if half.y > 6.0 {
        let sheen = Rect::from_min_max(
            center - half + Vec2::new(3.0, 2.0),
            Pos2::new(
                center.x + half.x - 3.0,
                center.y - half.y + (half.y * 0.55).min(12.0) + 2.0,
            ),
        );
        painter.rect_filled(
            sheen,
            rounding * 0.6,
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), (a_main * 0.10) as u8),
        );
    }
    painter.rect_stroke(
        Rect::from_center_size(center, half * 2.0),
        rounding,
        egui::Stroke::new(
            1.4,
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), (a_main * 0.8) as u8),
        ),
        egui::StrokeKind::Outside,
    );
}

fn paint_sparks(painter: &Painter, field: &SparkField, now: Instant) {
    let life = SPARK_LIFE.as_secs_f32();
    let age = now.duration_since(field.born).as_secs_f32();
    let t = (age / life).clamp(0.0, 1.0);
    // Fade in quickly, hold, fade out at the tail.
    let fade_in = (t / 0.12).clamp(0.0, 1.0);
    let fade_out = ((1.0 - t) / 0.30).clamp(0.0, 1.0);
    let env = fade_in * fade_out;
    if env <= 0.01 {
        return;
    }

    let accent = field.accent;
    let center = field.rect.center();
    let a = field.rect.width() * 0.5 + 12.0;
    let b = field.rect.height() * 0.5 + 12.0;
    let tau = std::f32::consts::TAU;

    for i in 0..SPARKS {
        let ang0 = rnd(field.seed, i as u64) * tau;
        let phase = rnd(field.seed, i as u64 + 101) * tau;
        let speed = 0.5 + rnd(field.seed, i as u64 + 202) * 0.6;
        let ang = ang0 + age * speed;
        let wob = 0.96 + 0.06 * (age * 2.4 + phase).sin();
        let pos = center + Vec2::new(ang.cos() * a * wob, ang.sin() * b * wob);

        let twinkle = 0.45 + 0.55 * (age * 6.0 + phase).sin().abs();
        let alpha = (env * twinkle * 210.0) as u8;
        if alpha < 6 {
            continue;
        }
        let sr = 1.6 + 1.2 * twinkle;
        painter.circle_filled(
            pos,
            sr * 1.8,
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), alpha / 3),
        );
        painter.circle_filled(pos, sr, Color32::from_rgba_unmultiplied(255, 255, 255, alpha));
    }
}

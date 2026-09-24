//! Time-driven, spring-like motion for reveals (open/close, expand/collapse)
//! -- the "blobby"/overshoot easing HyprQuickShell's Dynamic Island uses on
//! its width/height property animations, ported for egui. See
//! `docs/design/2026-09-19_animated-reveals-transforms.md` (SDB repo) for
//! the research and the review that shaped this module's API.
//!
//! Everything below `presence`/`presence_with` is pure: time is passed in
//! rather than read from `std::time::Instant`, so tests (and headless
//! `egui::Context` runs) can drive it deterministically via
//! `RawInput::time`. Only the egui-glue functions at the bottom read
//! `ctx.input(|i| i.time)` and touch `ctx` memory.

/// Overshoots past 1.0 then settles back -- the "squish" behind Apple/
/// Material spring transitions and HyprQuickShell's Dynamic Island. Same
/// curve family as Hyprland's `easeOutBack` bezier (0.34, 1.56, 0.64, 1),
/// not the identical curve. `overshoot` is Penner's classic `c1`; 1.70158
/// gives the canonical ~10% peak. `t` is clamped to `[0, 1]` so callers
/// don't need to pre-clamp elapsed/duration themselves.
pub fn ease_out_back(t: f32, overshoot: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    let c1 = overshoot;
    let c3 = c1 + 1.0;
    1.0 + c3 * t.powi(3) + c1 * t.powi(2)
}

/// Plain ease-out, no overshoot -- used for collapse/close, where a bounce
/// on the way out reads as jittery rather than fluid (Chris, review
/// 2026-09-24). Delegates to egui's own curve so it stays in lockstep with
/// whatever else in the app uses `Context::animate_value`.
pub fn ease_out_cubic(t: f32) -> f32 {
    egui::emath::easing::cubic_out(t.clamp(0.0, 1.0))
}

/// Limits how far *above* `target` a value may sit -- e.g. a back-out
/// overshoot on a growing pixel quantity (the modal sheet's revealed
/// height, animating up from `0` to its natural height) that would
/// otherwise be a fixed ~10% of the tween's distance: ~3px on a 30px
/// flyout (fine), ~40px on a 400px sheet (not fine, per the design doc's
/// review). Values that haven't reached `target` yet (still approaching,
/// not overshooting) are returned unchanged -- this only clamps the
/// excess on the far side. `MotionSpec::EXPAND_OVERSHOOT_CAP_PX` (~6px) is
/// the cap this crate's own callers use.
pub fn cap_overshoot(value: f32, target: f32, cap: f32) -> f32 {
    value.min(target + cap)
}

/// Which easing curve a [`MotionSpec`] drives a [`Tween`] with. Kept
/// separate from `MotionSpec` (rather than folding `duration` into each
/// variant) so `retarget` call sites can match on curve alone when they
/// need to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    /// Penner back-out; `overshoot` is `c1` (see [`ease_out_back`]).
    BackOut { overshoot: f32 },
    /// Plain ease-out cubic, never exceeds the target.
    CubicOut,
}

/// A duration + curve pair for one leg of a reveal -- expand or collapse.
/// Two legs (not one shared spec) because the design's whole point is that
/// they're asymmetric: expand springs, collapse doesn't.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionSpec {
    /// Seconds. `<= 0.0` snaps instantly (see [`Tween::retarget`]).
    pub duration: f32,
    pub curve: Curve,
}

impl MotionSpec {
    /// ~200ms back-out, for opening/expanding. Penner's canonical
    /// `c1 = 1.70158` (~10% peak overshoot).
    pub const EXPAND: MotionSpec = MotionSpec {
        duration: 0.20,
        curve: Curve::BackOut { overshoot: 1.70158 },
    };

    /// ~140ms plain ease-out, for closing/collapsing. Faster than `EXPAND`
    /// and no overshoot -- closing should read as brisk, not springy.
    pub const COLLAPSE: MotionSpec = MotionSpec {
        duration: 0.14,
        curve: Curve::CubicOut,
    };

    /// The pixel cap `EXPAND`'s overshoot is meant to be used with via
    /// [`cap_overshoot`], for callers animating a pixel quantity (e.g. a
    /// sheet's revealed height) rather than an abstract `0..1` reveal.
    pub const EXPAND_OVERSHOOT_CAP_PX: f32 = 6.0;
}

/// Time-driven interpolation toward a retargetable value, using one of the
/// curves above. Pure -- `now` is passed into every method, nothing reads
/// the system clock. One property per instance (typically a size or a
/// `0..1` reveal amount), not a general animation system.
#[derive(Clone, Copy, Debug)]
pub struct Tween {
    from: f32,
    to: f32,
    start: f64,
    duration: f32,
    curve: Curve,
}

impl Tween {
    /// Starts settled at `value` -- `is_animating` is false and `value()`
    /// returns `value` until the first `retarget`.
    pub fn new(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            start: 0.0,
            duration: 0.0,
            curve: Curve::CubicOut,
        }
    }

    /// Retargets toward `to`, starting from the *current animated value*
    /// (not the old target) so an interruption never jumps -- e.g.
    /// retargeting mid-overshoot continues smoothly from wherever the
    /// overshoot currently is. A no-op if `to` already equals the current
    /// target (continuous per-frame callers -- window resize, Text Size
    /// changes -- must not restart the animation every frame). A
    /// `duration <= 0` spec snaps immediately to `to`.
    pub fn retarget(&mut self, now: f64, to: f32, spec: MotionSpec) {
        if (self.to - to).abs() <= f32::EPSILON {
            return;
        }
        if spec.duration <= 0.0 {
            self.snap(to);
            return;
        }
        self.from = self.value(now);
        self.to = to;
        self.start = now;
        self.duration = spec.duration;
        self.curve = spec.curve;
    }

    /// The interpolated value at `now`. Exact `target()` once `now` is past
    /// `start + duration`.
    pub fn value(&self, now: f64) -> f32 {
        if self.duration <= 0.0 {
            return self.to;
        }
        let t = ((now - self.start) as f32 / self.duration).clamp(0.0, 1.0);
        let eased = match self.curve {
            Curve::BackOut { overshoot } => ease_out_back(t, overshoot),
            Curve::CubicOut => ease_out_cubic(t),
        };
        self.from + (self.to - self.from) * eased
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    /// True strictly before the tween settles; false from the settling
    /// frame onward (so callers stop requesting repaints exactly once).
    pub fn is_animating(&self, now: f64) -> bool {
        self.duration > 0.0 && (now - self.start) < self.duration as f64
    }

    /// Jumps straight to `to`, no animation -- used for Reduce Motion and
    /// for a fresh `Tween`'s initial value.
    pub fn snap(&mut self, to: f32) {
        self.from = to;
        self.to = to;
        self.start = 0.0;
        self.duration = 0.0;
    }
}

/// Phase of a [`Presence`] state machine. `Closed` is the only phase where
/// the owning widget should stop rendering entirely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Opening,
    Open,
    Closing,
    Closed,
}

/// Pure open/close state machine for a thing that reveals and hides itself
/// (a modal sheet, a flyout, a toast) with an animated transition rather
/// than popping in/out. Fixes the specific gap `SquishAnim` in the design
/// doc didn't cover: a collapsing element must keep rendering,
/// non-interactive, until its close animation finishes, and reopening
/// mid-close must reverse rather than jump.
#[derive(Clone, Copy, Debug)]
pub struct Presence {
    tween: Tween,
    open: bool,
}

impl Presence {
    /// Starts fully closed (reveal `0.0`, `should_render` false) -- the
    /// correct state for an id `presence()` has never seen with `open ==
    /// false`. See `presence`/`presence_with` for the `open == true` case,
    /// which must animate rather than start revealed.
    pub fn closed() -> Self {
        Self {
            tween: Tween::new(0.0),
            open: false,
        }
    }

    /// Requests the open/closed state for the next transition. A no-op if
    /// `open` already matches the last call (continuous per-frame callers
    /// must not restart the animation). Reversing mid-transition (e.g.
    /// closing during Opening) reverses from the current reveal value --
    /// `Tween::retarget` already guarantees that continuity.
    pub fn set_open(&mut self, now: f64, open: bool, expand: MotionSpec, collapse: MotionSpec) {
        if open == self.open {
            return;
        }
        self.open = open;
        if open {
            self.tween.retarget(now, 1.0, expand);
        } else {
            self.tween.retarget(now, 0.0, collapse);
        }
    }

    /// Jumps straight to fully open or fully closed, no animation -- Reduce
    /// Motion.
    pub fn snap_open(&mut self, open: bool) {
        self.open = open;
        self.tween.snap(if open { 1.0 } else { 0.0 });
    }

    /// `0.0` closed .. `1.0` open; may briefly exceed `1.0` while Opening,
    /// since `MotionSpec::EXPAND` overshoots. Never exceeds `1.0` while
    /// Closing (`MotionSpec::COLLAPSE` doesn't overshoot).
    pub fn reveal(&self, now: f64) -> f32 {
        self.tween.value(now)
    }

    pub fn phase(&self, now: f64) -> Phase {
        if self.open {
            if self.tween.is_animating(now) {
                Phase::Opening
            } else {
                Phase::Open
            }
        } else if self.tween.is_animating(now) {
            Phase::Closing
        } else {
            Phase::Closed
        }
    }

    /// True in every phase except `Closed` -- a Closing element must keep
    /// rendering (non-interactively) until its collapse animation finishes.
    pub fn should_render(&self, now: f64) -> bool {
        self.phase(now) != Phase::Closed
    }

    /// False while Closing or Closed -- a closing sheet must not accept
    /// input or block input to the rest of the app.
    pub fn interactive(&self, now: f64) -> bool {
        matches!(self.phase(now), Phase::Opening | Phase::Open)
    }

    pub fn is_animating(&self, now: f64) -> bool {
        self.tween.is_animating(now)
    }
}

/// What `presence`/`presence_with` computed for this frame -- everything a
/// caller needs to decide whether/how to draw, without re-deriving it from
/// a raw `Presence`.
#[derive(Clone, Copy, Debug)]
pub struct PresenceFrame {
    pub reveal: f32,
    pub render: bool,
    pub interactive: bool,
    pub animating: bool,
}

fn presence_key(id: egui::Id) -> egui::Id {
    id.with("spatial_ui_kit::motion::presence")
}

fn reduce_motion_key() -> egui::Id {
    egui::Id::new("spatial_ui_kit::motion::reduce_motion")
}

/// Turns Reduce Motion on/off for every `presence`/`presence_with` call
/// against this `ctx` from now on -- stored in `ctx` memory like any other
/// app-wide UI setting this crate keeps there (see `theme::apply_text_scale`
/// for the same pattern). When on, transitions snap instead of animating,
/// which also makes tests deterministic.
pub fn set_reduce_motion(ctx: &egui::Context, reduce: bool) {
    ctx.data_mut(|d| d.insert_temp(reduce_motion_key(), reduce));
}

pub fn reduce_motion(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(reduce_motion_key()))
        .unwrap_or(false)
}

// =========================================================================
// Spring physics -- SwiftUI-style damped-spring motion.
//
// `Tween`/`MotionSpec` above are duration + easing-curve: a fixed-length
// animation that always finishes in exactly `duration` seconds no matter
// where it started. That's the wrong model for the "gently bouncy, Apple
// spring" feel Chris asked for (2026-09-24): real springs are
// *interruptible and velocity-preserving* -- retargeting mid-flight must
// continue from the current position **and** the current velocity, or the
// motion visibly kinks. A duration+curve tween can only ever restart from
// position (see `Tween::retarget`'s `self.from = self.value(now)`); it has
// no notion of velocity to carry over.
//
// `Spring` models a damped harmonic oscillator (`SwiftUI.Spring`'s own
// model) and solves it analytically -- `SpringTween` stores the oscillator's
// initial conditions at the moment of the last retarget (`x0`, `v0`) rather
// than integrating step by step, so it stays pure and time-injected exactly
// like `Tween` (no `Instant`, testable headless).
// =========================================================================

/// A damped-spring motion, modelled the way SwiftUI's `Spring` is:
/// `response` is roughly the spring's natural period in seconds (how fast
/// it would oscillate undamped), `damping_fraction` is the classic control-
/// theory damping ratio `zeta` -- `1.0` is critically damped (fastest
/// approach with no overshoot), `< 1.0` underdamped (bounces), `> 1.0`
/// overdamped (slower than critical, no overshoot).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub response: f32,
    pub damping_fraction: f32,
}

impl Spring {
    /// No bounce -- critically damped. SwiftUI's `.smooth` default
    /// (`response: 0.5, dampingFraction: 1.0`), trimmed slightly (0.45s)
    /// since Chris's brief is "not too much".
    pub const SMOOTH: Spring = Spring {
        response: 0.45,
        damping_fraction: 1.0,
    };

    /// A small bounce -- SwiftUI's `.snappy` (`response: 0.5,
    /// dampingFraction: 0.85`).
    pub const SNAPPY: Spring = Spring {
        response: 0.3,
        damping_fraction: 0.85,
    };

    /// More bounce -- SwiftUI's `.bouncy` (`response: 0.5,
    /// dampingFraction: 0.7`).
    pub const BOUNCY: Spring = Spring {
        response: 0.5,
        damping_fraction: 0.7,
    };

    /// SwiftUI's `.spring(duration:bounce:)` constructor. `bounce` is
    /// `-1.0..=1.0` (0 = no bounce); mapped the same way SwiftUI documents
    /// it -- `duration` becomes `response` directly, `damping_fraction =
    /// 1.0 - bounce`. Clamped so a pathological `bounce` can't produce a
    /// non-physical (zero or negative) damping fraction.
    pub fn new(duration: f32, bounce: f32) -> Spring {
        Spring {
            response: duration.max(0.001),
            damping_fraction: (1.0 - bounce).clamp(0.05, 2.0),
        }
    }

    fn omega(&self) -> f32 {
        (2.0 * std::f32::consts::PI) / self.response.max(1e-4)
    }

    /// Applies the global bounce-amount multiplier (see
    /// `set_bounce_amount`): `amount == 0.0` forces critical damping (no
    /// overshoot at all); `amount == 1.0` is this spring unchanged;
    /// `amount > 1.0` pushes damping *below* this spring's own fraction,
    /// i.e. more bounce. A spring that is already critically/over-damped
    /// (`damping_fraction >= 1.0`, e.g. `SMOOTH`) is unaffected at any
    /// `amount` -- there's no bounce in it to scale, which is exactly why
    /// `COLLAPSE`-style springs stay non-bouncy even when Chris turns the
    /// slider up.
    pub fn scaled_by_bounce(&self, amount: f32) -> Spring {
        let amount = amount.max(0.0);
        let inherent_bounce = (1.0 - self.damping_fraction).max(0.0);
        Spring {
            response: self.response,
            damping_fraction: (1.0 - inherent_bounce * amount).max(0.05),
        }
    }

    /// Analytic solution of the damped harmonic oscillator `y'' + 2*zeta*
    /// omega*y' + omega^2*y = 0` at elapsed time `t`, given displacement
    /// `x0` and velocity `v0` from the target (target held at `0`; callers
    /// add their own target back). Returns `(displacement, velocity)`.
    /// Handles all three regimes (under/critically/over-damped) since a
    /// caller-supplied `Spring` (via `new`/`scaled_by_bounce`) can land in
    /// any of them.
    pub fn sample(&self, t: f32, x0: f32, v0: f32) -> (f32, f32) {
        if t <= 0.0 {
            return (x0, v0);
        }
        let omega = self.omega();
        let zeta = self.damping_fraction;
        if (zeta - 1.0).abs() < 1e-3 {
            // Critically damped: y = e^-wt * (x0 + c*t), c = v0 + w*x0.
            let c = v0 + omega * x0;
            let e = (-omega * t).exp();
            let x = e * (x0 + c * t);
            let v = e * (c - omega * (x0 + c * t));
            (x, v)
        } else if zeta < 1.0 {
            // Underdamped: y = e^-zwt * (A cos(wd t) + B sin(wd t)).
            let omega_d = omega * (1.0 - zeta * zeta).sqrt();
            let a = x0;
            let b = (v0 + zeta * omega * x0) / omega_d;
            let e = (-zeta * omega * t).exp();
            let (s, c) = (omega_d * t).sin_cos();
            let osc = a * c + b * s;
            let d_osc = -a * omega_d * s + b * omega_d * c;
            let x = e * osc;
            let v = e * (-zeta * omega * osc + d_osc);
            (x, v)
        } else {
            // Overdamped: y = C1 e^(r1 t) + C2 e^(r2 t), both roots real
            // and negative.
            let disc = (zeta * zeta - 1.0).sqrt();
            let r1 = omega * (-zeta + disc);
            let r2 = omega * (-zeta - disc);
            let c1 = (v0 - r2 * x0) / (r1 - r2);
            let c2 = x0 - c1;
            let e1 = (r1 * t).exp();
            let e2 = (r2 * t).exp();
            let x = c1 * e1 + c2 * e2;
            let v = c1 * r1 * e1 + c2 * r2 * e2;
            (x, v)
        }
    }
}

/// How close to the target (value and velocity) a `SpringTween` must get
/// before it reports settled and stops requesting repaints.
const SPRING_EPS_X: f32 = 0.0015;
const SPRING_EPS_V: f32 = 0.002;
/// Safety cutoff: forces settled after this long regardless of the
/// analytic epsilon check, so a pathological spring (near-zero damping,
/// tiny `response`) can never pin a widget in a perpetual repaint loop.
const SPRING_MAX_SECONDS: f32 = 4.0;

/// Time-driven, retargetable spring motion for one scalar property --
/// `Tween`'s spring-physics counterpart. Stores the oscillator's initial
/// conditions at the last retarget (`x0`/`v0`, displacement and velocity
/// from `target` at that moment) rather than integrating frame to frame, so
/// `value(now)`/`velocity(now)` are exact closed-form evaluations at any
/// `now` -- pure, time injected, no `Instant`.
#[derive(Clone, Copy, Debug)]
pub struct SpringTween {
    x0: f32,
    v0: f32,
    target: f32,
    start: f64,
    spring: Spring,
}

impl SpringTween {
    /// Starts settled at `value`.
    pub fn new(value: f32) -> Self {
        Self {
            x0: 0.0,
            v0: 0.0,
            target: value,
            start: 0.0,
            spring: Spring::SMOOTH,
        }
    }

    fn sample_absolute(&self, now: f64) -> (f32, f32) {
        let t = (now - self.start) as f32;
        let (y, v) = self.spring.sample(t, self.x0, self.v0);
        (self.target + y, v)
    }

    /// Retargets toward `to`, sampling the *current* value and velocity as
    /// the new initial conditions -- this is the whole point of a spring
    /// model: an interruption (five rapid hold/release cycles, say) carries
    /// momentum through rather than snapping to a fresh standstill. A
    /// no-op if `to` already equals the current target, same as `Tween`
    /// (continuous per-frame callers -- window resize, Text Size -- must
    /// not restart the animation every frame).
    pub fn retarget(&mut self, now: f64, to: f32, spring: Spring) {
        if (self.target - to).abs() <= f32::EPSILON {
            return;
        }
        let (x, v) = self.sample_absolute(now);
        self.x0 = x - to;
        self.v0 = v;
        self.target = to;
        self.start = now;
        self.spring = spring;
    }

    pub fn value(&self, now: f64) -> f32 {
        self.sample_absolute(now).0
    }

    pub fn velocity(&self, now: f64) -> f32 {
        self.sample_absolute(now).1
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    /// True until both displacement and velocity fall under their epsilons
    /// (or the safety cutoff elapses), same contract as `Tween::
    /// is_animating` -- false from the settling frame onward.
    pub fn is_animating(&self, now: f64) -> bool {
        let elapsed = (now - self.start) as f32;
        if elapsed >= SPRING_MAX_SECONDS {
            return false;
        }
        let (x, v) = self.sample_absolute(now);
        (x - self.target).abs() > SPRING_EPS_X || v.abs() > SPRING_EPS_V
    }

    /// Jumps straight to `to`, no animation, zero velocity -- Reduce Motion
    /// and a fresh instance's initial value.
    pub fn snap(&mut self, to: f32) {
        self.x0 = 0.0;
        self.v0 = 0.0;
        self.target = to;
        self.start = 0.0;
    }
}

/// `Presence`'s spring-physics counterpart -- same `Phase` state machine
/// (a Closing element keeps rendering, non-interactive, until it settles;
/// reopening mid-close reverses from the current reveal *and velocity*
/// rather than jumping), driven by a `SpringTween` instead of a `Tween`.
#[derive(Clone, Copy, Debug)]
pub struct SpringPresence {
    tween: SpringTween,
    open: bool,
}

impl SpringPresence {
    pub fn closed() -> Self {
        Self {
            tween: SpringTween::new(0.0),
            open: false,
        }
    }

    pub fn set_open(&mut self, now: f64, open: bool, expand: Spring, collapse: Spring) {
        if open == self.open {
            return;
        }
        self.open = open;
        self.tween.retarget(
            now,
            if open { 1.0 } else { 0.0 },
            if open { expand } else { collapse },
        );
    }

    pub fn snap_open(&mut self, open: bool) {
        self.open = open;
        self.tween.snap(if open { 1.0 } else { 0.0 });
    }

    /// `0.0` closed .. `1.0` open; may exceed `1.0` while Opening if the
    /// expand spring is underdamped (bounces).
    pub fn reveal(&self, now: f64) -> f32 {
        self.tween.value(now)
    }

    pub fn velocity(&self, now: f64) -> f32 {
        self.tween.velocity(now)
    }

    pub fn phase(&self, now: f64) -> Phase {
        if self.open {
            if self.tween.is_animating(now) {
                Phase::Opening
            } else {
                Phase::Open
            }
        } else if self.tween.is_animating(now) {
            Phase::Closing
        } else {
            Phase::Closed
        }
    }

    pub fn should_render(&self, now: f64) -> bool {
        self.phase(now) != Phase::Closed
    }

    pub fn interactive(&self, now: f64) -> bool {
        matches!(self.phase(now), Phase::Opening | Phase::Open)
    }

    pub fn is_animating(&self, now: f64) -> bool {
        self.tween.is_animating(now)
    }
}

fn spring_presence_key(id: egui::Id) -> egui::Id {
    id.with("spatial_ui_kit::motion::spring_presence")
}

fn bounce_amount_key() -> egui::Id {
    egui::Id::new("spatial_ui_kit::motion::bounce_amount")
}

/// Global bounce-amount multiplier for every `spring_presence`/
/// `spring_presence_with` call against this `ctx` from now on -- SDB's
/// Settings > Appearance > "Animation Bounce" slider (0%..150%) calls this
/// each frame/on change. `0.0` removes all overshoot; `1.0` (default) is
/// each spring's own preset; above `1.0` exaggerates it. See `Spring::
/// scaled_by_bounce` for the exact mapping and why a non-bouncy spring
/// (`SMOOTH`) is unaffected at any value.
pub fn set_bounce_amount(ctx: &egui::Context, amount: f32) {
    ctx.data_mut(|d| d.insert_temp(bounce_amount_key(), amount));
}

pub fn bounce_amount(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp(bounce_amount_key())).unwrap_or(1.0)
}

/// Drives a [`SpringPresence`] stored in `ctx` memory for `id` from `open`,
/// using `Spring::BOUNCY` to open and `Spring::SMOOTH` to close (both
/// scaled by the global bounce amount, see `set_bounce_amount`) -- the
/// "expand springs, collapse doesn't" rule, spring-physics version of
/// [`presence`]. See `spring_presence_with` for custom springs.
pub fn spring_presence(ctx: &egui::Context, id: egui::Id, open: bool) -> PresenceFrame {
    spring_presence_with(ctx, id, open, Spring::BOUNCY, Spring::SMOOTH)
}

/// As [`spring_presence`], with caller-chosen expand/collapse springs.
pub fn spring_presence_with(
    ctx: &egui::Context,
    id: egui::Id,
    open: bool,
    expand: Spring,
    collapse: Spring,
) -> PresenceFrame {
    let now = ctx.input(|i| i.time);
    let bounce = bounce_amount(ctx);
    let expand = expand.scaled_by_bounce(bounce);
    let collapse = collapse.scaled_by_bounce(bounce);
    let key = spring_presence_key(id);
    let mut presence: SpringPresence = ctx
        .data(|d| d.get_temp(key))
        .unwrap_or_else(SpringPresence::closed);

    if reduce_motion(ctx) {
        presence.snap_open(open);
    } else {
        presence.set_open(now, open, expand, collapse);
    }

    let frame = PresenceFrame {
        reveal: presence.reveal(now),
        render: presence.should_render(now),
        interactive: presence.interactive(now),
        animating: presence.is_animating(now),
    };
    if frame.animating {
        ctx.request_repaint();
    }

    ctx.data_mut(|d| d.insert_temp(key, presence));
    frame
}

/// Drives a [`Presence`] stored in `ctx` memory for `id` from `open`, using
/// `MotionSpec::EXPAND`/`MotionSpec::COLLAPSE`. See `presence_with` for
/// custom specs.
///
/// An id `presence`/`presence_with` has never seen before starts from
/// [`Presence::closed`] -- so an `open == true` first call animates open
/// from `0.0` rather than starting revealed. That's the fix for the exact
/// bug `ThemedWindow::modal` had: egui's own `animate_bool` initializes an
/// unseen id at its *end* value, so a first-frame-open modal never got to
/// play its slide-in.
pub fn presence(ctx: &egui::Context, id: egui::Id, open: bool) -> PresenceFrame {
    presence_with(ctx, id, open, MotionSpec::EXPAND, MotionSpec::COLLAPSE)
}

/// As [`presence`], with caller-chosen expand/collapse specs.
pub fn presence_with(
    ctx: &egui::Context,
    id: egui::Id,
    open: bool,
    expand: MotionSpec,
    collapse: MotionSpec,
) -> PresenceFrame {
    let now = ctx.input(|i| i.time);
    let key = presence_key(id);
    let mut presence: Presence = ctx
        .data(|d| d.get_temp(key))
        .unwrap_or_else(Presence::closed);

    if reduce_motion(ctx) {
        presence.snap_open(open);
    } else {
        presence.set_open(now, open, expand, collapse);
    }

    let frame = PresenceFrame {
        reveal: presence.reveal(now),
        render: presence.should_render(now),
        interactive: presence.interactive(now),
        animating: presence.is_animating(now),
    };
    if frame.animating {
        ctx.request_repaint();
    }

    ctx.data_mut(|d| d.insert_temp(key, presence));
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Spring ----------------------------------------------------------

    #[test]
    fn spring_sample_endpoints() {
        // Settled: no displacement, no velocity -> stays put at any t.
        let (x, v) = Spring::SMOOTH.sample(1.0, 0.0, 0.0);
        assert_eq!(x, 0.0);
        assert_eq!(v, 0.0);
        // t == 0 always returns the initial conditions unchanged.
        let (x, v) = Spring::BOUNCY.sample(0.0, 1.0, 2.0);
        assert_eq!(x, 1.0);
        assert_eq!(v, 2.0);
    }

    #[test]
    fn spring_reaches_target_eventually() {
        for spring in [Spring::SMOOTH, Spring::SNAPPY, Spring::BOUNCY] {
            let (x, v) = spring.sample(5.0, 1.0, 0.0);
            assert!(x.abs() < 0.01, "{spring:?} left x={x} after 5s");
            assert!(v.abs() < 0.01, "{spring:?} left v={v} after 5s");
        }
    }

    #[test]
    fn critically_damped_never_overshoots() {
        let spring = Spring {
            response: 0.4,
            damping_fraction: 1.0,
        };
        let mut t = 0.0f32;
        while t <= 3.0 {
            let (x, _) = spring.sample(t, 1.0, 0.0);
            // Displacement starts at 1.0 (target 0) and must only shrink in
            // magnitude, never cross past 0 and back (that would be
            // overshoot for a unit step).
            assert!(x >= -1e-4, "critically damped overshot: x={x} at t={t}");
            t += 0.01;
        }
    }

    #[test]
    fn underdamped_overshoot_matches_analytic_peak() {
        // Underdamped step response peak (unit step, i.e. x0 = -1 relative
        // to a target reached from below) overshoots by
        // exp(-zeta*pi/sqrt(1-zeta^2)) fraction of the step -- the standard
        // control-theory result. Verify our numeric peak matches it for a
        // couple of damping ratios.
        for zeta in [0.3f32, 0.7] {
            let spring = Spring {
                response: 0.5,
                damping_fraction: zeta,
            };
            let expected_overshoot_frac =
                (-zeta * std::f32::consts::PI / (1.0 - zeta * zeta).sqrt()).exp();
            // Step from x0 = -1 (below target 0) up toward 0; track the max
            // overshoot above 0.
            let mut peak = 0.0f32;
            let mut t = 0.0f32;
            while t <= 3.0 {
                let (x, _) = spring.sample(t, -1.0, 0.0);
                peak = peak.max(x);
                t += 0.001;
            }
            assert!(
                (peak - expected_overshoot_frac).abs() < 0.02,
                "zeta={zeta}: peak={peak}, expected={expected_overshoot_frac}"
            );
        }
    }

    #[test]
    fn spring_tween_retarget_preserves_value_and_velocity_continuity() {
        let mut tw = SpringTween::new(0.0);
        tw.retarget(0.0, 1.0, Spring::BOUNCY);
        let now = 0.15;
        let value_before = tw.value(now);
        let velocity_before = tw.velocity(now);
        tw.retarget(now, 0.0, Spring::SMOOTH);
        let value_after = tw.value(now);
        let velocity_after = tw.velocity(now);
        assert!(
            (value_before - value_after).abs() < 1e-4,
            "value jumped: {value_before} -> {value_after}"
        );
        assert!(
            (velocity_before - velocity_after).abs() < 1e-3,
            "velocity jumped: {velocity_before} -> {velocity_after}"
        );
    }

    #[test]
    fn spring_tween_settles_in_finite_time() {
        let mut tw = SpringTween::new(0.0);
        tw.retarget(0.0, 1.0, Spring::BOUNCY);
        assert!(tw.is_animating(0.05));
        assert!(!tw.is_animating(SPRING_MAX_SECONDS as f64 + 0.01));
        // Well before the safety cutoff too, for a spring this fast.
        assert!(!tw.is_animating(3.0));
    }

    #[test]
    fn spring_tween_same_target_retarget_does_not_restart() {
        let mut tw = SpringTween::new(0.0);
        tw.retarget(0.0, 1.0, Spring::BOUNCY);
        let mid = tw.value(0.1);
        tw.retarget(0.1, 1.0, Spring::BOUNCY); // same target -- no-op
        assert_eq!(tw.value(0.1), mid);
    }

    #[test]
    fn spring_presence_reduce_motion_snaps() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("spring_motion_test_reduce");
        set_reduce_motion(&ctx, true);
        let frame = spring_presence(&ctx, id, true);
        assert_eq!(frame.reveal, 1.0);
        assert!(!frame.animating);
        set_reduce_motion(&ctx, false);
    }

    #[test]
    fn bounce_amount_zero_means_no_overshoot() {
        let ctx = egui::Context::default();
        set_bounce_amount(&ctx, 0.0);
        let spring = Spring::BOUNCY.scaled_by_bounce(bounce_amount(&ctx));
        assert_eq!(
            spring.damping_fraction, 1.0,
            "amount=0 must fully remove bounce"
        );

        let mut peak = 0.0f32;
        let mut t = 0.0f32;
        while t <= 2.0 {
            let (x, _) = spring.sample(t, -1.0, 0.0);
            peak = peak.max(x);
            t += 0.001;
        }
        assert!(peak <= 1e-4, "no overshoot expected, peak was {peak}");
        set_bounce_amount(&ctx, 1.0);
    }

    #[test]
    fn bounce_amount_scales_smooth_spring_not_at_all() {
        // SMOOTH's damping_fraction is already 1.0 (no inherent bounce) --
        // scaling it must be a no-op at any amount, which is what keeps
        // COLLAPSE-style motion non-bouncy regardless of the slider.
        for amount in [0.0f32, 1.0, 1.5] {
            let scaled = Spring::SMOOTH.scaled_by_bounce(amount);
            assert_eq!(scaled.damping_fraction, 1.0);
        }
    }

    #[test]
    fn bounce_amount_above_one_increases_bounce() {
        let default_spring = Spring::BOUNCY.scaled_by_bounce(1.0);
        let more_bounce = Spring::BOUNCY.scaled_by_bounce(1.5);
        assert!(more_bounce.damping_fraction < default_spring.damping_fraction);
    }

    #[test]
    fn spring_tween_snap_zeroes_velocity() {
        let mut tw = SpringTween::new(0.0);
        tw.retarget(0.0, 1.0, Spring::BOUNCY);
        tw.snap(2.0);
        assert_eq!(tw.value(0.0), 2.0);
        assert_eq!(tw.velocity(0.0), 0.0);
        assert!(!tw.is_animating(0.0));
    }

    // -- easing --------------------------------------------------------

    #[test]
    fn ease_out_back_endpoints() {
        assert_eq!(ease_out_back(0.0, 1.70158), 0.0);
        assert!((ease_out_back(1.0, 1.70158) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ease_out_back_clamps_t() {
        assert_eq!(ease_out_back(-1.0, 1.70158), ease_out_back(0.0, 1.70158));
        assert_eq!(ease_out_back(2.0, 1.70158), ease_out_back(1.0, 1.70158));
    }

    #[test]
    fn ease_out_back_peak_is_about_1_10() {
        let mut peak = 0.0f32;
        let mut t = 0.0;
        while t <= 1.0 {
            peak = peak.max(ease_out_back(t, 1.70158));
            t += 0.001;
        }
        assert!((peak - 1.10).abs() < 0.01, "peak was {peak}");
    }

    #[test]
    fn ease_out_cubic_endpoints_and_never_exceeds_one() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 1e-6);
        let mut t = 0.0;
        while t <= 1.0 {
            assert!(ease_out_cubic(t) <= 1.0 + 1e-6, "t={t}");
            t += 0.01;
        }
    }

    #[test]
    fn cap_overshoot_only_clamps_values_above_target() {
        assert_eq!(cap_overshoot(50.0, 100.0, 6.0), 50.0); // still approaching, untouched
        assert_eq!(cap_overshoot(103.0, 100.0, 6.0), 103.0); // within cap
        assert_eq!(cap_overshoot(140.0, 100.0, 6.0), 106.0); // over cap, clamped
        assert_eq!(cap_overshoot(100.0, 100.0, 6.0), 100.0);
    }

    // -- Tween -----------------------------------------------------------

    #[test]
    fn tween_new_is_settled() {
        let tw = Tween::new(5.0);
        assert_eq!(tw.value(0.0), 5.0);
        assert_eq!(tw.target(), 5.0);
        assert!(!tw.is_animating(0.0));
    }

    #[test]
    fn tween_zero_duration_snaps() {
        let mut tw = Tween::new(0.0);
        tw.retarget(
            0.0,
            10.0,
            MotionSpec {
                duration: 0.0,
                curve: Curve::CubicOut,
            },
        );
        assert_eq!(tw.value(0.0), 10.0);
        assert!(!tw.is_animating(0.0));
    }

    #[test]
    fn tween_settles_to_exact_target() {
        let mut tw = Tween::new(0.0);
        tw.retarget(0.0, 1.0, MotionSpec::EXPAND);
        assert_eq!(tw.value(10.0), 1.0);
        assert!(!tw.is_animating(10.0));
    }

    #[test]
    fn tween_same_target_retarget_does_not_restart() {
        let mut tw = Tween::new(0.0);
        tw.retarget(0.0, 1.0, MotionSpec::EXPAND);
        let mid = tw.value(0.05);
        tw.retarget(0.05, 1.0, MotionSpec::EXPAND); // same target -- no-op
        assert_eq!(tw.value(0.05), mid);
    }

    #[test]
    fn tween_retarget_continuity_including_mid_overshoot() {
        let mut tw = Tween::new(0.0);
        tw.retarget(0.0, 1.0, MotionSpec::EXPAND);
        // Sample mid-flight, likely mid-overshoot for a back-out curve.
        let now = MotionSpec::EXPAND.duration as f64 * 0.9;
        let just_before = tw.value(now);
        tw.retarget(now, 0.0, MotionSpec::COLLAPSE);
        let just_after = tw.value(now);
        assert!((just_before - just_after).abs() < 1e-5);
    }

    // -- Presence ----------------------------------------------------------

    #[test]
    fn presence_open_animates_from_zero() {
        let mut p = Presence::closed();
        p.set_open(0.0, true, MotionSpec::EXPAND, MotionSpec::COLLAPSE);
        assert_eq!(p.reveal(0.0), 0.0);
        assert!(p.reveal(0.0) < 1.0);
        assert_eq!(p.phase(0.0), Phase::Opening);
    }

    #[test]
    fn presence_closed_id_that_stays_closed_does_not_render() {
        let mut p = Presence::closed();
        p.set_open(0.0, false, MotionSpec::EXPAND, MotionSpec::COLLAPSE);
        assert!(!p.should_render(0.0));
        assert_eq!(p.phase(0.0), Phase::Closed);
    }

    #[test]
    fn presence_not_interactive_while_closing() {
        let mut p = Presence::closed();
        p.set_open(0.0, true, MotionSpec::EXPAND, MotionSpec::COLLAPSE);
        let opened_at = MotionSpec::EXPAND.duration as f64;
        assert!(!p.is_animating(opened_at));
        p.set_open(opened_at, false, MotionSpec::EXPAND, MotionSpec::COLLAPSE);
        assert_eq!(p.phase(opened_at), Phase::Closing);
        assert!(!p.interactive(opened_at));
    }

    #[test]
    fn presence_should_render_until_close_finishes_then_stops() {
        let mut p = Presence::closed();
        p.set_open(0.0, true, MotionSpec::EXPAND, MotionSpec::COLLAPSE);
        let opened_at = MotionSpec::EXPAND.duration as f64;
        p.set_open(opened_at, false, MotionSpec::EXPAND, MotionSpec::COLLAPSE);
        let mid_close = opened_at + (MotionSpec::COLLAPSE.duration as f64 * 0.5);
        assert!(p.should_render(mid_close));
        let closed_at = opened_at + MotionSpec::COLLAPSE.duration as f64 + 1.0;
        assert!(!p.should_render(closed_at));
    }

    #[test]
    fn presence_reopen_during_close_reverses_without_a_jump() {
        let mut p = Presence::closed();
        p.set_open(0.0, true, MotionSpec::EXPAND, MotionSpec::COLLAPSE);
        let opened_at = MotionSpec::EXPAND.duration as f64;
        p.set_open(opened_at, false, MotionSpec::EXPAND, MotionSpec::COLLAPSE);
        let mid_close = opened_at + (MotionSpec::COLLAPSE.duration as f64 * 0.4);
        let reveal_just_before = p.reveal(mid_close);
        p.set_open(mid_close, true, MotionSpec::EXPAND, MotionSpec::COLLAPSE);
        let reveal_just_after = p.reveal(mid_close);
        assert!((reveal_just_before - reveal_just_after).abs() < 1e-5);
        assert_eq!(p.phase(mid_close), Phase::Opening);
    }

    #[test]
    fn presence_snap_open_is_instant() {
        let mut p = Presence::closed();
        p.snap_open(true);
        assert_eq!(p.reveal(0.0), 1.0);
        assert_eq!(p.phase(0.0), Phase::Open);
        p.snap_open(false);
        assert_eq!(p.reveal(0.0), 0.0);
        assert_eq!(p.phase(0.0), Phase::Closed);
    }

    // -- egui glue -----------------------------------------------------

    /// Drives one egui pass at time `t`, running `f` inside it (egui 0.35
    /// has no `Context::run` returning a caller value -- only `run_ui`,
    /// which is `FnMut(&mut Ui)` -- so this uses `begin_pass`/`end_pass`
    /// directly to get both `f`'s result and the pass's `FullOutput`).
    fn step<R>(
        ctx: &egui::Context,
        t: f64,
        f: impl FnOnce(&egui::Context) -> R,
    ) -> (R, egui::FullOutput) {
        ctx.begin_pass(egui::RawInput {
            time: Some(t),
            ..Default::default()
        });
        let result = f(ctx);
        (result, ctx.end_pass())
    }

    #[test]
    fn first_seen_open_id_animates_not_starts_at_one() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("motion_test_modal");
        let (frame, _) = step(&ctx, 0.0, |ctx| presence(ctx, id, true));
        assert!(frame.reveal < 1.0);
        assert!(frame.animating);
    }

    #[test]
    fn repaint_requested_while_animating_and_not_once_settled() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("motion_test_repaint");

        // A context's very first pass always reports a repaint (egui's own
        // font/texture warm-up, unrelated to us) -- establish that baseline
        // before asserting on our own repaint requests.
        step(&ctx, 0.0, |ctx| {
            presence(ctx, id, true);
        });

        let mid_flight = MotionSpec::EXPAND.duration as f64 * 0.5;
        let (_, out) = step(&ctx, mid_flight, |ctx| {
            presence(ctx, id, true);
        });
        assert!(out
            .viewport_output
            .values()
            .next()
            .is_some_and(|v| v.repaint_delay < std::time::Duration::from_secs(1)));

        // Advance well past MotionSpec::EXPAND's duration; settled now, no
        // further repaint should be requested for this presence.
        // Advance well past MotionSpec::EXPAND's duration; settled now, no
        // further repaint should be requested for this presence. egui
        // itself schedules one extra repaint after any `request_repaint`
        // ("two repaints per request, to let frame-delayed responses
        // settle" -- see `Context::request_repaint_after`'s doc comment),
        // so the pass immediately after the last animating one still
        // carries that leftover repaint; only the pass after *that* one
        // reflects whether we ourselves are still asking for repaints.
        let settled_t = MotionSpec::EXPAND.duration as f64 + 1.0;
        step(&ctx, settled_t, |ctx| {
            presence(ctx, id, true);
        });
        let (_, out) = step(&ctx, settled_t, |ctx| {
            presence(ctx, id, true);
        });
        assert!(out
            .viewport_output
            .values()
            .next()
            .is_some_and(|v| v.repaint_delay >= std::time::Duration::from_secs(1)));
    }

    #[test]
    fn reduce_motion_snaps_presence() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("motion_test_reduce");
        set_reduce_motion(&ctx, true);
        let (frame, _) = step(&ctx, 0.0, |ctx| presence(ctx, id, true));
        assert_eq!(frame.reveal, 1.0);
        assert!(!frame.animating);
    }
}

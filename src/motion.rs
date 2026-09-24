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

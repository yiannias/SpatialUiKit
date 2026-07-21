//! Docking/tabbing/floating-panel mechanics, generic over an app-supplied
//! tab identity type `T`, shared between SpatialSketchPad and
//! SpatialDrawingBoard.
//!
//! Ported from SpatialSketchPad's `ssp-app::window` + the drag-to-detach/
//! redock mechanism documented in its
//! `docs/design/2026-07-20-panel-drag-dock-redock-design-brief.md`. Two
//! prior attempts to build this gesture failed by mutating dock state
//! mid-drag, racing egui_dock's own internal drag lifecycle -- confirmed by
//! reading `egui_dock 0.20.1`'s `widgets/dock_area/show/leaf.rs`: once a
//! tab's own drag engages, egui_dock swaps in a *second*,
//! `id.with("dragged")` interact response, but egui keeps attributing the
//! whole drag to the *original* tab-title response throughout -- so a
//! `TabViewer::on_tab_button` response never reports `dragged()`/
//! `drag_started()`. [`detect_tab_drag`] therefore tracks drags passively
//! via raw pointer state, and all dock-state mutation happens on the release
//! frame via [`classify_drag_release`]/[`move_panel`] -- which callers must
//! run **after both dock areas have shown**, once egui_dock has fully
//! resolved its own same-frame drag handling. Do not "simplify" this
//! ordering; it's the entire fix, not an implementation detail.
//!
//! What stays app-specific: everything about *what a tab is* beyond its bare
//! identity -- title text and content rendering go through [`DockHost`];
//! detached-OS-window lifecycle (`eframe::show_viewport_deferred`, boot-vs-
//! live geometry, action dispatch) is main-loop code that stays in each app,
//! since a shared crate can't own per-app viewport state. `egui_dock` is
//! this crate's first non-`egui` dependency, pinned to an exact patch
//! (`=0.20.1`) since the fix above depends on that patch's specific internal
//! behavior -- an incidental minor bump could silently change or fix it out
//! from under `detect_tab_drag`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Bound set for a tab's identity type: cheap to copy, usable as a map key,
/// and hashable into a stable [`egui::ViewportId`] for its detached window.
pub trait TabId: Copy + Eq + std::hash::Hash + std::fmt::Debug {}
impl<T: Copy + Eq + std::hash::Hash + std::fmt::Debug> TabId for T {}

/// Which of the two docks a tab lives in. Fixed at two sides -- nothing in
/// either app's design asks for more, and the drag-release classification
/// below is written specifically in terms of "own side / opposite side".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockSide {
    Left,
    Right,
}

impl DockSide {
    pub fn opposite(self) -> DockSide {
        match self {
            DockSide::Left => DockSide::Right,
            DockSide::Right => DockSide::Left,
        }
    }
}

/// Where a panel should be placed. Mirrors each app's own persisted
/// placement enum (e.g. SSP's `PanelPlacement`), but stays deliberately
/// smaller and non-serializable -- it's the vocabulary [`move_panel`]
/// operates on, not a storage format. Apps keep their own enum for
/// persistence and convert to/from this at the call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockTarget {
    Side(DockSide),
    Floating,
    Hidden,
}

/// Live state of an in-progress docked-tab drag, tracked passively by
/// [`detect_tab_drag`] and consumed only on pointer release, by
/// [`classify_drag_release`].
#[derive(Clone, Copy, Debug)]
pub struct TabDrag<T: TabId> {
    pub tab: T,
    /// Which dock the drag started from, so release can tell "dropped back
    /// on my own dock" (egui_dock already handled it) from "opposite dock"
    /// from "open space".
    pub source_side: Option<DockSide>,
    /// Accumulated drag path length in points, to ignore twitchy clicks.
    pub dist: f32,
}

/// Default minimum accumulated drag path (points) before a tab drag counts
/// as a deliberate drag-out rather than a twitchy click on the tab.
pub const DETACH_MIN_DRAG_PX: f32 = 30.0;

/// Passive drag tracking for one tab's `TabViewer::on_tab_button` call --
/// see this module's doc comment for why this can't use
/// `response.dragged()`. Returns `Some` only once a real drag is confirmed
/// (press origin inside the tab's rect, egui "decidedly dragging"); the
/// caller should store it into whatever `Option<TabDrag<T>>` it threads
/// through the frame.
pub fn detect_tab_drag<T: TabId>(tab: T, response: &egui::Response, source_side: Option<DockSide>) -> Option<TabDrag<T>> {
    let (down, origin, latest, decided) = response.ctx.input(|i| {
        (
            i.pointer.primary_down(),
            i.pointer.press_origin(),
            i.pointer.latest_pos(),
            i.pointer.is_decidedly_dragging(),
        )
    });
    if down && decided {
        if let (Some(origin), Some(latest)) = (origin, latest) {
            if response.rect.contains(origin) {
                return Some(TabDrag {
                    tab,
                    source_side,
                    dist: (latest - origin).length(),
                });
            }
        }
    }
    None
}

/// Where a released tab drag implies moving to.
#[derive(Clone, Copy, Debug)]
pub enum DragTarget {
    Side(DockSide),
    /// Open-space drop -- caller should call [`move_panel`] with
    /// `DockTarget::Floating` and detach to a real OS window spawned at this
    /// screen position (position handling itself stays app-side, since it's
    /// tied to each app's own `show_viewport_deferred`/boot-geometry setup).
    FloatingAt(egui::Pos2),
}

/// What a released drag resolved to.
#[derive(Clone, Copy, Debug)]
pub enum ReleaseAction<T: TabId> {
    /// A click (never exceeded the distance threshold), or dropped back on
    /// its own source dock -- egui_dock already resolved the reorder/split.
    NoOp,
    MoveTo { tab: T, target: DragTarget },
}

/// Result of [`classify_drag_release`] for the current frame.
#[derive(Clone, Copy, Debug)]
pub enum DragReleaseOutcome<T: TabId> {
    /// Not yet released; still mid-drag. `ghost_at` is the pointer position
    /// over open space (outside both docks), if any -- pass it to
    /// [`draw_detach_ghost`] for "release here to float" feedback. `None`
    /// while hovering a dock, where egui_dock's own reorder/split feedback
    /// already covers it.
    StillDragging { ghost_at: Option<egui::Pos2> },
    /// The pointer vanished mid-drag without a release event reaching us
    /// (window focus loss, etc.) -- caller should drop the tracked
    /// `TabDrag` without acting on it.
    Lost,
    /// Released this frame. Caller should always clear the tracked
    /// `TabDrag` here, then act on `action`.
    Released { action: ReleaseAction<T> },
}

/// Classify a drag-in-progress or just-released tab drag against the two
/// dock panels' on-screen rects this frame. Must be called **once per frame,
/// after both dock areas have shown** -- see the module doc comment for why;
/// this function itself doesn't and can't enforce that ordering, since it
/// has no visibility into when the caller invoked it.
pub fn classify_drag_release<T: TabId>(
    drag: &TabDrag<T>,
    released: bool,
    pointer_down: bool,
    pointer_pos: Option<egui::Pos2>,
    left_dock_rect: Option<egui::Rect>,
    right_dock_rect: Option<egui::Rect>,
    min_drag_px: f32,
) -> DragReleaseOutcome<T> {
    let over_left = |pos: egui::Pos2| left_dock_rect.is_some_and(|r| r.contains(pos));
    let over_right = |pos: egui::Pos2| right_dock_rect.is_some_and(|r| r.contains(pos));

    if !released {
        if !pointer_down {
            return DragReleaseOutcome::Lost;
        }
        if drag.dist > min_drag_px {
            if let Some(pos) = pointer_pos {
                if !over_left(pos) && !over_right(pos) {
                    return DragReleaseOutcome::StillDragging { ghost_at: Some(pos) };
                }
            }
        }
        return DragReleaseOutcome::StillDragging { ghost_at: None };
    }

    if drag.dist <= min_drag_px {
        return DragReleaseOutcome::Released { action: ReleaseAction::NoOp };
    }
    let Some(pos) = pointer_pos else {
        return DragReleaseOutcome::Released { action: ReleaseAction::NoOp };
    };
    let (on_left, on_right) = (over_left(pos), over_right(pos));
    let action = match drag.source_side {
        Some(DockSide::Left) if on_left => ReleaseAction::NoOp,
        Some(DockSide::Right) if on_right => ReleaseAction::NoOp,
        _ if on_left => ReleaseAction::MoveTo { tab: drag.tab, target: DragTarget::Side(DockSide::Left) },
        _ if on_right => ReleaseAction::MoveTo { tab: drag.tab, target: DragTarget::Side(DockSide::Right) },
        _ => ReleaseAction::MoveTo { tab: drag.tab, target: DragTarget::FloatingAt(pos) },
    };
    DragReleaseOutcome::Released { action }
}

/// Cursor-following outline shown while a tab drag is over open space --
/// egui_dock's own ghost-tab preview is clipped to the dock panel and gives
/// no feedback once the pointer leaves it.
pub fn draw_detach_ghost(ctx: &egui::Context, pos: egui::Pos2, title: &str) {
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("tab_detach_ghost")));
    let rect = egui::Rect::from_min_size(pos + egui::vec2(14.0, 10.0), egui::vec2(180.0, 110.0));
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgba_unmultiplied(120, 160, 220, 20));
    painter.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(150, 180, 230, 200)),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.min + egui::vec2(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        title,
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgba_unmultiplied(200, 215, 240, 230),
    );
}

/// Removes `panel` from both docks (if present) and forgets its proximity-
/// redock history, then re-places it per `target`. The `redock_watch`
/// removal is unconditional and happens for every call, not just a move to
/// `Floating` -- a freshly (re)placed panel must never inherit an
/// `ever_moved` flag from a previous detached life, or a window re-detached
/// near the main window's edge could auto-redock itself moments after
/// spawning. `Floating`/`Hidden` targets don't touch either `DockState`;
/// the caller owns whatever set/map tracks which panels are currently
/// floating (this crate doesn't own that collection -- see the module doc).
pub fn move_panel<T: TabId>(
    panel: T,
    target: DockTarget,
    left: &mut egui_dock::DockState<T>,
    right: &mut egui_dock::DockState<T>,
    redock_watch: &mut HashMap<T, RedockWatch>,
) {
    if let Some(loc) = left.find_tab(&panel) {
        left.remove_tab(loc);
    }
    if let Some(loc) = right.find_tab(&panel) {
        right.remove_tab(loc);
    }
    redock_watch.remove(&panel);

    if let DockTarget::Side(side) = target {
        match side {
            DockSide::Left => left.push_to_first_leaf(panel),
            DockSide::Right => right.push_to_first_leaf(panel),
        }
    }
}

// ---------------------------------------------------------------------------
// Redock-by-proximity: drag a detached OS window near the main window's
// edge and hold it there briefly to redock it. Pure proximity/stability
// math, callable every frame a detached window is visible.
// ---------------------------------------------------------------------------

/// Default screen-pixel distance from the main window's edge that arms
/// auto-redock.
pub const REDOCK_SNAP_PX: f32 = 40.0;
/// Default duration (ms) a detached window must stay put once armed before
/// it actually redocks.
pub const REDOCK_STABLE_MS: u64 = 400;

/// Per-tab bookkeeping for [`poll_redock`].
#[derive(Clone, Copy, Debug)]
pub struct RedockWatch {
    rect: egui::Rect,
    last_change: Instant,
    /// Precondition for auto-redock: an actual drag must have moved this
    /// window at some point, so a window merely resting near the edge (at
    /// boot, or freshly spawned there by drag-detach) never self-redocks.
    ever_moved: bool,
}

/// Outcome of one [`poll_redock`] check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedockPollOutcome {
    /// Not near an edge (or near but never actually dragged there) -- no
    /// action.
    Idle,
    /// Near an edge but hasn't been stable long enough yet -- caller should
    /// keep repainting (egui is reactive; once the OS drag stops emitting
    /// Moved events, no more frames run on their own, so the stability
    /// timer needs a nudge to get re-checked).
    Watching,
    /// Near an edge and stable for the required duration -- caller should
    /// redock to `side` and remove this tab's watch entry.
    Redock { side: DockSide },
}

/// Proximity + stability check for one detached window's on-screen rect
/// against the main window's rect, called every frame a detached window is
/// visible. There's no OS-level "window drag stopped" signal to hook (unlike
/// an egui-internal `Response::drag_stopped()`), so "stopped" is
/// approximated as "this rect hasn't changed in `stable`".
pub fn poll_redock<T: TabId>(
    tab: T,
    current_rect: egui::Rect,
    main_window_rect: egui::Rect,
    watch: &mut HashMap<T, RedockWatch>,
    snap_px: f32,
    stable: Duration,
) -> RedockPollOutcome {
    let now = Instant::now();
    let entry = watch.entry(tab).or_insert(RedockWatch {
        rect: current_rect,
        last_change: now,
        ever_moved: false,
    });
    if (entry.rect.min - current_rect.min).length() >= 1.0 || (entry.rect.max - current_rect.max).length() >= 1.0 {
        *entry = RedockWatch {
            rect: current_rect,
            last_change: now,
            ever_moved: true,
        };
    }
    let vertical_overlap = current_rect.min.y < main_window_rect.max.y && current_rect.max.y > main_window_rect.min.y;
    let near_left = (current_rect.left() - main_window_rect.left()).abs() < snap_px;
    let near_right = (current_rect.right() - main_window_rect.right()).abs() < snap_px;
    if vertical_overlap && (near_left || near_right) && entry.ever_moved {
        if now.duration_since(entry.last_change) >= stable {
            let side = if near_left { DockSide::Left } else { DockSide::Right };
            return RedockPollOutcome::Redock { side };
        }
        return RedockPollOutcome::Watching;
    }
    RedockPollOutcome::Idle
}

// ---------------------------------------------------------------------------
// TabViewer adapter
// ---------------------------------------------------------------------------

/// App-specific rendering hooks a docked tab needs but this crate can't
/// provide generically: what to draw for a tab's content, and its title.
/// Deliberately built fresh each frame by the caller (so it can freely
/// borrow that frame's panel-content data) rather than owning it long-term
/// -- same reasoning as `menu::MenuHost`.
pub trait DockHost<T: TabId> {
    /// Human-readable title for a tab. Centralizing this in one `DockHost`
    /// impl (rather than re-matching the tab type at every call site) is
    /// itself a correctness win: SSP's pre-extraction code had this same
    /// match duplicated 4 times, and a fifth call site was easy to forget.
    fn title(&self, tab: T) -> String;

    /// Render this tab's content into `ui`.
    fn content(&mut self, ui: &mut egui::Ui, tab: T);
}

/// Requested from a tab's right-click context menu -- caller applies it via
/// [`move_panel`] after the dock area has shown, the same way a resolved
/// drag is applied.
#[derive(Clone, Copy, Debug)]
pub enum DockMenuAction<T: TabId> {
    Detach(T),
    MoveTo(T, DockSide),
}

/// Wraps a [`DockHost`] as an `egui_dock::TabViewer`, owning the fixed
/// policy every consuming app needs identically: real OS windows only for
/// floating panels (`allowed_in_windows` fixed to `false` -- egui_dock's own
/// same-window floating surfaces are explicitly not an option here, see the
/// module doc), tabs always closeable, drag tracked passively via
/// [`detect_tab_drag`] rather than through `egui_dock`'s own drag response.
pub struct TabViewerAdapter<'h, T: TabId, H: DockHost<T>> {
    pub host: &'h mut H,
    pub tab_drag: &'h mut Option<TabDrag<T>>,
    pub side: Option<DockSide>,
    pub closed_tabs: Vec<T>,
    pub menu_action: Option<DockMenuAction<T>>,
}

impl<'h, T: TabId, H: DockHost<T>> TabViewerAdapter<'h, T, H> {
    pub fn new(host: &'h mut H, tab_drag: &'h mut Option<TabDrag<T>>, side: Option<DockSide>) -> Self {
        Self {
            host,
            tab_drag,
            side,
            closed_tabs: Vec::new(),
            menu_action: None,
        }
    }
}

impl<'h, T: TabId, H: DockHost<T>> egui_dock::TabViewer for TabViewerAdapter<'h, T, H> {
    type Tab = T;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        self.host.content(ui, *tab);
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        self.host.title(*tab).into()
    }

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        true
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> egui_dock::widgets::tab_viewer::OnCloseResponse {
        self.closed_tabs.push(*tab);
        egui_dock::widgets::tab_viewer::OnCloseResponse::Close
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        false
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab, _node: egui_dock::NodePath) {
        if ui.button("Detach to Floating Window").clicked() {
            self.menu_action = Some(DockMenuAction::Detach(*tab));
            ui.close();
        }
        let target_side = match self.side {
            Some(DockSide::Left) => DockSide::Right,
            _ => DockSide::Left,
        };
        let label = match target_side {
            DockSide::Left => "Move to Left Side",
            DockSide::Right => "Move to Right Side",
        };
        if ui.button(label).clicked() {
            self.menu_action = Some(DockMenuAction::MoveTo(*tab, target_side));
            ui.close();
        }
    }

    /// See the module doc comment for why this uses [`detect_tab_drag`]'s
    /// raw-pointer-state approach instead of `response.dragged()`.
    fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
        if let Some(drag) = detect_tab_drag(*tab, response, self.side) {
            *self.tab_drag = Some(drag);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
    }

    fn drag(tab: u32, source_side: Option<DockSide>, dist: f32) -> TabDrag<u32> {
        TabDrag { tab, source_side, dist }
    }

    #[test]
    fn sub_threshold_release_is_a_click_no_op() {
        let d = drag(1, Some(DockSide::Left), 5.0);
        let outcome = classify_drag_release(&d, true, false, Some(egui::pos2(50.0, 50.0)), Some(rect(0.0, 0.0, 100.0, 100.0)), None, 30.0);
        assert!(matches!(outcome, DragReleaseOutcome::Released { action: ReleaseAction::NoOp }));
    }

    #[test]
    fn drop_on_own_source_dock_is_no_op() {
        let d = drag(1, Some(DockSide::Left), 50.0);
        let left = Some(rect(0.0, 0.0, 100.0, 100.0));
        let outcome = classify_drag_release(&d, true, false, Some(egui::pos2(50.0, 50.0)), left, None, 30.0);
        assert!(matches!(outcome, DragReleaseOutcome::Released { action: ReleaseAction::NoOp }));
    }

    #[test]
    fn drop_on_opposite_dock_moves_there() {
        let d = drag(1, Some(DockSide::Left), 50.0);
        let left = Some(rect(0.0, 0.0, 100.0, 100.0));
        let right = Some(rect(200.0, 0.0, 100.0, 100.0));
        let outcome = classify_drag_release(&d, true, false, Some(egui::pos2(250.0, 50.0)), left, right, 30.0);
        match outcome {
            DragReleaseOutcome::Released {
                action: ReleaseAction::MoveTo { tab, target: DragTarget::Side(DockSide::Right) },
            } => assert_eq!(tab, 1),
            other => panic!("expected move to right side, got {other:?}"),
        }
    }

    #[test]
    fn drop_in_open_space_floats_at_cursor() {
        let d = drag(1, Some(DockSide::Left), 50.0);
        let left = Some(rect(0.0, 0.0, 100.0, 100.0));
        let right = Some(rect(200.0, 0.0, 100.0, 100.0));
        let pos = egui::pos2(500.0, 500.0);
        let outcome = classify_drag_release(&d, true, false, Some(pos), left, right, 30.0);
        match outcome {
            DragReleaseOutcome::Released {
                action: ReleaseAction::MoveTo { tab, target: DragTarget::FloatingAt(p) },
            } => {
                assert_eq!(tab, 1);
                assert_eq!(p, pos);
            }
            other => panic!("expected floating drop, got {other:?}"),
        }
    }

    #[test]
    fn still_dragging_over_open_space_reports_ghost_position() {
        let d = drag(1, Some(DockSide::Left), 50.0);
        let left = Some(rect(0.0, 0.0, 100.0, 100.0));
        let pos = egui::pos2(500.0, 500.0);
        let outcome = classify_drag_release(&d, false, true, Some(pos), left, None, 30.0);
        assert!(matches!(outcome, DragReleaseOutcome::StillDragging { ghost_at: Some(p) } if p == pos));
    }

    #[test]
    fn still_dragging_over_a_dock_has_no_ghost() {
        let d = drag(1, Some(DockSide::Left), 50.0);
        let left = Some(rect(0.0, 0.0, 100.0, 100.0));
        let outcome = classify_drag_release(&d, false, true, Some(egui::pos2(50.0, 50.0)), left, None, 30.0);
        assert!(matches!(outcome, DragReleaseOutcome::StillDragging { ghost_at: None }));
    }

    #[test]
    fn pointer_up_without_release_event_is_lost() {
        let d = drag(1, Some(DockSide::Left), 50.0);
        let outcome = classify_drag_release(&d, false, false, None, None, None, 30.0);
        assert!(matches!(outcome, DragReleaseOutcome::Lost));
    }

    #[test]
    fn poll_redock_is_idle_far_from_any_edge() {
        let mut watch = HashMap::new();
        let main = rect(0.0, 0.0, 800.0, 600.0);
        let far = rect(2000.0, 2000.0, 300.0, 400.0);
        let outcome = poll_redock(1u32, far, main, &mut watch, 40.0, Duration::from_millis(400));
        assert_eq!(outcome, RedockPollOutcome::Idle);
    }

    #[test]
    fn poll_redock_never_fires_for_a_window_that_was_never_dragged() {
        let mut watch = HashMap::new();
        let main = rect(0.0, 0.0, 800.0, 600.0);
        // Spawned already resting right at the left edge, but never moved.
        let resting = rect(-5.0, 100.0, 300.0, 400.0);
        for _ in 0..3 {
            let outcome = poll_redock(1u32, resting, main, &mut watch, 40.0, Duration::from_millis(0));
            assert_eq!(outcome, RedockPollOutcome::Idle, "must not redock without ever_moved");
        }
    }

    #[test]
    fn poll_redock_watches_then_redocks_once_stable_after_a_real_move() {
        let mut watch = HashMap::new();
        let main = rect(0.0, 0.0, 800.0, 600.0);
        let start = rect(2000.0, 100.0, 300.0, 400.0);
        assert_eq!(poll_redock(1u32, start, main, &mut watch, 40.0, Duration::from_millis(400)), RedockPollOutcome::Idle);

        // Drags near the left edge -- rect changed, so `ever_moved` latches true.
        let near = rect(-5.0, 100.0, 300.0, 400.0);
        assert_eq!(
            poll_redock(1u32, near, main, &mut watch, 40.0, Duration::from_millis(400)),
            RedockPollOutcome::Watching,
            "just arrived, not yet stable"
        );

        // Same rect, but pretend enough time has passed by using a zero
        // stability requirement on the next poll.
        let outcome = poll_redock(1u32, near, main, &mut watch, 40.0, Duration::from_millis(0));
        assert_eq!(outcome, RedockPollOutcome::Redock { side: DockSide::Left });
    }
}

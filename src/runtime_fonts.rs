//! Typefaces loaded into egui at runtime, so a font list or a text-style
//! preview can draw each name **in its own face** rather than describing it.
//!
//! Prior art: egui supports this natively (`FontData` for the
//! bytes, `FontFamily::Name` for the family, `Context::add_font` to insert it
//! without replacing the app's own fonts); the community crates
//! `egui_font_loader` and `egui_system_fonts` are built on the same calls. This
//! module adds the three things a picker over hundreds of system fonts needs on
//! top of them:
//!
//! - **Two-phase readiness.** `add_font` only queues: egui binds the family at
//!   the start of the *next* pass, and naming an unbound `FontFamily::Name` in a
//!   `FontId` panics egui. [`family`] therefore returns `None` until the family
//!   is really bound, and asks for the repaint that binds it. Bound-ness is read
//!   from egui's own font definitions, never remembered, so an app that later
//!   calls `set_fonts` (which drops runtime fonts) cannot make this hand back a
//!   family egui no longer knows.
//! - **A per-frame budget.** Every new family rebuilds egui's font atlas, so a
//!   list scrolled through hundreds of faces registers at most
//!   [`NEW_FONTS_PER_FRAME`] faces (and about [`NEW_BYTES_PER_FRAME`]) per
//!   frame and fills in the rest on the following
//!   frames (each registration requests one).
//! - **Per-context, allocation-free bookkeeping.** The registry lives in the
//!   context's own memory (a second context -- another window, an offscreen
//!   render -- has bound nothing and must not be told otherwise) and is mutated
//!   in place; a lookup of an already-bound face allocates nothing.
//!
//! The bytes come from the host app through a loader closure, so the kit never
//! depends on a font database: SDB hands over the same bytes its canvas draws
//! with, SketchPad can hand over its own.

use std::collections::HashMap;
use std::sync::Arc;

/// Font file bytes and the face index inside them (a `.ttc` collection holds
/// several faces; egui's `FontData::index` selects one).
///
/// `Cow` because egui stores `Cow<'static, [u8]>`: a host that keeps one
/// `'static` copy of each font file (SDB does, shared by every face of a
/// collection and every window) hands over `Borrowed` and nothing is copied;
/// `Owned` bytes become egui's own copy.
pub type FontBytes = (std::borrow::Cow<'static, [u8]>, u32);

/// Most new typefaces registered with one context in one frame (each one
/// rebuilds the font atlas once).
pub const NEW_FONTS_PER_FRAME: usize = 4;

/// Most font-file bytes registered with one context in one frame. The first
/// face of a frame is always allowed, so one large face (a 20 MB CJK
/// collection) still loads; it just loads alone.
pub const NEW_BYTES_PER_FRAME: usize = 8 * 1024 * 1024;

/// Whether a frame that has already registered `fonts` faces totalling
/// `bytes` must wait for the next frame.
fn over_budget(fonts: usize, bytes: usize) -> bool {
    fonts >= NEW_FONTS_PER_FRAME || (fonts > 0 && bytes >= NEW_BYTES_PER_FRAME)
}

#[derive(Clone)]
enum Entry {
    /// Handed to egui on frame `.1`; `.0` is the `FontFamily::Name` to draw it
    /// with.
    Requested(egui::FontFamily, u64),
    /// The loader had no bytes for it (not installed, unreadable). Not retried.
    Unavailable,
}

#[derive(Clone, Default)]
struct Registry {
    entries: HashMap<String, Entry>,
    /// The frame `added` and `added_bytes` count for.
    frame: u64,
    added: usize,
    added_bytes: usize,
}

fn registry_id() -> egui::Id {
    egui::Id::new("spatial_ui_kit_runtime_fonts")
}

/// Where a typeface stands in one context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FaceState {
    /// Bound: draw with this family.
    Ready(egui::FontFamily),
    /// Asked for (or waiting for this frame's budget); usable on a later frame.
    Pending,
    /// The loader has no bytes for this name.
    Unavailable,
}

/// The state of `name` in `ctx`, registering it (within this frame's budget)
/// when it is new. `load` is called at most once per name per context.
pub fn face(
    ctx: &egui::Context,
    name: &str,
    load: impl FnOnce(&str) -> Option<FontBytes>,
) -> FaceState {
    request(ctx, name, load, true)
}

/// [`face`] reduced to "the family to draw with, if it is usable this frame".
pub fn family(
    ctx: &egui::Context,
    name: &str,
    load: impl FnOnce(&str) -> Option<FontBytes>,
) -> Option<egui::FontFamily> {
    match face(ctx, name, load) {
        FaceState::Ready(family) => Some(family),
        _ => None,
    }
}

/// Registers every name with `ctx` now, ignoring the per-frame budget, so a
/// window that is about to open has its faces bound on its first frame
/// (registering during that frame would show a fallback face for one frame).
pub fn prefetch<'n>(
    ctx: &egui::Context,
    names: impl IntoIterator<Item = &'n str>,
    load: impl Fn(&str) -> Option<FontBytes>,
) {
    for name in names {
        let _ = request(ctx, name, &load, false);
    }
}

/// Whether `name` has been handed to `ctx` (bound or binding).
pub fn is_requested(ctx: &egui::Context, name: &str) -> bool {
    ctx.data(|d| {
        d.get_temp::<Registry>(registry_id())
            .is_some_and(|r| matches!(r.entries.get(name), Some(Entry::Requested(..))))
    })
}

fn bound(ctx: &egui::Context, family: &egui::FontFamily) -> bool {
    ctx.fonts(|f| f.definitions().families.contains_key(family))
}

fn request(
    ctx: &egui::Context,
    name: &str,
    load: impl FnOnce(&str) -> Option<FontBytes>,
    budgeted: bool,
) -> FaceState {
    if name.is_empty() {
        return FaceState::Unavailable;
    }
    let known = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Registry>(registry_id())
            .entries
            .get(name)
            .cloned()
    });
    match known {
        Some(Entry::Unavailable) => return FaceState::Unavailable,
        Some(Entry::Requested(family, added_on)) => {
            if bound(ctx, &family) {
                return FaceState::Ready(family);
            }
            // Added this frame: it binds at the start of the next pass. Added
            // earlier and still unbound: the app replaced the font definitions
            // since (`set_fonts` drops runtime faces), so add it again below.
            if added_on == ctx.cumulative_frame_nr() {
                return FaceState::Pending;
            }
        }
        None => {}
    }
    let frame = ctx.cumulative_frame_nr();
    if budgeted {
        let over = ctx.data_mut(|d| {
            let r = d.get_temp_mut_or_default::<Registry>(registry_id());
            if r.frame != frame {
                r.frame = frame;
                r.added = 0;
                r.added_bytes = 0;
            }
            over_budget(r.added, r.added_bytes)
        });
        if over {
            ctx.request_repaint();
            return FaceState::Pending;
        }
    }
    let Some((bytes, index)) = load(name).filter(|(b, _)| !b.is_empty()) else {
        ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Registry>(registry_id())
                .entries
                .insert(name.to_owned(), Entry::Unavailable)
        });
        return FaceState::Unavailable;
    };
    let size = bytes.len();
    let data = egui::FontData {
        font: bytes,
        index,
        tweak: Default::default(),
    };
    let family = egui::FontFamily::Name(Arc::<str>::from(name));
    ctx.add_font(egui::epaint::text::FontInsert {
        name: format!("spatial_ui_kit_runtime:{name}"),
        data,
        families: vec![egui::epaint::text::InsertFontFamily {
            // A named family of its own, never `Proportional`: a runtime face
            // must not change any other text in the app. Glyphs it lacks are
            // simply missing (shown as the replacement box), which is the truth
            // about that face.
            family: family.clone(),
            priority: egui::epaint::text::FontPriority::Highest,
        }],
    });
    ctx.data_mut(|d| {
        let r = d.get_temp_mut_or_default::<Registry>(registry_id());
        if r.frame != frame {
            r.frame = frame;
            r.added = 0;
            r.added_bytes = 0;
        }
        r.added += 1;
        r.added_bytes += size;
        r.entries
            .insert(name.to_owned(), Entry::Requested(family, frame));
    });
    // Binds at the start of the next pass; without a repaint a still window
    // would keep the fallback indefinitely.
    ctx.request_repaint();
    FaceState::Pending
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTER: &[u8] = include_bytes!("../assets/Inter.ttf");

    fn warmed() -> egui::Context {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        ctx
    }

    fn inter(_: &str) -> Option<FontBytes> {
        Some((std::borrow::Cow::Borrowed(INTER), 0))
    }

    fn pass(ctx: &egui::Context, f: &mut dyn FnMut(&egui::Context)) {
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| f(ui.ctx()));
    }

    #[test]
    fn the_budget_counts_faces_and_bytes_but_always_lets_one_face_through() {
        assert!(!over_budget(0, 0));
        assert!(
            !over_budget(0, 50 * 1024 * 1024),
            "the first face always loads"
        );
        assert!(
            over_budget(1, NEW_BYTES_PER_FRAME),
            "a big face loads alone"
        );
        assert!(!over_budget(3, 1024));
        assert!(over_budget(NEW_FONTS_PER_FRAME, 0));
    }

    #[test]
    fn a_face_is_pending_on_the_frame_it_is_added_and_ready_after() {
        let ctx = warmed();
        let mut states = Vec::new();
        for _ in 0..3 {
            pass(&ctx, &mut |c| states.push(face(c, "Face A", inter)));
        }
        assert_eq!(states[0], FaceState::Pending, "never named while unbound");
        assert!(matches!(states[1], FaceState::Ready(_)), "{states:?}");
        // And the bound face really lays text out.
        let FaceState::Ready(family) = states[2].clone() else {
            panic!()
        };
        let w = ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                "Ag".into(),
                egui::FontId::new(20.0, family),
                egui::Color32::WHITE,
            )
            .size()
            .x
        });
        assert!(w > 0.0);
    }

    #[test]
    fn the_loader_runs_once_and_a_missing_face_is_not_retried() {
        let ctx = warmed();
        let mut calls = 0;
        for _ in 0..3 {
            pass(&ctx, &mut |c| {
                let s = face(c, "Nowhere", |_| {
                    calls += 1;
                    None
                });
                assert_eq!(s, FaceState::Unavailable);
            });
        }
        assert_eq!(calls, 1);
        let mut loads = 0;
        for _ in 0..3 {
            pass(&ctx, &mut |c| {
                let _ = face(c, "Face B", |n| {
                    loads += 1;
                    inter(n)
                });
            });
        }
        assert_eq!(loads, 1, "a registered face is a lookup, not a reload");
    }

    #[test]
    fn at_most_the_budget_is_registered_per_frame() {
        let ctx = warmed();
        let names: Vec<String> = (0..10).map(|i| format!("Face {i}")).collect();
        let mut first = 0;
        pass(&ctx, &mut |c| {
            for n in &names {
                let _ = face(c, n, inter);
            }
            first = names.iter().filter(|n| is_requested(c, n)).count();
        });
        assert_eq!(first, NEW_FONTS_PER_FRAME);
        // The rest arrive over the next frames.
        for _ in 0..4 {
            pass(&ctx, &mut |c| {
                for n in &names {
                    let _ = face(c, n, inter);
                }
            });
        }
        let mut ready = 0;
        pass(&ctx, &mut |c| {
            ready = names
                .iter()
                .filter(|n| matches!(face(c, n, inter), FaceState::Ready(_)))
                .count();
        });
        assert_eq!(ready, names.len());
    }

    #[test]
    fn a_second_context_does_not_inherit_the_first_contexts_faces() {
        let a = warmed();
        for _ in 0..2 {
            pass(&a, &mut |c| {
                let _ = face(c, "Shared", inter);
            });
        }
        let b = warmed();
        let mut s = None;
        pass(&b, &mut |c| s = Some(face(c, "Shared", inter)));
        assert_eq!(s, Some(FaceState::Pending), "b must bind it for itself");
    }

    #[test]
    fn replacing_the_font_definitions_is_survived_by_re_adding() {
        let ctx = warmed();
        for _ in 0..2 {
            pass(&ctx, &mut |c| {
                let _ = face(c, "Face C", inter);
            });
        }
        // The app re-applies its UI typeface: runtime faces are dropped.
        ctx.set_fonts(egui::FontDefinitions::default());
        let mut states = Vec::new();
        for _ in 0..4 {
            pass(&ctx, &mut |c| states.push(face(c, "Face C", inter)));
        }
        assert!(
            !matches!(states[0], FaceState::Ready(_)),
            "an unbound family is never handed out: {states:?}"
        );
        assert!(matches!(states[3], FaceState::Ready(_)), "{states:?}");
    }

    #[test]
    fn prefetch_ignores_the_budget() {
        let ctx = warmed();
        let names: Vec<String> = (0..9).map(|i| format!("P {i}")).collect();
        pass(&ctx, &mut |c| {
            prefetch(c, names.iter().map(String::as_str), inter)
        });
        let mut ready = 0;
        pass(&ctx, &mut |c| {
            ready = names
                .iter()
                .filter(|n| matches!(face(c, n, inter), FaceState::Ready(_)))
                .count();
        });
        assert_eq!(ready, names.len());
    }
}

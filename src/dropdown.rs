//! The one dropdown every Spatial app uses (SDB panels, the Pattern Editor,
//! the FILL ribbon's Type grid).
//!
//! # What it looks like
//!
//! Closed, it is a rounded control showing the current value (optionally with
//! a thumbnail) and a small arrow. Open, the control and its list become ONE
//! rounded box: the box starts exactly at the control's rectangle, the
//! control's own row stays at the top of the box (highlighted, because it is
//! the current selection) and the remaining options hang directly beneath it.
//! There is no second box, no gap and no shadow, so the edges and corners of
//! control and list are the same edges and corners. Colours come from the
//! theme ([`DropdownStyle::from_visuals`] / [`DropdownStyle::from_palette`]);
//! the highlight is a lighter/darker tint of the text colour, never a hue.
//!
//! In [`DropdownLayout::Grid`] the box holds a grid of tiles (for example the
//! FILL ribbon's Type picker) below the same control row, with the current
//! tile highlighted.
//!
//! # Where it opens
//!
//! Down, from the control, and up only when the list does not fit below but
//! does fit above ([`choose_direction`]); if it fits neither way it opens on
//! the roomier side and scrolls. The space it is measured against is the
//! `bounds` rectangle ([`Dropdown::bounds`]), which defaults to the window's
//! content rectangle. **egui paints every popup inside its OS window**, so
//! that is the most a popup can use; see
//! `docs/design/2026-10-18_dropdown-design.md` in SpatialDrawingBoard for why
//! the list is not opened in a borderless child window instead, and what
//! passing the monitor rectangle as `bounds` would take.
//!
//! # Keyboard
//!
//! The closed control takes focus (Tab) and draws a focus ring. Enter, Space
//! or a click opens it. Open: Up/Down (and Left/Right in a grid) move the
//! highlight, Home/End jump, Enter/Space choose, Esc closes, and typing a
//! letter jumps to the next option starting with it. A click anywhere outside
//! closes it. Every control and row announces itself to accessibility tools
//! (`widget_info`).

use egui::{
    Align, Align2, Color32, CornerRadius, Event, EventFilter, FontId, Id, Key, Layout, Modifiers,
    Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, WidgetInfo, WidgetType,
};

use crate::sketch::truncate_middle_with;
use crate::theme::ThemePalette;

/// Paints one option's thumbnail into the rect it is given.
pub type ThumbPaint<'a> = Box<dyn Fn(&Ui, Rect) + 'a>;

/// One entry of a [`Dropdown`].
pub struct DropdownOption<'a> {
    /// The text shown for the entry.
    pub text: String,
    /// Optional thumbnail painter, called for the closed control (when this is
    /// the current option) and for the entry's own row or tile.
    pub thumb: Option<ThumbPaint<'a>>,
    /// Extra left indent in points (nested categories).
    pub indent: f32,
    /// Draw the text in italics.
    pub italic: bool,
    /// Tooltip; by default a text that had to be shortened shows in full.
    pub tooltip: Option<String>,
}

impl<'a> DropdownOption<'a> {
    /// A plain text option.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            thumb: None,
            indent: 0.0,
            italic: false,
            tooltip: None,
        }
    }
    /// Adds a thumbnail painter.
    pub fn with_thumb(mut self, paint: impl Fn(&Ui, Rect) + 'a) -> Self {
        self.thumb = Some(Box::new(paint));
        self
    }
    /// Indents the row by `points`.
    pub fn indent(mut self, points: f32) -> Self {
        self.indent = points;
        self
    }
    /// Draws the text in italics.
    pub fn italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }
    /// Sets the hover tooltip.
    pub fn tooltip(mut self, tip: impl Into<String>) -> Self {
        self.tooltip = Some(tip.into());
        self
    }
}

impl From<&str> for DropdownOption<'_> {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}
impl From<String> for DropdownOption<'_> {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// How the options are laid out under the control row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropdownLayout {
    /// One option per row.
    List,
    /// Tiles (thumbnail over name), `columns` per row, each `tile` points.
    Grid {
        /// Tiles per row.
        columns: usize,
        /// Size of one tile in points.
        tile: Vec2,
    },
}

/// Colours and shape of a dropdown, derived from the theme.
#[derive(Clone, Debug, PartialEq)]
pub struct DropdownStyle {
    /// Closed control fill.
    pub control_fill: Color32,
    /// Closed control fill while hovered.
    pub control_fill_hovered: Color32,
    /// Fill of the open box behind the rows.
    pub box_fill: Color32,
    /// Outline of the closed control and of the open box.
    pub border: Color32,
    /// Outline while hovered or open.
    pub border_strong: Color32,
    /// Text and arrow colour.
    pub text: Color32,
    /// Fill of the current selection's row.
    pub selected_fill: Color32,
    /// Fill of the row under the pointer or the keyboard cursor.
    pub hover_fill: Color32,
    /// Focus ring (keyboard focus on the control, cursor row).
    pub focus: Stroke,
    /// Corner radius of the control and the box.
    pub corner_radius: u8,
}

impl DropdownStyle {
    /// Derives the style from egui visuals, which already carry the theme.
    pub fn from_visuals(v: &egui::Visuals) -> Self {
        let text = v.widgets.inactive.fg_stroke.color;
        let box_fill = v.extreme_bg_color;
        let r = v.widgets.inactive.corner_radius.nw.max(3);
        Self {
            control_fill: v.widgets.inactive.weak_bg_fill,
            control_fill_hovered: v.widgets.hovered.weak_bg_fill,
            box_fill,
            border: v.widgets.inactive.bg_stroke.color,
            border_strong: v.widgets.hovered.bg_stroke.color,
            text,
            selected_fill: box_fill.lerp_to_gamma(text, 0.22),
            hover_fill: box_fill.lerp_to_gamma(text, 0.11),
            focus: v.selection.stroke,
            corner_radius: r,
        }
    }

    /// Takes the control and surface colours from a [`ThemePalette`]'s tokens
    /// (`control.cap_*` for the control, `surface.background` for the box),
    /// with text and focus ring from `v`.
    pub fn from_palette(p: &ThemePalette, v: &egui::Visuals) -> Self {
        let mut s = Self::from_visuals(v);
        if let (Ok(cap), Ok(cap_border), Ok(cap_hot), Ok(surface)) = (
            p.control.cap_background.color32(),
            p.control.cap_border.color32(),
            p.control.cap_border_hovered.color32(),
            p.surface.background.color32(),
        ) {
            s.control_fill = cap;
            s.control_fill_hovered = cap.lerp_to_gamma(s.text, 0.08);
            s.box_fill = surface;
            s.border = cap_border;
            s.border_strong = cap_hot;
            s.selected_fill = surface.lerp_to_gamma(s.text, 0.22);
            s.hover_fill = surface.lerp_to_gamma(s.text, 0.11);
        }
        s
    }
}

/// Which way the open box extends from the control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// The box's top edge is the control's top edge; the list is below.
    Down,
    /// The box's bottom edge is the control's bottom edge; the list is above.
    Up,
}

/// Where the open box goes, from [`choose_direction`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Direction the box extends from the control.
    pub direction: Direction,
    /// Height of the box, including the control row. Less than the wanted
    /// height when the list has to scroll.
    pub height: f32,
}

/// Decides which way the open box extends. `control` is the closed control's
/// rectangle, `wanted_height` the full height of the box (control row plus
/// all options) and `bounds` the region the box may use: the monitor's work
/// area where a popup can leave its window, otherwise the window's content
/// rectangle.
///
/// Down unless the box does not fit below; up only when it fits above; when
/// it fits neither way, the roomier side with a reduced height (scrolling).
pub fn choose_direction(control: Rect, wanted_height: f32, bounds: Rect) -> Placement {
    let below = (bounds.bottom() - control.top()).max(0.0);
    let above = (control.bottom() - bounds.top()).max(0.0);
    if wanted_height <= below {
        Placement {
            direction: Direction::Down,
            height: wanted_height,
        }
    } else if wanted_height <= above {
        Placement {
            direction: Direction::Up,
            height: wanted_height,
        }
    } else if below >= above {
        Placement {
            direction: Direction::Down,
            height: below.max(control.height()),
        }
    } else {
        Placement {
            direction: Direction::Up,
            height: above.max(control.height()),
        }
    }
}

/// Corner radii for a row that may touch the box's top and/or bottom edge.
pub fn row_rounding(radius: u8, touches_top: bool, touches_bottom: bool) -> CornerRadius {
    CornerRadius {
        nw: if touches_top { radius } else { 0 },
        ne: if touches_top { radius } else { 0 },
        sw: if touches_bottom { radius } else { 0 },
        se: if touches_bottom { radius } else { 0 },
    }
}

/// Index of the next option after `from` (wrapping) whose text starts with
/// `typed`, ignoring case. `None` when nothing matches.
pub fn type_ahead(texts: &[&str], from: Option<usize>, typed: char) -> Option<usize> {
    let n = texts.len();
    let typed = typed.to_lowercase().next()?;
    let start = from.map_or(0, |f| f + 1);
    (0..n).map(|k| (start + k) % n).find(|&i| {
        texts[i]
            .chars()
            .next()
            .and_then(|c| c.to_lowercase().next())
            == Some(typed)
    })
}

/// Moves a highlight by `delta` in a top-to-bottom list of option indices,
/// clamped to the ends. With no highlight, forward lands on the first entry
/// and backward on the last.
fn step_list(nav: &[usize], cursor: Option<usize>, delta: isize) -> Option<usize> {
    if nav.is_empty() {
        return None;
    }
    match cursor.and_then(|c| nav.iter().position(|&i| i == c)) {
        Some(p) => Some(nav[(p as isize + delta).clamp(0, nav.len() as isize - 1) as usize]),
        None if delta >= 0 => Some(nav[0]),
        None => Some(nav[nav.len() - 1]),
    }
}

#[derive(Clone, Default)]
struct State {
    open: bool,
    cursor: Option<usize>,
    /// The cursor is shown only after a key moved it, not while the pointer drives.
    kbd: bool,
    scroll_to_cursor: bool,
}

/// What [`Dropdown::show`] returns.
pub struct DropdownResponse {
    /// The closed control's response (hover, focus, rect).
    pub response: egui::Response,
    /// The option chosen this frame, if any.
    pub chosen: Option<usize>,
    /// The open box's rectangle, when open (control row included).
    pub popup_rect: Option<Rect>,
    /// Which way the box extends, when open.
    pub direction: Option<Direction>,
}

/// The shared dropdown. Build it, add options, call [`Self::show`].
///
/// ```ignore
/// let out = Dropdown::new("units").options(["mm", "cm", "m"]).selected(Some(i)).show(ui);
/// if let Some(i) = out.chosen { value = i; }
/// ```
pub struct Dropdown<'a> {
    id_salt: Id,
    text: Option<String>,
    italic_text: bool,
    options: Vec<DropdownOption<'a>>,
    selected: Option<usize>,
    width: Option<f32>,
    min_popup_width: f32,
    truncate: bool,
    layout: DropdownLayout,
    font: Option<FontId>,
    row_height: Option<f32>,
    thumb_side: Option<f32>,
    rect: Option<Rect>,
    max_popup_height: f32,
    empty_text: String,
    bounds: Option<Rect>,
    style: Option<DropdownStyle>,
}

impl<'a> Dropdown<'a> {
    /// A dropdown with a unique id source.
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            text: None,
            italic_text: false,
            options: Vec::new(),
            selected: None,
            width: None,
            min_popup_width: 0.0,
            truncate: false,
            layout: DropdownLayout::List,
            font: None,
            row_height: None,
            thumb_side: None,
            rect: None,
            max_popup_height: f32::INFINITY,
            empty_text: "No options".to_owned(),
            bounds: None,
            style: None,
        }
    }
    /// Text of the closed control; by default the selected option's text.
    pub fn selected_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }
    /// Draws the control's text in italics ("Multiple").
    pub fn italic_text(mut self, italic: bool) -> Self {
        self.italic_text = italic;
        self
    }
    /// Adds one option.
    pub fn option(mut self, option: impl Into<DropdownOption<'a>>) -> Self {
        self.options.push(option.into());
        self
    }
    /// Adds options.
    pub fn options<I>(mut self, options: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<DropdownOption<'a>>,
    {
        self.options.extend(options.into_iter().map(Into::into));
        self
    }
    /// Index of the current option (highlighted, shown first in a list).
    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }
    /// Minimum width of the control (the box matches it).
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    /// The box is at least this wide even when the control is narrower.
    pub fn min_popup_width(mut self, width: f32) -> Self {
        self.min_popup_width = width;
        self
    }
    /// Keep the control at `width` and shorten its text instead of widening.
    pub fn truncate(mut self) -> Self {
        self.truncate = true;
        self
    }
    /// Lay the options out as a grid of tiles.
    pub fn grid(mut self, columns: usize, tile: Vec2) -> Self {
        self.layout = DropdownLayout::Grid {
            columns: columns.max(1),
            tile,
        };
        self
    }
    /// Font for the control and the rows (default: the button font).
    pub fn font(mut self, font: FontId) -> Self {
        self.font = Some(font);
        self
    }
    /// Height of one list row (default: from the font).
    pub fn row_height(mut self, h: f32) -> Self {
        self.row_height = Some(h);
        self
    }
    /// Side of the square thumbnail in the control and in list rows.
    pub fn thumb_side(mut self, side: f32) -> Self {
        self.thumb_side = Some(side);
        self
    }
    /// Place the control at an explicit rectangle instead of allocating one.
    pub fn at_rect(mut self, rect: Rect) -> Self {
        self.rect = Some(rect);
        self
    }
    /// Tallest the box grows before its options scroll.
    pub fn max_popup_height(mut self, h: f32) -> Self {
        self.max_popup_height = h;
        self
    }
    /// Message shown when there are no options.
    pub fn empty_text(mut self, text: impl Into<String>) -> Self {
        self.empty_text = text.into();
        self
    }
    /// Region the open box must fit in (default: the window's content
    /// rectangle). See [`choose_direction`].
    pub fn bounds(mut self, bounds: Rect) -> Self {
        self.bounds = Some(bounds);
        self
    }
    /// Overrides the theme-derived style.
    pub fn style(mut self, style: DropdownStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Shows the dropdown over `items` and returns the item chosen this frame.
    /// `label` names each item and `is_current` marks the current one (shown
    /// on top, highlighted). The convenience form for a list of values (enum
    /// variants, units) where the caller maps the pick back to the value.
    pub fn show_items<T: Clone>(
        mut self,
        ui: &mut Ui,
        items: impl IntoIterator<Item = T>,
        label: impl Fn(&T) -> String,
        is_current: impl Fn(&T) -> bool,
    ) -> Option<T> {
        let items: Vec<T> = items.into_iter().collect();
        self.selected = items.iter().position(&is_current);
        self.options
            .extend(items.iter().map(|t| DropdownOption::new(label(t))));
        self.show(ui).chosen.map(|i| items[i].clone())
    }

    /// Draws the control and, while open, the box. Returns the choice made
    /// this frame.
    pub fn show(self, ui: &mut Ui) -> DropdownResponse {
        let style = self
            .style
            .clone()
            .unwrap_or_else(|| DropdownStyle::from_visuals(ui.visuals()));
        let font = self
            .font
            .clone()
            .unwrap_or_else(|| egui::TextStyle::Button.resolve(ui.style()));
        let n = self.options.len();
        let selected = self.selected.filter(|&s| s < n);
        let list_mode = self.layout == DropdownLayout::List;
        let header_is_option = list_mode && selected.is_some();
        let row_h = self
            .row_height
            .unwrap_or_else(|| ui.spacing().interact_size.y.max(font.size + 8.0));
        let ctrl_h = self
            .rect
            .map(|r| r.height())
            .unwrap_or_else(|| ui.spacing().interact_size.y.max(font.size + 6.0));
        let thumb_side = self.thumb_side.unwrap_or(ctrl_h - 6.0).min(row_h - 2.0);
        let pad = 8.0;
        let arrow_w = 22.0;

        let text = self
            .text
            .clone()
            .or_else(|| selected.map(|s| self.options[s].text.clone()))
            .unwrap_or_default();
        let header_thumb = if list_mode {
            selected.and_then(|s| self.options[s].thumb.as_ref())
        } else {
            None
        };
        let text_w = ui
            .painter()
            .layout_no_wrap(text.clone(), font.clone(), Color32::WHITE)
            .size()
            .x;
        let thumb_w = if header_thumb.is_some() {
            thumb_side + 6.0
        } else {
            0.0
        };
        let natural_w = pad + thumb_w + text_w + arrow_w;

        let id = ui.make_persistent_id(self.id_salt);
        let control = match self.rect {
            Some(r) => r,
            None => {
                let base = self.width.unwrap_or(ui.spacing().combo_width);
                let w = if self.truncate {
                    base
                } else {
                    base.max(natural_w)
                };
                ui.allocate_space(Vec2::new(w, ctrl_h)).1
            }
        };
        let response = ui.interact(control, id, Sense::click());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, ui.is_enabled(), &text));

        let mut st: State = ui.data(|d| d.get_temp(id)).unwrap_or_default();
        let mut chosen = None;

        let mut just_opened = false;
        if response.clicked() && !st.open {
            just_opened = true;
            st.open = true;
            st.cursor = selected;
            st.kbd = false;
            response.request_focus();
        }

        // Geometry of the open box.
        let bounds = self.bounds.unwrap_or_else(|| ui.ctx().content_rect());
        let others: Vec<usize> = if header_is_option {
            (0..n).filter(|&i| Some(i) != selected).collect()
        } else {
            (0..n).collect()
        };
        let (cols, tile) = match self.layout {
            DropdownLayout::Grid { columns, tile } => (columns, tile),
            DropdownLayout::List => (1, Vec2::ZERO),
        };
        let grid_gap = 4.0;
        let grid_pad = 4.0;
        let body_h = if n == 0 {
            row_h
        } else if list_mode {
            others.len() as f32 * row_h
        } else {
            let rows = n.div_ceil(cols);
            2.0 * grid_pad + rows as f32 * tile.y + (rows.saturating_sub(1)) as f32 * grid_gap
        };
        let grid_w = if list_mode {
            0.0
        } else {
            2.0 * grid_pad + cols as f32 * tile.x + (cols - 1) as f32 * grid_gap + 2.0
        };
        let popup_w = control.width().max(self.min_popup_width).max(grid_w);
        let wanted = (ctrl_h + body_h + 1.0).min(self.max_popup_height.max(ctrl_h + row_h));
        let placement = choose_direction(control, wanted, bounds);
        let up = placement.direction == Direction::Up;
        let mut x = control.left();
        if x + popup_w > bounds.right() {
            x = (bounds.right() - popup_w).max(bounds.left());
        }
        let box_rect = if up {
            Rect::from_min_max(
                Pos2::new(x, control.bottom() - placement.height),
                Pos2::new(x + popup_w, control.bottom()),
            )
        } else {
            Rect::from_min_size(
                Pos2::new(x, control.top()),
                Vec2::new(popup_w, placement.height),
            )
        };
        let header_rect = if up {
            Rect::from_min_size(
                Pos2::new(x, control.bottom() - ctrl_h),
                Vec2::new(popup_w, ctrl_h),
            )
        } else {
            Rect::from_min_size(box_rect.min, Vec2::new(popup_w, ctrl_h))
        };
        let body_rect = if up {
            Rect::from_min_max(
                Pos2::new(x, box_rect.top() + 1.0),
                Pos2::new(x + popup_w, header_rect.top()),
            )
        } else {
            Rect::from_min_max(
                Pos2::new(x, header_rect.bottom()),
                Pos2::new(x + popup_w, box_rect.bottom() - 1.0),
            )
        };

        // Visual (top to bottom) order of list rows, header included when it
        // is the current option.
        let mut visual_rows = others.clone();
        if up && list_mode {
            visual_rows.reverse();
        }
        let nav: Vec<usize> = if list_mode {
            let mut v = visual_rows.clone();
            if let Some(s) = selected {
                if up {
                    v.push(s);
                } else {
                    v.insert(0, s);
                }
            }
            v
        } else {
            (0..n).collect()
        };

        // Keyboard. (Not on the frame that opened it: the Enter or Space that
        // opened it is still in this frame's input.)
        if st.open && !just_opened {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    EventFilter {
                        tab: false,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                )
            });
            let (down_k, up_k, left_k, right_k, home_k, end_k, enter_k, space_k, esc_k, typed) = ui
                .input_mut(|i| {
                    let m = Modifiers::NONE;
                    let typed: Vec<char> = i
                        .events
                        .iter()
                        .filter_map(|e| match e {
                            Event::Text(t) => t.chars().find(|c| c.is_alphanumeric()),
                            _ => None,
                        })
                        .collect();
                    (
                        i.consume_key(m, Key::ArrowDown),
                        i.consume_key(m, Key::ArrowUp),
                        i.consume_key(m, Key::ArrowLeft),
                        i.consume_key(m, Key::ArrowRight),
                        i.consume_key(m, Key::Home),
                        i.consume_key(m, Key::End),
                        i.consume_key(m, Key::Enter),
                        i.consume_key(m, Key::Space),
                        i.consume_key(m, Key::Escape),
                        typed,
                    )
                });
            let before = st.cursor;
            if list_mode {
                if down_k {
                    st.cursor = step_list(&nav, st.cursor, 1);
                }
                if up_k {
                    st.cursor = step_list(&nav, st.cursor, -1);
                }
                if home_k {
                    st.cursor = nav.first().copied();
                }
                if end_k {
                    st.cursor = nav.last().copied();
                }
            } else if n > 0 {
                let cur = st.cursor.unwrap_or(0) as isize;
                let mut next = None;
                let c = cols as isize;
                if st.cursor.is_none() && (down_k || up_k || left_k || right_k) {
                    next = Some(0);
                } else if down_k {
                    next = Some((cur + c).min(n as isize - 1));
                } else if up_k {
                    next = Some((cur - c).max(0));
                } else if right_k {
                    next = Some((cur + 1).min(n as isize - 1));
                } else if left_k {
                    next = Some((cur - 1).max(0));
                } else if home_k {
                    next = Some(0);
                } else if end_k {
                    next = Some(n as isize - 1);
                }
                if let Some(nx) = next {
                    st.cursor = Some(nx as usize);
                }
            }
            if !typed.is_empty() {
                let texts: Vec<&str> = self.options.iter().map(|o| o.text.as_str()).collect();
                for ch in typed {
                    if let Some(i) = type_ahead(&texts, st.cursor, ch) {
                        st.cursor = Some(i);
                    }
                }
            }
            if st.cursor != before {
                st.kbd = true;
                st.scroll_to_cursor = true;
            }
            if esc_k {
                st.open = false;
            } else if enter_k || space_k {
                match st.cursor {
                    Some(c) if header_is_option && Some(c) == selected => st.open = false,
                    Some(c) => {
                        chosen = Some(c);
                        st.open = false;
                    }
                    None => st.open = false,
                }
            }
            // A press outside both the box and the control closes it.
            if st.open && ui.input(|i| i.pointer.any_pressed()) {
                if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                    if !box_rect.contains(p) {
                        st.open = false;
                    }
                }
            }
            if ui.input(|i| i.pointer.delta() != Vec2::ZERO) {
                st.kbd = false;
            }
        }

        // The closed face is always painted; when open the box covers it.
        let focus = response.has_focus();
        {
            let fill = if response.hovered() {
                style.control_fill_hovered
            } else {
                style.control_fill
            };
            let r = style.corner_radius;
            ui.painter().rect_filled(control, r, fill);
            let face = Face {
                text: &text,
                italic: self.italic_text,
                font: &font,
                color: style.text,
                thumb: header_thumb,
                thumb_side,
                arrow: Some(false),
                pad,
                arrow_w,
            };
            paint_face(ui, control, &face);
            let stroke = if response.hovered() || st.open {
                style.border_strong
            } else {
                style.border
            };
            ui.painter()
                .rect_stroke(control, r, Stroke::new(1.0, stroke), StrokeKind::Inside);
            if focus && !st.open {
                ui.painter().rect_stroke(
                    control.expand(1.0),
                    r + 1,
                    style.focus,
                    StrokeKind::Outside,
                );
            }
        }

        let mut popup_rect = None;
        if st.open {
            popup_rect = Some(box_rect);
            let mut close = false;
            let mut hovered_any = None;
            egui::Area::new(id.with("box"))
                .order(Order::Foreground)
                .fixed_pos(box_rect.min)
                .constrain(false)
                .sense(Sense::hover())
                .show(ui.ctx(), |pui| {
                    pui.set_clip_rect(box_rect.expand(2.0).intersect(pui.clip_rect()));
                    let r = style.corner_radius;
                    let ri = r.saturating_sub(1);
                    pui.painter().rect_filled(box_rect, r, style.box_fill);

                    // Header row: the control itself, at the box's edge.
                    let inner_header = header_rect.shrink(1.0);
                    let h_resp = pui.interact(header_rect, id.with("header"), Sense::click());
                    let has_body = n > 0 && (!list_mode || !others.is_empty() || !header_is_option);
                    let head_round = row_rounding(ri, !up || !has_body, up || !has_body);
                    if header_is_option {
                        pui.painter()
                            .rect_filled(inner_header, head_round, style.selected_fill);
                    } else if h_resp.hovered() {
                        pui.painter()
                            .rect_filled(inner_header, head_round, style.hover_fill);
                    }
                    if header_is_option && st.kbd && st.cursor == selected && focus {
                        pui.painter().rect_stroke(
                            inner_header,
                            head_round,
                            style.focus,
                            StrokeKind::Inside,
                        );
                    }
                    let face = Face {
                        text: &text,
                        italic: self.italic_text,
                        font: &font,
                        color: style.text,
                        thumb: header_thumb,
                        thumb_side,
                        arrow: Some(true),
                        pad,
                        arrow_w,
                    };
                    paint_face(pui, header_rect, &face);
                    h_resp.widget_info(|| {
                        WidgetInfo::selected(
                            WidgetType::SelectableLabel,
                            true,
                            header_is_option,
                            &text,
                        )
                    });
                    if h_resp.clicked() {
                        close = true;
                    }

                    // Body.
                    let mut body = pui.new_child(
                        UiBuilder::new()
                            .max_rect(body_rect)
                            .layout(Layout::top_down(Align::Min)),
                    );
                    body.set_clip_rect(body_rect.intersect(pui.clip_rect()));
                    let visible_h = body_rect.height();
                    egui::ScrollArea::vertical()
                        .id_salt(id.with("scroll"))
                        .max_height(visible_h)
                        .auto_shrink([false, false])
                        .show(&mut body, |sui| {
                            sui.spacing_mut().item_spacing = Vec2::ZERO;
                            if n == 0 {
                                let (rect, _) = sui.allocate_exact_size(
                                    Vec2::new(popup_w - 2.0, row_h),
                                    Sense::hover(),
                                );
                                sui.painter().text(
                                    Pos2::new(rect.left() + pad, rect.center().y),
                                    Align2::LEFT_CENTER,
                                    &self.empty_text,
                                    font.clone(),
                                    style.text.gamma_multiply(0.6),
                                );
                            } else if list_mode {
                                let m = visual_rows.len();
                                for (k, &i) in visual_rows.iter().enumerate() {
                                    let opt = &self.options[i];
                                    let (rect, row) = sui.allocate_exact_size(
                                        Vec2::new(popup_w - 2.0, row_h),
                                        Sense::click(),
                                    );
                                    let rect = rect.translate(Vec2::new(1.0, 0.0));
                                    let round = row_rounding(ri, up && k == 0, !up && k == m - 1);
                                    let is_cursor = st.kbd && st.cursor == Some(i);
                                    if row.hovered() {
                                        hovered_any = Some(i);
                                    }
                                    if row.hovered() || is_cursor {
                                        sui.painter().rect_filled(rect, round, style.hover_fill);
                                    }
                                    if is_cursor && focus {
                                        sui.painter().rect_stroke(
                                            rect,
                                            round,
                                            style.focus,
                                            StrokeKind::Inside,
                                        );
                                    }
                                    if is_cursor && st.scroll_to_cursor {
                                        sui.scroll_to_rect(rect, None);
                                    }
                                    let mut left = rect.left() + pad + opt.indent;
                                    if let Some(paint) = &opt.thumb {
                                        let t = Rect::from_min_size(
                                            Pos2::new(left, rect.center().y - thumb_side / 2.0),
                                            Vec2::splat(thumb_side),
                                        );
                                        paint(sui, t);
                                        left = t.right() + 6.0;
                                    }
                                    let shown = paint_text(
                                        sui,
                                        Pos2::new(left, rect.center().y),
                                        rect.right() - left - pad,
                                        &opt.text,
                                        opt.italic,
                                        &font,
                                        style.text,
                                    );
                                    row.widget_info(|| {
                                        WidgetInfo::selected(
                                            WidgetType::SelectableLabel,
                                            true,
                                            false,
                                            &opt.text,
                                        )
                                    });
                                    let row = if let Some(tip) = &opt.tooltip {
                                        row.on_hover_text(tip)
                                    } else if shown != opt.text {
                                        row.on_hover_text(&opt.text)
                                    } else {
                                        row
                                    };
                                    if row.clicked() {
                                        chosen = Some(i);
                                        close = true;
                                    }
                                }
                            } else {
                                let rows = n.div_ceil(cols);
                                let grid_h = 2.0 * grid_pad
                                    + rows as f32 * tile.y
                                    + (rows.saturating_sub(1)) as f32 * grid_gap;
                                let (area, _) = sui.allocate_exact_size(
                                    Vec2::new(popup_w - 2.0, grid_h),
                                    Sense::hover(),
                                );
                                let origin = area.min + Vec2::new(grid_pad + 1.0, grid_pad);
                                for (i, opt) in self.options.iter().enumerate() {
                                    let (cx, cy) = (i % cols, i / cols);
                                    let tr = Rect::from_min_size(
                                        origin
                                            + Vec2::new(
                                                cx as f32 * (tile.x + grid_gap),
                                                cy as f32 * (tile.y + grid_gap),
                                            ),
                                        tile,
                                    );
                                    let resp =
                                        sui.interact(tr, id.with(("tile", i)), Sense::click());
                                    let is_cur = Some(i) == selected;
                                    let is_cursor = st.kbd && st.cursor == Some(i);
                                    if resp.hovered() {
                                        hovered_any = Some(i);
                                    }
                                    if is_cur {
                                        sui.painter().rect_filled(tr, 4.0, style.selected_fill);
                                    } else if resp.hovered() || is_cursor {
                                        sui.painter().rect_filled(tr, 4.0, style.hover_fill);
                                    }
                                    if is_cur || is_cursor {
                                        let stroke = if is_cursor && focus {
                                            style.focus
                                        } else {
                                            Stroke::new(1.0, style.border_strong)
                                        };
                                        sui.painter().rect_stroke(
                                            tr,
                                            4.0,
                                            stroke,
                                            StrokeKind::Inside,
                                        );
                                    }
                                    if is_cursor && st.scroll_to_cursor {
                                        sui.scroll_to_rect(tr, None);
                                    }
                                    let label_h = font.size + 6.0;
                                    if let Some(paint) = &opt.thumb {
                                        let t = Rect::from_min_max(
                                            tr.min + Vec2::splat(3.0),
                                            Pos2::new(tr.right() - 3.0, tr.bottom() - label_h),
                                        );
                                        paint(sui, t);
                                        paint_text_centered(
                                            sui,
                                            Rect::from_min_max(
                                                Pos2::new(tr.left() + 2.0, tr.bottom() - label_h),
                                                Pos2::new(tr.right() - 2.0, tr.bottom()),
                                            ),
                                            &opt.text,
                                            &font,
                                            style.text,
                                        );
                                    } else {
                                        paint_text_centered(
                                            sui,
                                            tr.shrink2(Vec2::new(2.0, 0.0)),
                                            &opt.text,
                                            &font,
                                            style.text,
                                        );
                                    }
                                    resp.widget_info(|| {
                                        WidgetInfo::selected(
                                            WidgetType::SelectableLabel,
                                            true,
                                            is_cur,
                                            &opt.text,
                                        )
                                    });
                                    let resp = resp
                                        .on_hover_text(opt.tooltip.as_deref().unwrap_or(&opt.text));
                                    if resp.clicked() {
                                        chosen = Some(i);
                                        close = true;
                                    }
                                }
                            }
                        });
                    // Outline last, so rows never poke past the rounded corners.
                    pui.painter().rect_stroke(
                        box_rect,
                        r,
                        Stroke::new(1.0, style.border_strong),
                        StrokeKind::Inside,
                    );
                });
            let _ = hovered_any;
            st.scroll_to_cursor = false;
            if close {
                st.open = false;
            }
        }
        if chosen.is_some() {
            st.open = false;
        }
        let mut response = response;
        if chosen.is_some() {
            response.mark_changed();
        }
        let direction = popup_rect.map(|_| placement.direction);
        ui.data_mut(|d| d.insert_temp(id, st));
        DropdownResponse {
            response,
            chosen,
            popup_rect,
            direction,
        }
    }
}

struct Face<'a, 'b> {
    text: &'a str,
    italic: bool,
    font: &'a FontId,
    color: Color32,
    thumb: Option<&'b ThumbPaint<'b>>,
    thumb_side: f32,
    /// `Some(open)`: draw the arrow, pointing up when open.
    arrow: Option<bool>,
    pad: f32,
    arrow_w: f32,
}

/// Paints the control row: thumbnail, text, arrow.
fn paint_face(ui: &Ui, rect: Rect, face: &Face<'_, '_>) {
    let mut left = rect.left() + face.pad;
    if let Some(paint) = face.thumb {
        let t = Rect::from_min_size(
            Pos2::new(left, rect.center().y - face.thumb_side / 2.0),
            Vec2::splat(face.thumb_side),
        );
        paint(ui, t);
        left = t.right() + 6.0;
    }
    let avail = rect.right() - left - face.arrow_w;
    paint_text(
        ui,
        Pos2::new(left, rect.center().y),
        avail,
        face.text,
        face.italic,
        face.font,
        face.color,
    );
    if let Some(open) = face.arrow {
        let c = Pos2::new(rect.right() - face.arrow_w * 0.5, rect.center().y);
        let h = 3.5;
        let dir = if open { -1.0 } else { 1.0 };
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c + Vec2::new(-h, -h * 0.6 * dir),
                c + Vec2::new(h, -h * 0.6 * dir),
                c + Vec2::new(0.0, h * 0.7 * dir),
            ],
            face.color,
            Stroke::NONE,
        ));
    }
}

/// Left-aligned, vertically centred at `pos`, middle-shortened to `max_w`.
/// Returns the text actually painted.
fn paint_text(
    ui: &Ui,
    pos: Pos2,
    max_w: f32,
    text: &str,
    italic: bool,
    font: &FontId,
    color: Color32,
) -> String {
    let shown = truncate_middle_with(text, max_w.max(0.0), |s| {
        ui.painter()
            .layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x
    });
    let galley = layout(ui, &shown, italic, font, color);
    ui.painter().galley(
        Pos2::new(pos.x, pos.y - galley.size().y / 2.0),
        galley,
        color,
    );
    shown
}

fn paint_text_centered(ui: &Ui, rect: Rect, text: &str, font: &FontId, color: Color32) {
    let shown = truncate_middle_with(text, rect.width(), |s| {
        ui.painter()
            .layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x
    });
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        shown,
        font.clone(),
        color,
    );
}

fn layout(
    ui: &Ui,
    text: &str,
    italic: bool,
    font: &FontId,
    color: Color32,
) -> std::sync::Arc<egui::Galley> {
    if italic {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            text,
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color,
                italics: true,
                ..Default::default()
            },
        );
        ui.painter().layout_job(job)
    } else {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font.clone(), color)
    }
}

#[cfg(test)]
mod tests;

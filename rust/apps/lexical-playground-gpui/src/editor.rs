//! The editor: a GPUI element that draws a [`Surface`] (the document, its
//! selection and an input method's composition) and hands it the window's
//! text input (`EntityInputHandler`), keys (actions) and pointer.
//!
//! Lexical owns the document and the selection, as in a browser: every
//! edit runs in `lexical-editor`, and the element is drawn again from its
//! state, as Lexical's reconciler draws the DOM. Moving the caret and
//! laying out text are the element's, as they are the browser's.

use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
use lexical_playground_core::{Outcome, Playground, Surface, Unit, format_flag};
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

actions!(
    lexical_editor,
    [
        Backspace,
        Delete,
        DeleteWordBackward,
        DeleteWordForward,
        DeleteLineBackward,
        DeleteLineForward,
        InsertParagraph,
        InsertLineBreak,
        InsertTab,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        LineStart,
        LineEnd,
        SelectLineStart,
        SelectLineEnd,
        SelectAll,
        Bold,
        Italic,
        Underline,
        Undo,
        Redo,
        Copy,
        Cut,
        Paste,
        ShowCharacterPalette,
    ]
);

const CONTEXT: &str = "LexicalEditor";

/// How long the caret stays on, then off, as a Mac text field's does.
const BLINK_PHASE: Duration = Duration::from_millis(500);

/// The editor's keys, as a browser's `contenteditable` takes them on macOS.
pub fn bind_keys(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("shift-backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("ctrl-d", Delete, c),
        KeyBinding::new("alt-backspace", DeleteWordBackward, c),
        KeyBinding::new("alt-delete", DeleteWordForward, c),
        KeyBinding::new("cmd-backspace", DeleteLineBackward, c),
        KeyBinding::new("cmd-delete", DeleteLineForward, c),
        KeyBinding::new("enter", InsertParagraph, c),
        KeyBinding::new("shift-enter", InsertLineBreak, c),
        KeyBinding::new("tab", InsertTab, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("cmd-left", LineStart, c),
        KeyBinding::new("home", LineStart, c),
        KeyBinding::new("cmd-right", LineEnd, c),
        KeyBinding::new("end", LineEnd, c),
        KeyBinding::new("cmd-shift-left", SelectLineStart, c),
        KeyBinding::new("cmd-shift-right", SelectLineEnd, c),
        KeyBinding::new("cmd-a", SelectAll, c),
        KeyBinding::new("cmd-b", Bold, c),
        KeyBinding::new("cmd-i", Italic, c),
        KeyBinding::new("cmd-u", Underline, c),
        KeyBinding::new("cmd-z", Undo, c),
        KeyBinding::new("cmd-shift-z", Redo, c),
        KeyBinding::new("cmd-c", Copy, c),
        KeyBinding::new("cmd-x", Cut, c),
        KeyBinding::new("cmd-v", Paste, c),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, c),
    ]);
}

/// Where the last paint put each line of the display text.
struct Layout {
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    text: String,
    /// Each `\n`-separated line: its first byte, its top, and its shape.
    lines: Rc<Vec<(usize, Pixels, WrappedLine)>>,
}

impl Layout {
    /// The line holding byte `index`.
    fn line_of(&self, index: usize) -> Option<&(usize, Pixels, WrappedLine)> {
        self.lines
            .iter()
            .rev()
            .find(|(start, _, _)| *start <= index)
    }

    /// The byte under `position` (window coordinates).
    fn index_at(&self, position: Point<Pixels>) -> usize {
        let local = position - self.bounds.origin;
        if local.y < px(0.) {
            return 0;
        }
        let line = self
            .lines
            .iter()
            .rev()
            .find(|(_, top, _)| *top <= local.y)
            .or(self.lines.first());
        let Some((start, top, line)) = line else {
            return 0;
        };
        let at = point(local.x.max(px(0.)), local.y - *top);
        match line.closest_index_for_position(at, self.line_height) {
            Ok(i) | Err(i) => start + i,
        }
    }

    /// The top left of the caret at byte `index` (window coordinates).
    fn position_of(&self, index: usize) -> Option<Point<Pixels>> {
        let (start, top, line) = self.line_of(index)?;
        let at = line.position_for_index(index - start, self.line_height)?;
        Some(self.bounds.origin + point(at.x, *top + at.y))
    }

    /// Each row of each line as `(byte range, top left, width)`.
    fn rows(&self) -> Vec<(Range<usize>, Point<Pixels>, Pixels)> {
        let mut rows = Vec::new();
        for (start, top, line) in self.lines.iter() {
            let layout = &line.unwrapped_layout;
            let mut breaks: Vec<usize> = line
                .wrap_boundaries
                .iter()
                .map(|b| layout.runs[b.run_ix].glyphs[b.glyph_ix].index)
                .collect();
            breaks.insert(0, 0);
            breaks.push(line.len());
            for (row, pair) in breaks.windows(2).enumerate() {
                let (from, to) = (pair[0], pair[1]);
                let x0 = layout.x_for_index(from);
                let width = layout.x_for_index(to) - x0;
                let y = *top + self.line_height * row as f32;
                rows.push((
                    start + from..start + to,
                    self.bounds.origin + point(px(0.), y),
                    width,
                ));
            }
        }
        rows
    }
}

pub struct LexicalEditor {
    pub surface: Surface,
    focus_handle: FocusHandle,
    layout: Option<Layout>,
    selecting: bool,
    /// The column a vertical move keeps.
    goal_x: Option<Pixels>,
    /// The last edit's error.
    pub error: Option<String>,
    /// Whether the caret is in its on phase (see [`LexicalEditor::blink`]).
    caret_on: bool,
    /// Bumped by every restart, so an older blink loop stops.
    blink_epoch: usize,
    _blink: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl LexicalEditor {
    pub fn new(playground: Playground, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let this = cx.entity().downgrade();
        let _subscriptions = vec![
            // Every keystroke shows the caret, before its action runs, as
            // gpui-component's input does: a key binding can consume it.
            cx.intercept_keystrokes({
                let focus_handle = focus_handle.clone();
                move |_, window, cx| {
                    if focus_handle.is_focused(window) {
                        _ = this.update(cx, |editor, cx| editor.blink(cx));
                    }
                }
            }),
            cx.observe_window_activation(window, |editor, window, cx| {
                if window.is_window_active() && editor.focus_handle.is_focused(window) {
                    editor.blink(cx);
                } else {
                    editor.stop_blink(cx);
                }
            }),
            cx.on_focus(&focus_handle, window, |editor, _, cx| editor.blink(cx)),
            cx.on_blur(&focus_handle, window, |editor, _, cx| editor.stop_blink(cx)),
        ];
        LexicalEditor {
            surface: Surface::new(playground),
            focus_handle,
            layout: None,
            selecting: false,
            goal_x: None,
            error: None,
            caret_on: true,
            blink_epoch: 0,
            _blink: Task::ready(()),
            _subscriptions,
        }
    }

    /// Shows the caret and blinks it from now, as a Mac text field does
    /// after a key, an edit or a move: on for a phase, then off and on
    /// every phase, until the editor or the window loses the keyboard.
    fn blink(&mut self, cx: &mut Context<Self>) {
        self.blink_epoch = self.blink_epoch.wrapping_add(1);
        let epoch = self.blink_epoch;
        self.caret_on = true;
        cx.notify();
        self._blink = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(BLINK_PHASE).await;
                let blinking = this.update(cx, |editor, cx| {
                    if editor.blink_epoch != epoch {
                        return false;
                    }
                    editor.caret_on = !editor.caret_on;
                    cx.notify();
                    true
                });
                if !matches!(blinking, Ok(true)) {
                    break;
                }
            }
        });
    }

    /// Stops blinking; the caret isn't drawn while the editor or the
    /// window doesn't have the keyboard.
    fn stop_blink(&mut self, cx: &mut Context<Self>) {
        self.blink_epoch = self.blink_epoch.wrapping_add(1);
        self._blink = Task::ready(());
        self.caret_on = false;
        cx.notify();
    }

    /// Whether the element has painted, so keys can move the caret.
    #[cfg(feature = "e2e")]
    pub fn is_laid_out(&self) -> bool {
        self.layout.is_some()
    }

    /// Runs an edit at the selection, then draws again.
    pub fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Playground) -> Outcome) {
        let outcome = self.surface.edit(f);
        self.done(outcome, cx);
    }

    fn done(&mut self, outcome: Outcome, cx: &mut Context<Self>) {
        self.error = outcome.error;
        self.goal_x = None;
        self.blink(cx);
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    fn delete(&mut self, unit: Unit, backward: bool, cx: &mut Context<Self>) {
        let outcome = self.surface.delete(unit, backward);
        self.done(outcome, cx);
    }

    fn move_to(&mut self, offset: usize, extend: bool, cx: &mut Context<Self>) {
        self.surface.move_to(offset, extend);
        self.blink(cx);
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    fn step(&self, backward: bool) -> usize {
        let text = self.surface.display_text();
        let caret = self.surface.caret();
        if backward {
            text[..caret]
                .grapheme_indices(true)
                .next_back()
                .map_or(0, |(i, _)| i)
        } else {
            text[caret..]
                .graphemes(true)
                .next()
                .map_or(caret, |g| caret + g.len())
        }
    }

    fn horizontal(&mut self, backward: bool, extend: bool, cx: &mut Context<Self>) {
        let (range, _) = self.surface.selection();
        let to = if !extend && !range.is_empty() {
            if backward { range.start } else { range.end }
        } else {
            self.step(backward)
        };
        self.goal_x = None;
        self.move_to(to, extend, cx);
    }

    fn vertical(&mut self, up: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(layout) = &self.layout else { return };
        let caret = self.surface.caret();
        let Some(at) = layout.position_of(caret) else {
            return;
        };
        let x = *self.goal_x.get_or_insert(at.x);
        let y = if up {
            at.y - layout.line_height * 0.5
        } else {
            at.y + layout.line_height * 1.5
        };
        let to = if y < layout.bounds.top() {
            0
        } else {
            layout.index_at(point(x, y))
        };
        let goal = self.goal_x;
        self.move_to(to, extend, cx);
        self.goal_x = goal;
    }

    fn line_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let caret = self.surface.caret();
        let to = self.layout.as_ref().and_then(|l| {
            l.rows()
                .into_iter()
                .find(|(r, _, _)| r.start <= caret && caret <= r.end)
                .map(|(r, _, _)| if end { r.end } else { r.start })
        });
        let text = self.surface.display_text();
        let to = to.unwrap_or(if end { text.len() } else { 0 });
        self.move_to(to, extend, cx);
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        let Some(layout) = &self.layout else { return };
        let at = layout.index_at(event.position);
        self.selecting = true;
        if event.click_count >= 2 {
            // A word, as a double click selects it.
            let text = &layout.text;
            let (start, end) = text
                .split_word_bound_indices()
                .find(|(i, w)| *i <= at && at <= i + w.len())
                .map_or((at, at), |(i, w)| (i, i + w.len()));
            self.move_to(start, false, cx);
            self.move_to(end, true, cx);
            return;
        }
        self.move_to(at, event.modifiers.shift, cx);
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting
            && let Some(layout) = &self.layout
        {
            let at = layout.index_at(event.position);
            self.move_to(at, true, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.edit(cx, |p| p.insert_raw_text(&text));
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.surface.selected_text();
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        self.copy(&Copy, window, cx);
        self.edit(cx, Playground::remove_text);
    }
}

pub enum EditorEvent {
    Changed,
}

impl EventEmitter<EditorEvent> for LexicalEditor {}

impl Focusable for LexicalEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for LexicalEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let (text, actual) = self.surface.text_for_range_utf16(range);
        actual_range.replace(actual);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let (range, reversed) = self.surface.selected_range_utf16();
        Some(UTF16Selection { range, reversed })
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.surface.set_selected_range_utf16(range);
        cx.notify();
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.surface.text_length_utf16())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.surface.marked_range_utf16()
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(outcome) = self.surface.unmark() {
            self.done(outcome, cx);
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let outcome = self.surface.replace_text_utf16(range, text);
        self.done(outcome, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.surface.replace_and_mark_utf16(range, text, selected);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let text = &layout.text;
        let start = lexical_playground_core::byte_of_utf16(text, range.start);
        let at = layout.position_of(start)?;
        Some(Bounds::new(at, size(px(2.), layout.line_height)))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let index = layout.index_at(position);
        Some(lexical_playground_core::utf16_of_byte(&layout.text, index))
    }
}

impl Render for LexicalEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("lexical-editor")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .size_full()
            .overflow_y_scroll()
            .on_action(
                cx.listener(|this, _: &Backspace, _, cx| this.delete(Unit::Character, true, cx)),
            )
            .on_action(
                cx.listener(|this, _: &Delete, _, cx| this.delete(Unit::Character, false, cx)),
            )
            .on_action(
                cx.listener(|this, _: &DeleteWordBackward, _, cx| {
                    this.delete(Unit::Word, true, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &DeleteWordForward, _, cx| {
                    this.delete(Unit::Word, false, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &DeleteLineBackward, _, cx| {
                    this.delete(Unit::Line, true, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &DeleteLineForward, _, cx| {
                    this.delete(Unit::Line, false, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &InsertParagraph, _, cx| {
                this.edit(cx, Playground::insert_paragraph)
            }))
            .on_action(cx.listener(|this, _: &InsertLineBreak, _, cx| {
                this.edit(cx, Playground::insert_line_break)
            }))
            .on_action(
                cx.listener(|this, _: &InsertTab, _, cx| {
                    this.edit(cx, |p| p.insert_raw_text("\t"))
                }),
            )
            .on_action(cx.listener(|this, _: &Left, _, cx| this.horizontal(true, false, cx)))
            .on_action(cx.listener(|this, _: &Right, _, cx| this.horizontal(false, false, cx)))
            .on_action(cx.listener(|this, _: &SelectLeft, _, cx| this.horizontal(true, true, cx)))
            .on_action(cx.listener(|this, _: &SelectRight, _, cx| this.horizontal(false, true, cx)))
            .on_action(cx.listener(|this, _: &Up, _, cx| this.vertical(true, false, cx)))
            .on_action(cx.listener(|this, _: &Down, _, cx| this.vertical(false, false, cx)))
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.vertical(true, true, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.vertical(false, true, cx)))
            .on_action(cx.listener(|this, _: &LineStart, _, cx| this.line_edge(false, false, cx)))
            .on_action(cx.listener(|this, _: &LineEnd, _, cx| this.line_edge(true, false, cx)))
            .on_action(
                cx.listener(|this, _: &SelectLineStart, _, cx| this.line_edge(false, true, cx)),
            )
            .on_action(cx.listener(|this, _: &SelectLineEnd, _, cx| this.line_edge(true, true, cx)))
            .on_action(
                cx.listener(|this, _: &SelectAll, _, cx| this.edit(cx, Playground::select_all)),
            )
            .on_action(
                cx.listener(|this, _: &Bold, _, cx| this.edit(cx, |p| p.format_text("bold"))),
            )
            .on_action(
                cx.listener(|this, _: &Italic, _, cx| this.edit(cx, |p| p.format_text("italic"))),
            )
            .on_action(cx.listener(|this, _: &Underline, _, cx| {
                this.edit(cx, |p| p.format_text("underline"))
            }))
            .on_action(cx.listener(|this, _: &Undo, _, cx| this.edit(cx, Playground::undo)))
            .on_action(cx.listener(|this, _: &Redo, _, cx| this.edit(cx, Playground::redo)))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(
                cx.listener(|_, _: &ShowCharacterPalette, window, _| {
                    window.show_character_palette()
                }),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(EditorElement {
                editor: cx.entity(),
            })
    }
}

/// The text runs to shape: each run's format as a font, colors and lines.
fn text_runs(surface: &Surface, base: &TextStyle, accent: Hsla) -> Vec<TextRun> {
    let base_font = base.font();
    surface
        .runs()
        .into_iter()
        .map(|run| {
            let has = |name| run.format & format_flag(name) != 0;
            let mut font = if has("code") {
                font("Menlo")
            } else {
                base_font.clone()
            };
            if has("bold") {
                font.weight = FontWeight::BOLD;
            }
            if has("italic") {
                font.style = FontStyle::Italic;
            }
            let line = |on: bool| on.then_some(px(1.));
            TextRun {
                len: run.range.len(),
                font,
                color: if has("subscript") || has("superscript") {
                    accent
                } else {
                    base.color
                },
                background_color: if has("highlight") {
                    Some(hsla(48. / 360., 0.96, 0.75, 1.))
                } else if has("code") {
                    Some(base.color.opacity(0.08))
                } else {
                    None
                },
                underline: (has("underline") || run.composing).then(|| UnderlineStyle {
                    thickness: px(1.),
                    color: Some(if run.composing { accent } else { base.color }),
                    wavy: false,
                }),
                strikethrough: line(has("strikethrough")).map(|thickness| StrikethroughStyle {
                    thickness,
                    color: Some(base.color),
                }),
            }
        })
        .collect()
}

struct EditorElement {
    editor: Entity<LexicalEditor>,
}

struct Prepaint {
    lines: Rc<Vec<(usize, Pixels, WrappedLine)>>,
    selection: Vec<PaintQuad>,
    caret: Option<PaintQuad>,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Shapes `text` at `width`: each line's first byte, top and shape.
fn shape(
    window: &mut Window,
    text: &str,
    runs: &[TextRun],
    font_size: Pixels,
    line_height: Pixels,
    width: Option<Pixels>,
) -> Vec<(usize, Pixels, WrappedLine)> {
    let shaped = window
        .text_system()
        .shape_text(
            SharedString::from(text.to_owned()),
            font_size,
            runs,
            width,
            None,
        )
        .unwrap_or_default();
    let mut out = Vec::new();
    let (mut start, mut top) = (0, px(0.));
    for line in shaped {
        let rows = line.wrap_boundaries.len() + 1;
        let len = line.len();
        out.push((start, top, line));
        start += len + 1;
        top += line_height * rows as f32;
    }
    out
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let editor = self.editor.read(cx);
        let text = editor.surface.display_text();
        let style = window.text_style();
        let runs = text_runs(&editor.surface, &style, cx.theme().primary);
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let mut layout_style = Style::default();
        layout_style.size.width = relative(1.).into();
        let id =
            window.request_measured_layout(layout_style, move |known, available, window, _| {
                let width = known.width.or(match available.width {
                    AvailableSpace::Definite(w) => Some(w),
                    _ => None,
                });
                let lines = shape(window, &text, &runs, font_size, line_height, width);
                let height = lines.last().map_or(line_height, |(_, top, line)| {
                    *top + line_height * (line.wrap_boundaries.len() + 1) as f32
                });
                size(width.unwrap_or(px(0.)), height)
            });
        (id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let editor = self.editor.read(cx);
        let text = editor.surface.display_text();
        let style = window.text_style();
        let accent = cx.theme().primary;
        let runs = text_runs(&editor.surface, &style, accent);
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let lines = Rc::new(shape(
            window,
            &text,
            &runs,
            font_size,
            line_height,
            Some(bounds.size.width),
        ));
        let layout = Layout {
            bounds,
            line_height,
            text: text.clone(),
            lines: lines.clone(),
        };
        let (range, _) = editor.surface.selection();
        let mut selection = Vec::new();
        if !range.is_empty() {
            for (row, origin, width) in layout.rows() {
                if range.end < row.start || range.start > row.end {
                    continue;
                }
                let layout_x = |i: usize| layout.position_of(i).map_or(origin.x, |p| p.x);
                let x0 = if range.start <= row.start {
                    origin.x
                } else {
                    layout_x(range.start)
                };
                // A selection that goes on past the row covers its end, and
                // past a line's end, the newline.
                let x1 = if range.end > row.end {
                    origin.x + width + px(6.)
                } else {
                    layout_x(range.end)
                };
                if x1 > x0 {
                    selection.push(fill(
                        Bounds::from_corners(
                            point(x0, origin.y),
                            point(x1, origin.y + line_height),
                        ),
                        cx.theme().selection,
                    ));
                }
            }
        }
        let caret = range
            .is_empty()
            .then(|| layout.position_of(editor.surface.caret()))
            .flatten()
            .map(|at| fill(Bounds::new(at, size(px(2.), line_height)), cx.theme().caret));
        self.editor
            .update(cx, |editor, _| editor.layout = Some(layout));
        Prepaint {
            lines,
            selection,
            caret,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus_handle, caret_on) = {
            let editor = self.editor.read(cx);
            (editor.focus_handle.clone(), editor.caret_on)
        };
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        for quad in prepaint.selection.drain(..) {
            window.paint_quad(quad);
        }
        let line_height = window.line_height();
        for (_, top, line) in prepaint.lines.iter() {
            let origin = bounds.origin + point(px(0.), *top);
            line.paint_background(
                origin,
                line_height,
                TextAlign::Left,
                Some(bounds),
                window,
                cx,
            )
            .ok();
            line.paint(
                origin,
                line_height,
                TextAlign::Left,
                Some(bounds),
                window,
                cx,
            )
            .ok();
        }
        if focus_handle.is_focused(window)
            && window.is_window_active()
            && caret_on
            && let Some(caret) = prepaint.caret.take()
        {
            window.paint_quad(caret);
        }
    }
}

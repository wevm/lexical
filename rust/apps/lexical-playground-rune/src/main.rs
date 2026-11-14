//! The Lexical playground on Rune: a document in `lexical-editor`, drawn
//! and edited in a native window.
//!
//! Lexical owns the document and the selection. The editor is Rune's
//! `text_input` over a mirror of the document (`mirror`): the field lays the
//! text out, hit-tests the pointer, moves the caret and composes input
//! methods, but every change goes through Lexical. Keys that edit (typing,
//! Return, Backspace, ⌘B) are taken before the field sees them and run as
//! Lexical updates, and anything the field still changes on its own (an
//! input method's commit, a cut) is replayed into Lexical as the text it
//! replaced. After each edit the mirror is rebuilt from Lexical's state, as
//! Lexical's reconciler rebuilds the DOM.

mod view;

use lexical_playground_core::{Document, Outcome, Playground, Pos, RunKind, Sel, example};
use rune::{AppSpec, Block, ClipboardContent, Key, KeyEvent, Mark, MarkRange, TextEdit, Tx};

/// The side panel's tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Tree,
    Json,
    Parity,
}

pub struct State {
    pub playground: Playground,
    /// The field's model: the document's text, formats and line breaks.
    pub edit: TextEdit,
    pub panel: Panel,
    /// The conformance report, after Replay vectors.
    pub report: Option<(String, bool)>,
    /// What the last action said (an export's path, an edit's error).
    pub status: Option<String>,
    /// The caret's blink: see [`State::caret_visible`].
    pub blink: Blink,
}

/// How long the caret stays on, then off, as a Mac text field's does.
const BLINK_PHASE: f64 = 0.5;

/// The caret blinks as a Mac text field's: solid for a phase after any
/// key, edit or move, then off and on every phase, and hidden while the
/// window or the field doesn't have the keyboard. Rune's field draws its
/// caret solid, so the view paints it clear in the off phases.
pub struct Blink {
    /// The window's clock (seconds) at the last key, edit or move.
    since: f64,
    /// The field's selection then, to see the pointer move the caret.
    selection: (usize, usize),
    window_focused: bool,
    pub on: bool,
}

impl Blink {
    fn restart(&mut self, clock: f64) {
        self.since = clock;
        self.on = true;
    }
}

impl State {
    fn new() -> State {
        let playground = Playground::new(example()).expect("the example document loads");
        let mut state = State {
            playground,
            edit: TextEdit::new(""),
            panel: Panel::Tree,
            report: None,
            status: None,
            blink: Blink {
                since: 0.0,
                selection: (0, 0),
                window_focused: true,
                on: true,
            },
        };
        state.edit.focused = true;
        state.sync();
        state
    }

    /// Rebuilds the field's model from the editor (keeping its focus).
    pub fn sync(&mut self) {
        let focused = self.edit.focused;
        self.edit = mirror(self.playground.document(), self.playground.selection());
        self.edit.focused = focused;
    }

    /// The field's selection, which the toolbar shows: a click or an
    /// arrow key moves it before Lexical hears of it at the next edit.
    pub fn shown_selection(&self) -> Sel {
        let doc = self.playground.document();
        Sel {
            anchor: doc.pos_at(self.edit.anchor),
            focus: doc.pos_at(self.edit.focus),
        }
    }

    /// Hands the field's selection to the editor, before an edit.
    fn take_selection(&mut self) {
        let sel = self.shown_selection();
        self.playground.set_selection(Some(sel));
    }

    /// Runs an edit at the field's selection, then redraws.
    pub fn edit_with(&mut self, f: impl FnOnce(&mut Playground) -> Outcome) {
        self.take_selection();
        let outcome = f(&mut self.playground);
        self.status = outcome.error;
        self.sync();
    }

    /// A toolbar button's edit: as [`State::edit_with`], then the keyboard
    /// goes back to the editor, as a toolbar button in a web editor keeps
    /// the selection.
    pub fn toolbar_edit(&mut self, f: impl FnOnce(&mut Playground) -> Outcome) {
        self.edit_with(f);
        self.edit.focused = true;
    }

    /// A key, before the field: true when it was an edit Lexical made.
    fn key(&mut self, ev: &KeyEvent) -> bool {
        let m = ev.mods;
        match &ev.key {
            Key::Text(s) if !m.cmd => {
                let s = s.clone();
                self.edit_with(|p| p.insert_text(&s));
            }
            Key::Enter if m.shift => self.edit_with(Playground::insert_line_break),
            Key::Enter => self.edit_with(Playground::insert_paragraph),
            Key::Backspace | Key::Delete => {
                let backward = ev.key == Key::Backspace;
                if m.cmd {
                    self.edit_with(|p| p.delete_line(backward));
                } else if m.alt {
                    self.edit_with(|p| p.delete_word(backward));
                } else {
                    self.edit_with(|p| p.delete_character(backward));
                }
            }
            Key::Tab => self.edit_with(|p| p.tab(m.shift)),
            Key::Command(c) => match c {
                'b' => self.edit_with(|p| p.format_text("bold")),
                'i' => self.edit_with(|p| p.format_text("italic")),
                'u' => self.edit_with(|p| p.format_text("underline")),
                'a' => self.edit_with(Playground::select_all),
                // Lexical's history, not the field's own undo, which would
                // edit behind Lexical's back.
                'z' | 'Z' if m.shift => self.edit_with(Playground::redo),
                'z' => self.edit_with(Playground::undo),
                'y' => {}
                _ => return false,
            },
            _ => return false,
        }
        true
    }

    /// Pasted text: `insertRawText`, as Lexical pastes plain text.
    fn paste(&mut self, content: &ClipboardContent) -> bool {
        let text = content.text().to_owned();
        self.edit_with(|p| p.insert_raw_text(&text));
        true
    }

    /// The field changed its text itself: replay that change in Lexical.
    fn changed(&mut self) {
        let text = self.edit.text.clone();
        let outcome = self.playground.apply_text(&text);
        if let Some(outcome) = outcome {
            self.status = outcome.error;
        }
        self.sync();
    }
}

/// The field's model for `doc`: its text, a mark per format the field
/// draws, its links, a Rune block per line, and its line breaks as soft
/// breaks.
pub fn mirror(doc: &Document, selection: Option<Sel>) -> TextEdit {
    let mut marks = Vec::new();
    let mut blocks = Vec::new();
    let mut offset = 0;
    for block in &doc.blocks {
        let checked = block
            .list
            .as_ref()
            .is_some_and(|l| l.list_type == "check" && l.checked);
        for run in &block.runs {
            if run.kind == RunKind::Text {
                let range = offset + run.start..offset + run.end();
                if range.is_empty() {
                    continue;
                }
                for (name, mark) in [
                    ("bold", Mark::Bold),
                    ("italic", Mark::Italic),
                    ("underline", Mark::Underline),
                    ("strikethrough", Mark::Strike),
                    ("code", Mark::Code),
                ] {
                    // A checked item is struck through, as the JS
                    // playground's `.PlaygroundEditorTheme__listItemChecked`.
                    if run.has_format(name) || (checked && mark == Mark::Strike) {
                        marks.push(MarkRange::new(range.clone(), mark));
                    }
                }
                if let Some(url) = &run.link {
                    marks.push(MarkRange::new(range, Mark::Link(url.clone())));
                }
            }
        }
        let lines = 1 + block
            .runs
            .iter()
            .filter(|r| r.kind == RunKind::LineBreak)
            .count();
        blocks.extend(std::iter::repeat_n(rune_block(block), lines));
        offset += block.text.len() + 1;
    }
    let mut edit =
        TextEdit::from_parts(doc.text(), marks, blocks).with_soft_breaks(doc.soft_breaks());
    let sel = selection.unwrap_or(Sel::caret(Pos::default()));
    edit.set_selection(doc.offset_of(sel.anchor), doc.offset_of(sel.focus));
    edit
}

/// How the field draws `block`: a list item as a bullet or a number at its
/// list's depth (a check item as a bullet; its box is the toolbar's Check
/// button), a quote with the field's quote bar. Headings are paragraphs
/// with larger text (see [`decorations`]).
fn rune_block(block: &lexical_playground_core::Block) -> Block {
    if let Some(list) = &block.list {
        let level = list.depth.min(usize::from(rune::MAX_LIST_LEVEL)) as u8;
        return match list.list_type.as_str() {
            "number" => Block::Numbered(level),
            _ => Block::Bullet(level),
        };
    }
    match block.kind.as_str() {
        "quote" => Block::Quote,
        "code" => Block::Code,
        _ => Block::Paragraph,
    }
}

/// A heading's text size and weight, as the JS playground's theme
/// (`h1` 24px ... `h6`, in the field's 16-point body).
fn heading_style(tag: &str) -> Option<(f32, f32)> {
    Some(match tag {
        "h1" => (26.0, 700.0),
        "h2" => (21.0, 700.0),
        "h3" => (18.0, 650.0),
        "h4" => (16.0, 650.0),
        "h5" => (14.0, 650.0),
        "h6" => (13.0, 650.0),
        _ => return None,
    })
}

/// Headings, highlight, subscript and superscript, which the field has no
/// marks or blocks for, drawn over it. `dark` is the window's appearance: highlight is a marker
/// yellow behind dark text and a deep amber behind light text.
pub fn decorations(doc: &Document, dark: bool) -> Vec<rune::Span> {
    let mut spans = Vec::new();
    let mut offset = 0;
    for block in &doc.blocks {
        if let Some((size, weight)) = block.tag.as_deref().and_then(heading_style)
            && !block.text.is_empty()
        {
            let mut span = rune::Span::new(offset, offset + block.text.len()).weight(weight);
            span.size = Some(size);
            spans.push(span);
        }
        for run in &block.runs {
            let (start, end) = (offset + run.start, offset + run.end());
            if run.kind != RunKind::Text || start == end {
                continue;
            }
            if run.has_format("highlight") {
                let mut span = rune::Span::new(start, end);
                span.bg = Some(if dark {
                    rune::oklch(0.5, 0.1, 75.0)
                } else {
                    rune::oklch(0.92, 0.12, 95.0)
                });
                spans.push(span);
            }
            if run.has_format("subscript") || run.has_format("superscript") {
                let mut span = rune::Span::new(start, end);
                span.size = Some(11.0);
                spans.push(span);
            }
        }
        offset += block.text.len() + 1;
    }
    spans
}

pub fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    s.blink.restart(tx.clock());
    s.key(ev)
}

pub fn on_paste(s: &mut State, content: &ClipboardContent, tx: &mut Tx) -> bool {
    s.blink.restart(tx.clock());
    s.paste(content)
}

pub fn on_change(s: &mut State, tx: &mut Tx) {
    s.blink.restart(tx.clock());
    s.changed();
}

pub fn on_window_focus(s: &mut State, focused: bool, tx: &mut Tx) {
    s.blink.window_focused = focused;
    s.blink.restart(tx.clock());
}

/// Every frame: the caret's blink phase, and a wake for the next one.
fn poll(s: &mut State, clock: f64, tx: &mut Tx) -> bool {
    let b = &mut s.blink;
    let selection = (s.edit.anchor, s.edit.focus);
    if selection != b.selection {
        b.selection = selection;
        b.since = clock;
    }
    let was = b.on;
    if b.window_focused && s.edit.focused {
        let phase = ((clock - b.since).max(0.0) / BLINK_PHASE).floor();
        b.on = (phase as u64).is_multiple_of(2);
        tx.wake_at(b.since + (phase + 1.0) * BLINK_PHASE);
    } else {
        b.on = false;
    }
    b.on != was
}

/// A session headless snapshots show: typed text, a new paragraph, bold.
fn typed(s: &mut State) {
    let end = s.playground.document().end();
    s.playground.set_selection(Some(Sel::caret(end)));
    s.playground.insert_paragraph();
    s.playground.format_text("bold");
    s.playground.insert_text("Typed in Rust");
    s.playground.format_text("bold");
    s.playground.insert_text(", then plain.");
    s.sync();
}

fn main() {
    rune::run(AppSpec::<State> {
        scenes: &[
            ("typed", typed),
            ("json", |s| {
                typed(s);
                s.panel = Panel::Json;
            }),
            ("parity", |s| {
                s.panel = Panel::Parity;
                s.report = Some(view::replay());
            }),
        ],
        // Inline code (`Mark::Code`) draws in the mono slot: the system's
        // monospaced face (SF Mono on macOS).
        families: ("Inter", "ui-monospace"),
        poll: Some(poll),
        ..AppSpec::new(
            "Lexical Playground (Rune)",
            (1100.0, 720.0),
            State::new,
            view::view,
        )
    });
}

#[cfg(test)]
mod tests;

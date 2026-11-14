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

    /// Hands the field's selection to the editor, before an edit.
    fn take_selection(&mut self) {
        let doc = self.playground.document();
        let sel = Sel {
            anchor: doc.pos_at(self.edit.anchor),
            focus: doc.pos_at(self.edit.focus),
        };
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
            Key::Tab if !m.shift => self.edit_with(|p| p.insert_raw_text("\t")),
            Key::Command(c) => match c {
                'b' => self.edit_with(|p| p.format_text("bold")),
                'i' => self.edit_with(|p| p.format_text("italic")),
                'u' => self.edit_with(|p| p.format_text("underline")),
                'a' => self.edit_with(Playground::select_all),
                // Undo comes with @lexical/history; until then the field's
                // own undo would edit behind Lexical's back.
                'z' | 'y' => {}
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
/// draws, and its line breaks as soft breaks.
pub fn mirror(doc: &Document, selection: Option<Sel>) -> TextEdit {
    let mut marks = Vec::new();
    let mut offset = 0;
    for block in &doc.blocks {
        for run in &block.runs {
            if run.kind == RunKind::Text {
                let range = offset + run.start..offset + run.end();
                for (name, mark) in [
                    ("bold", Mark::Bold),
                    ("italic", Mark::Italic),
                    ("underline", Mark::Underline),
                    ("strikethrough", Mark::Strike),
                    ("code", Mark::Code),
                ] {
                    if run.has_format(name) && !range.is_empty() {
                        marks.push(MarkRange::new(range.clone(), mark));
                    }
                }
            }
        }
        offset += block.text.len() + 1;
    }
    let text = doc.text();
    let lines = text.matches('\n').count() + 1;
    let mut edit = TextEdit::from_parts(text, marks, vec![Block::Paragraph; lines])
        .with_soft_breaks(doc.soft_breaks());
    let sel = selection.unwrap_or(Sel::caret(Pos::default()));
    edit.set_selection(doc.offset_of(sel.anchor), doc.offset_of(sel.focus));
    edit
}

/// Highlight, subscript and superscript, which the field has no marks for,
/// drawn over it.
pub fn decorations(doc: &Document) -> Vec<rune::Span> {
    let mut spans = Vec::new();
    let mut offset = 0;
    for block in &doc.blocks {
        for run in &block.runs {
            let (start, end) = (offset + run.start, offset + run.end());
            if run.kind != RunKind::Text || start == end {
                continue;
            }
            if run.has_format("highlight") {
                let mut span = rune::Span::new(start, end);
                span.bg = Some(rune::oklch(0.92, 0.12, 95.0));
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

pub fn on_key(s: &mut State, ev: &KeyEvent, _: &mut Tx) -> bool {
    s.key(ev)
}

pub fn on_paste(s: &mut State, content: &ClipboardContent, _: &mut Tx) -> bool {
    s.paste(content)
}

pub fn on_change(s: &mut State, _: &mut Tx) {
    s.changed();
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
        ],
        // Inline code (`Mark::Code`) draws in the mono slot: the system's
        // monospaced face (SF Mono on macOS).
        families: ("Inter", "ui-monospace"),
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

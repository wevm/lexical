//! The editor binding the desktop playgrounds share
//! (`lexical-playground-rune`, `lexical-playground-gpui`).
//!
//! Lexical owns the document and the selection; the app draws them, as
//! Lexical's reconciler draws the DOM. This crate gives the app:
//!
//! - [`Document`]: the editor state as blocks of styled runs, each block's
//!   text in one string, so a UI framework can lay it out and hit-test it in
//!   byte offsets.
//! - [`Pos`] and [`Document::point_at`] / [`Document::pos_of`]: between a
//!   position in that text and a Lexical selection point (UTF-16 offsets).
//! - [`Playground`]: edits (typing, Enter, Backspace, formatting, undo) run as
//!   steps of the conformance script language (`ops.ts`), through the same
//!   interpreter that replays the vectors. Every step and its result are
//!   kept, so [`Playground::export_case`] writes the session as a
//!   `scripts/*` vector case that JS Lexical replays
//!   (`packages/lexical-conformance/src/replay.test.ts`): parity for a
//!   session typed by hand.
//! - [`Surface`]: the text input client for frameworks that hand the app
//!   raw text input and input-method compositions (GPUI), over a
//!   [`Playground`].
//! - [`replay_vectors`]: the conformance report, run in the app.

use lexical::json::{self, Object, Value};
use lexical::{NodeRef, get_root};
use lexical_conformance::scripts::Session;
use std::rc::Rc;
use std::time::Instant;

mod surface;
pub use surface::{DisplayRun, Surface, byte_of_utf16, utf16_of_byte};

/// A run of a block: a text node, a tab, or a line break.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub key: String,
    pub kind: RunKind,
    /// The run's text: the node's text, `"\t"` for a tab, `"\n"` for a
    /// line break.
    pub text: String,
    /// The text format bits (`IS_BOLD`, ...); 0 for a line break.
    pub format: u32,
    /// The node's inline CSS (`style`).
    pub style: String,
    /// The run's start in its block's text, in bytes.
    pub start: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Text,
    Tab,
    LineBreak,
}

impl Run {
    pub fn end(&self) -> usize {
        self.start + self.text.len()
    }

    /// Whether a selection point can be inside it (a `TextNode`, which a
    /// tab is too).
    pub fn is_text(&self) -> bool {
        self.kind != RunKind::LineBreak
    }

    pub fn has_format(&self, name: &str) -> bool {
        self.format & format_flag(name) != 0
    }
}

/// A child of the root: a paragraph, or later a heading, quote, list ...
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub key: String,
    /// The node's type (`"paragraph"`, `"heading"`, ...).
    pub kind: String,
    /// A heading's `tag` (`"h1"` ...), from its JSON.
    pub tag: Option<String>,
    /// The element format (`"left"`, `"center"`, ...; `""` for none).
    pub align: String,
    pub runs: Vec<Run>,
    /// The runs' text, joined.
    pub text: String,
}

/// A position in the document: a block and a byte offset in its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Pos {
    pub block: usize,
    pub byte: usize,
}

/// What a deletion at a caret takes: Backspace, Option-Backspace or
/// Command-Backspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Character,
    Word,
    Line,
}

/// A selection in positions; `anchor == focus` is a caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Sel {
    pub anchor: Pos,
    pub focus: Pos,
}

impl Sel {
    pub fn caret(pos: Pos) -> Sel {
        Sel {
            anchor: pos,
            focus: pos,
        }
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }

    /// The start and end, in document order.
    pub fn ordered(&self) -> (Pos, Pos) {
        if self.anchor <= self.focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }
}

/// A selection point as the scripts write it: `[key, offset, type]`, the
/// offset in UTF-16 code units for a text point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub key: String,
    pub offset: usize,
    pub text: bool,
}

impl Point {
    fn to_json(&self) -> Value {
        Value::Array(vec![
            self.key.as_str().into(),
            (self.offset as f64).into(),
            (if self.text { "text" } else { "element" }).into(),
        ])
    }

    fn from_json(v: &Value) -> Option<Point> {
        let a = v.as_array()?;
        Some(Point {
            key: a.first()?.as_str()?.to_owned(),
            offset: a.get(1)?.as_f64()? as usize,
            text: a.get(2)?.as_str()? == "text",
        })
    }
}

/// The editor state, as the app draws it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub blocks: Vec<Block>,
}

impl Document {
    /// Reads the current editor state. Call it in a read of the editor.
    fn read() -> Document {
        let blocks = get_root().get_children().iter().map(read_block).collect();
        Document { blocks }
    }

    /// The last position.
    pub fn end(&self) -> Pos {
        let block = self.blocks.len().saturating_sub(1);
        Pos {
            block,
            byte: self.blocks.get(block).map_or(0, |b| b.text.len()),
        }
    }

    /// The selection point at `pos`. Inside or at the end of a text run it
    /// is a text point in that run (the run before when `pos` is between
    /// two); next to line breaks and in an empty block it is an element
    /// point in the block.
    pub fn point_at(&self, pos: Pos) -> Point {
        let Some(block) = self.blocks.get(pos.block) else {
            return Point {
                key: "root".into(),
                offset: self.blocks.len(),
                text: false,
            };
        };
        let byte = floor_char_boundary(&block.text, pos.byte);
        let in_run = |r: &&Run| r.is_text() && r.start < byte && byte <= r.end();
        let at_start = |r: &&Run| r.is_text() && r.start == byte;
        if let Some(run) = block
            .runs
            .iter()
            .find(in_run)
            .or_else(|| block.runs.iter().find(at_start))
        {
            return Point {
                key: run.key.clone(),
                offset: utf16_len(&run.text[..byte - run.start]),
                text: true,
            };
        }
        Point {
            key: block.key.clone(),
            offset: block.runs.iter().filter(|r| r.start < byte).count(),
            text: false,
        }
    }

    /// The position of a selection point, if its node is drawn.
    pub fn pos_of(&self, point: &Point) -> Option<Pos> {
        if point.key == "root" {
            return Some(match self.blocks.get(point.offset) {
                Some(_) => Pos {
                    block: point.offset,
                    byte: 0,
                },
                None => self.end(),
            });
        }
        for (i, block) in self.blocks.iter().enumerate() {
            if block.key == point.key {
                let byte = block
                    .runs
                    .get(point.offset)
                    .map_or(block.text.len(), |r| r.start);
                return Some(Pos { block: i, byte });
            }
            if let Some(run) = block.runs.iter().find(|r| r.key == point.key) {
                let byte = if point.text {
                    run.start + utf16_to_byte(&run.text, point.offset)
                } else {
                    run.start
                };
                return Some(Pos { block: i, byte });
            }
        }
        None
    }

    /// The document as one string: the blocks' text joined with `\n`, as
    /// text fields that keep one paragraph per line hold it.
    pub fn text(&self) -> String {
        let texts: Vec<&str> = self.blocks.iter().map(|b| b.text.as_str()).collect();
        texts.join("\n")
    }

    /// `pos` as a byte offset in [`Document::text`].
    pub fn offset_of(&self, pos: Pos) -> usize {
        let before: usize = self.blocks[..pos.block.min(self.blocks.len())]
            .iter()
            .map(|b| b.text.len() + 1)
            .sum();
        before + pos.byte
    }

    /// The position of a byte offset in [`Document::text`] (clamped).
    pub fn pos_at(&self, mut offset: usize) -> Pos {
        for (block, b) in self.blocks.iter().enumerate() {
            if offset <= b.text.len() {
                return Pos {
                    block,
                    byte: floor_char_boundary(&b.text, offset),
                };
            }
            offset -= b.text.len() + 1;
        }
        self.end()
    }

    /// The line indices in [`Document::text`] that a line break starts
    /// (a `LineBreakNode`, not a new block): Rune's soft breaks.
    pub fn soft_breaks(&self) -> Vec<u32> {
        let mut out = Vec::new();
        let mut line = 0u32;
        for block in &self.blocks {
            for run in &block.runs {
                if run.kind == RunKind::LineBreak {
                    line += 1;
                    out.push(line);
                }
            }
            line += 1;
        }
        out
    }

    /// The tree as text, a node per line with its key, like Lexical's
    /// TreeView: `(1) paragraph`, then `  (2) text "Hello" bold`.
    pub fn tree(&self) -> String {
        const FORMATS: [&str; 11] = [
            "bold",
            "italic",
            "strikethrough",
            "underline",
            "code",
            "subscript",
            "superscript",
            "highlight",
            "lowercase",
            "uppercase",
            "capitalize",
        ];
        let mut out = String::from("(root) root\n");
        for block in &self.blocks {
            let tag = block
                .tag
                .as_deref()
                .map(|t| format!(" {t}"))
                .unwrap_or_default();
            out.push_str(&format!("  ({}) {}{tag}\n", block.key, block.kind));
            for run in &block.runs {
                let name = match run.kind {
                    RunKind::Text => "text",
                    RunKind::Tab => "tab",
                    RunKind::LineBreak => "linebreak",
                };
                out.push_str(&format!("    ({}) {name}", run.key));
                if run.kind == RunKind::Text {
                    out.push_str(&format!(" {:?}", run.text));
                }
                for format in FORMATS.iter().filter(|f| run.has_format(f)) {
                    out.push_str(&format!(" {format}"));
                }
                out.push('\n');
            }
        }
        out
    }

    /// Where deleting one `unit` from the caret at `pos` reaches, inside
    /// its block: what the DOM's `Selection.modify('extend', ...)` gives
    /// Lexical in a browser. `None` at the block's edge (Lexical merges
    /// blocks itself, without the DOM). A line is the block's line between
    /// line breaks; wrapped lines need the app's layout.
    pub fn deletion_extent(&self, pos: Pos, backward: bool, unit: Unit) -> Option<Pos> {
        use unicode_segmentation::UnicodeSegmentation;
        let text = &self.blocks.get(pos.block)?.text;
        let byte = floor_char_boundary(text, pos.byte);
        let (before, after) = text.split_at(byte);
        let reach = match (unit, backward) {
            (Unit::Character, true) => before.grapheme_indices(true).next_back().map(|(i, _)| i),
            (Unit::Character, false) => after.graphemes(true).next().map(|g| byte + g.len()),
            (Unit::Word, true) => {
                // Spaces before the caret, then the word before them.
                let words: Vec<(usize, &str)> = before.split_word_bound_indices().collect();
                let mut at = None;
                for (i, w) in words.iter().rev() {
                    at = Some(*i);
                    if !w.trim().is_empty() {
                        break;
                    }
                }
                at
            }
            (Unit::Word, false) => {
                let mut at = None;
                for (i, w) in after.split_word_bound_indices() {
                    at = Some(byte + i + w.len());
                    if !w.trim().is_empty() {
                        break;
                    }
                }
                at
            }
            (Unit::Line, true) => Some(before.rfind('\n').map_or(0, |i| i + 1))
                .filter(|&i| i < byte)
                .or_else(|| before.ends_with('\n').then(|| byte - 1)),
            (Unit::Line, false) => Some(after.find('\n').map_or(text.len(), |i| byte + i))
                .filter(|&i| i > byte)
                .or_else(|| after.starts_with('\n').then_some(byte + 1)),
        }?;
        (reach != byte).then_some(Pos {
            block: pos.block,
            byte: reach,
        })
    }

    /// The position one character before or after `pos`, across blocks.
    pub fn step(&self, pos: Pos, backward: bool) -> Pos {
        let Some(block) = self.blocks.get(pos.block) else {
            return self.end();
        };
        if backward {
            if pos.byte > 0 {
                let byte = block.text[..pos.byte]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(i, _)| i);
                return Pos {
                    block: pos.block,
                    byte,
                };
            }
            if pos.block > 0 {
                let block = pos.block - 1;
                return Pos {
                    block,
                    byte: self.blocks[block].text.len(),
                };
            }
            return pos;
        }
        if let Some(c) = block.text[pos.byte..].chars().next() {
            return Pos {
                block: pos.block,
                byte: pos.byte + c.len_utf8(),
            };
        }
        if pos.block + 1 < self.blocks.len() {
            return Pos {
                block: pos.block + 1,
                byte: 0,
            };
        }
        pos
    }
}

fn read_block(node: &NodeRef) -> Block {
    let json = node.export_json(false);
    let str_field = |name: &str| json.get(name).and_then(Value::as_str).map(str::to_owned);
    let mut text = String::new();
    let mut runs = Vec::new();
    for child in node.get_children() {
        let (kind, run_text, format, style) = match child.get_type() {
            "linebreak" => (RunKind::LineBreak, "\n".to_owned(), 0, String::new()),
            _ if child.is_text() => {
                let data = child.text_data();
                let kind = if child.get_type() == "tab" {
                    RunKind::Tab
                } else {
                    RunKind::Text
                };
                (
                    kind,
                    data.get_text().to_owned(),
                    data.get_format() as u32,
                    data.get_style().to_owned(),
                )
            }
            // Inline elements (links) and decorators come with their
            // packages; their text stands in for them until then.
            _ => (RunKind::Text, child.get_text_content(), 0, String::new()),
        };
        runs.push(Run {
            key: child.get_key().to_owned(),
            kind,
            start: text.len(),
            text: run_text.clone(),
            format,
            style,
        });
        text.push_str(&run_text);
    }
    Block {
        key: node.get_key().to_owned(),
        kind: node.get_type().to_owned(),
        tag: str_field("tag"),
        align: str_field("format").unwrap_or_default(),
        runs,
        text,
    }
}

/// The flag of a text format (`"bold"` → 1), as `TEXT_TYPE_TO_FORMAT`.
pub fn format_flag(name: &str) -> u32 {
    lexical::toggle_text_format_type(0, name, None) as u32
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The byte offset of the UTF-16 offset `units` in `s`, clamped; an offset
/// inside a surrogate pair rounds down to the character's start.
fn utf16_to_byte(s: &str, units: usize) -> usize {
    let mut seen = 0;
    for (i, c) in s.char_indices() {
        if seen + c.len_utf16() > units {
            return i;
        }
        seen += c.len_utf16();
    }
    s.len()
}

/// The length in bytes of the longest common prefix, on a char boundary.
fn common_prefix(a: &str, b: &str) -> usize {
    a.char_indices()
        .zip(b.chars())
        .find(|((_, x), y)| x != y)
        .map_or(a.len().min(b.len()), |((i, _), _)| i)
        .min(a.len())
        .min(b.len())
}

/// The length in bytes of the longest common suffix, on a char boundary.
fn common_suffix(a: &str, b: &str) -> usize {
    let mut n = 0;
    for (x, y) in a.chars().rev().zip(b.chars().rev()) {
        if x != y {
            break;
        }
        n += x.len_utf8();
    }
    n
}

/// `JSON.stringify(value, null, 2)`.
pub fn pretty_json(value: &Value) -> String {
    fn write(value: &Value, indent: usize, out: &mut String) {
        let pad = |n: usize| "  ".repeat(n);
        match value {
            Value::Array(items) if !items.is_empty() => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    out.push_str(&pad(indent + 1));
                    write(item, indent + 1, out);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(indent));
                out.push(']');
            }
            Value::Object(object) if !object.is_empty() => {
                out.push_str("{\n");
                let n = object.len();
                for (i, (key, item)) in object.iter().enumerate() {
                    out.push_str(&pad(indent + 1));
                    out.push_str(&Value::from(key).to_string());
                    out.push_str(": ");
                    write(item, indent + 1, out);
                    out.push_str(if i + 1 < n { ",\n" } else { "\n" });
                }
                out.push_str(&pad(indent));
                out.push('}');
            }
            other => out.push_str(&other.to_string()),
        }
    }
    let mut out = String::new();
    write(value, 0, &mut out);
    out
}

fn floor_char_boundary(s: &str, byte: usize) -> usize {
    let mut byte = byte.min(s.len());
    while !s.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// One edit's outcome.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// Why the step failed, when it did (the editor discarded the update).
    pub error: Option<String>,
}

/// How long typing keeps merging into one undo step, in ms:
/// `HistoryExtension`'s default delay.
pub const HISTORY_DELAY: f64 = 300.0;

/// A clock in milliseconds, for history's merging by time.
pub type Clock = Rc<dyn Fn() -> f64>;

/// A playground editor: a script session, the document it draws, and the
/// selection.
pub struct Playground {
    session: Session,
    /// The session's script without its steps: history and the CAN_UNDO /
    /// CAN_REDO handlers, which the exported case carries for JS.
    script: Object,
    /// Each step's `now`, which history merges typing by.
    clock: Clock,
    can_undo: bool,
    can_redo: bool,
    steps: Vec<Value>,
    results: Vec<Value>,
    document: Document,
    /// The editor's selection, as the last step recorded it.
    model_selection: Value,
    /// The selection the app shows; it reaches the editor with the next
    /// edit (as a `select` action), as Lexical reads the DOM selection.
    selection: Option<Sel>,
    last_error: Option<String>,
}

impl Playground {
    /// A playground whose root starts with `children` (serialized nodes),
    /// with undo and redo (`@lexical/history`) on the wall clock.
    pub fn new(children: Vec<Value>) -> Result<Playground, String> {
        let start = Instant::now();
        Playground::with_clock(
            children,
            Rc::new(move || start.elapsed().as_millis() as f64),
        )
    }

    /// [`Playground::new`] on `clock` (ms), which stamps each step's `now`.
    pub fn with_clock(children: Vec<Value>, clock: Clock) -> Result<Playground, String> {
        let mut script = Object::new();
        script.insert("name", "playground".into());
        let mut history = Object::new();
        history.insert("delay", HISTORY_DELAY.into());
        script.insert("history", Value::Object(history));
        // History says whether undo and redo can run; a handler that
        // doesn't stop the command logs it.
        let handler = |id: &str, command: &str| {
            let mut h = Object::new();
            h.insert("id", id.into());
            h.insert("command", command.into());
            h.insert("result", false.into());
            Value::Object(h)
        };
        script.insert(
            "handlers",
            Value::Array(vec![
                handler("canUndo", "CAN_UNDO_COMMAND"),
                handler("canRedo", "CAN_REDO_COMMAND"),
            ]),
        );
        let session = Session::start(&Value::Object(script.clone()))?;
        let mut playground = Playground {
            session,
            script,
            clock,
            can_undo: false,
            can_redo: false,
            steps: Vec::new(),
            results: Vec::new(),
            document: Document::default(),
            model_selection: Value::Null,
            selection: None,
            last_error: None,
        };
        let mut append = Object::new();
        append.insert("do", "append".into());
        append.insert("key", "root".into());
        append.insert("nodes", Value::Array(children));
        playground.update(vec![Value::Object(append)]);
        match playground.last_error.take() {
            Some(e) => Err(e),
            None => Ok(playground),
        }
    }

    /// [`Playground::new`] with the root's children from an editor state's
    /// JSON (`{"root": {"children": [...]}}`).
    pub fn from_json(state: &str) -> Result<Playground, String> {
        let state = json::parse(state).map_err(|e| e.to_string())?;
        let children = state
            .get("root")
            .and_then(|r| r.get("children"))
            .and_then(Value::as_array)
            .cloned()
            .ok_or("no root.children")?;
        Playground::new(children)
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn selection(&self) -> Option<Sel> {
        self.selection
    }

    /// Moves the selection the app shows; the editor gets it with the next
    /// edit.
    pub fn set_selection(&mut self, selection: Option<Sel>) {
        self.selection = selection;
    }

    /// The last edit's error, if it failed.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// The selection's pending format (what typed text gets).
    pub fn selection_format(&self) -> u32 {
        self.model_selection
            .as_array()
            .filter(|a| a.first().and_then(Value::as_str) == Some("range"))
            .and_then(|a| a.get(3))
            .and_then(Value::as_f64)
            .map_or(0, |f| f as u32)
    }

    /// Whether `name` (`"bold"`, ...) is on at the selection: in the pending
    /// format, as Lexical's toolbar reads it.
    pub fn has_format(&self, name: &str) -> bool {
        self.selection_format() & format_flag(name) != 0
    }

    /// The editor state's JSON (`JSON.stringify(editorState.toJSON())`).
    pub fn state_json(&self) -> String {
        self.session
            .editor()
            .get_editor_state()
            .to_json(false)
            .to_string()
    }

    /// The editor state's JSON, indented.
    pub fn state_pretty_json(&self) -> String {
        pretty_json(&self.session.editor().get_editor_state().to_json(false))
    }

    /// How many steps the session has run.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    pub fn insert_text(&mut self, text: &str) -> Outcome {
        self.edit("insertText", vec![text.into()])
    }

    pub fn insert_raw_text(&mut self, text: &str) -> Outcome {
        self.edit("insertRawText", vec![text.into()])
    }

    pub fn insert_paragraph(&mut self) -> Outcome {
        self.edit("insertParagraph", vec![])
    }

    pub fn insert_line_break(&mut self) -> Outcome {
        self.edit("insertLineBreak", vec![])
    }

    /// Backspace (`backward`) or Delete: `deleteCharacter`, `deleteWord` or
    /// `deleteLine`. At a caret inside a block the selection is first
    /// extended over what the key deletes ([`Document::deletion_extent`]),
    /// as the browser's `Selection.modify` extends it for Lexical; at a
    /// block's edge Lexical merges the blocks itself. So each step runs the
    /// same in JS headless, and an exported session replays there.
    /// Backspace at the document's start and Delete at its end do nothing,
    /// as in a browser (headless, JS has no `Selection.modify` and throws).
    pub fn delete(&mut self, unit: Unit, backward: bool) -> Outcome {
        if let Some(sel) = self.selection
            && sel.is_collapsed()
        {
            match self.document.deletion_extent(sel.focus, backward, unit) {
                Some(reach) => {
                    self.selection = Some(Sel {
                        anchor: sel.focus,
                        focus: reach,
                    });
                }
                None if sel.focus
                    == if backward {
                        Pos::default()
                    } else {
                        self.document.end()
                    } =>
                {
                    return Outcome::default();
                }
                None => {}
            }
        }
        let method = match unit {
            Unit::Character => "deleteCharacter",
            Unit::Word => "deleteWord",
            Unit::Line => "deleteLine",
        };
        self.edit(method, vec![backward.into()])
    }

    pub fn delete_character(&mut self, backward: bool) -> Outcome {
        self.delete(Unit::Character, backward)
    }

    pub fn delete_word(&mut self, backward: bool) -> Outcome {
        self.delete(Unit::Word, backward)
    }

    pub fn delete_line(&mut self, backward: bool) -> Outcome {
        self.delete(Unit::Line, backward)
    }

    pub fn remove_text(&mut self) -> Outcome {
        self.edit("removeText", vec![])
    }

    /// `formatText(name)`: toggles a format on the selected text, or on the
    /// pending format at a caret.
    pub fn format_text(&mut self, name: &str) -> Outcome {
        self.edit("formatText", vec![name.into()])
    }

    /// ⌘Z: `UNDO_COMMAND`, which history handles. The app's selection
    /// reaches the editor first, as a browser's `selectionchange` does.
    pub fn undo(&mut self) -> Outcome {
        self.dispatch("UNDO_COMMAND")
    }

    /// ⌘⇧Z: `REDO_COMMAND`.
    pub fn redo(&mut self) -> Outcome {
        self.dispatch("REDO_COMMAND")
    }

    /// Whether there is something to undo, as history last said
    /// (`CAN_UNDO_COMMAND`).
    pub fn can_undo(&self) -> bool {
        self.can_undo
    }

    /// Whether there is something to redo (`CAN_REDO_COMMAND`).
    pub fn can_redo(&self) -> bool {
        self.can_redo
    }

    fn dispatch(&mut self, command: &str) -> Outcome {
        if let Some(select) = self.select_action() {
            let outcome = self.run_update(vec![select]);
            if outcome.error.is_some() {
                return outcome;
            }
        }
        let mut step = Object::new();
        step.insert("dispatch", command.into());
        self.run(Value::Object(step))
    }

    /// `$selectAll()`.
    pub fn select_all(&mut self) -> Outcome {
        let mut action = Object::new();
        action.insert("do", "selectAll".into());
        self.update(vec![Value::Object(action)])
    }

    /// The text the selection covers.
    pub fn selected_text(&self) -> String {
        let Some(sel) = self.selection else {
            return String::new();
        };
        let (start, end) = sel.ordered();
        let blocks = &self.document.blocks;
        let mut out = String::new();
        for (i, block) in blocks
            .iter()
            .enumerate()
            .take(end.block + 1)
            .skip(start.block)
        {
            let text = &block.text;
            let from = if i == start.block { start.byte } else { 0 };
            let to = if i == end.block { end.byte } else { text.len() };
            if i > start.block {
                out.push_str("\n\n");
            }
            out.push_str(&text[from.min(to)..to]);
        }
        out
    }

    /// Replays a change a text field made on its own (an input method's
    /// commit, a cut, a drag) as a Lexical edit: the text that differs
    /// between [`Document::text`] and `new_text` is selected and replaced
    /// (`insertText`, `insertRawText` when it has new lines, or `removeText`).
    /// Returns `None` when the text is the same.
    pub fn apply_text(&mut self, new_text: &str) -> Option<Outcome> {
        let old = self.document.text();
        if old == new_text {
            return None;
        }
        let prefix = common_prefix(&old, new_text);
        let suffix = common_suffix(&old[prefix..], &new_text[prefix..]);
        let inserted = &new_text[prefix..new_text.len() - suffix];
        self.selection = Some(Sel {
            anchor: self.document.pos_at(prefix),
            focus: self.document.pos_at(old.len() - suffix),
        });
        Some(if inserted.is_empty() {
            self.remove_text()
        } else if inserted.contains('\n') {
            self.insert_raw_text(inserted)
        } else {
            self.insert_text(inserted)
        })
    }

    fn edit(&mut self, method: &str, args: Vec<Value>) -> Outcome {
        let mut action = Object::new();
        action.insert("do", "selection".into());
        action.insert("method", method.into());
        action.insert("args", Value::Array(args));
        self.update(vec![Value::Object(action)])
    }

    /// Runs `actions` as one update step. When the app's selection isn't
    /// the editor's, a step with a `select` action runs first, as a
    /// browser's `selectionchange` reaches Lexical before the key does, so
    /// history merges typing after a click as it does in a browser.
    pub fn update(&mut self, actions: Vec<Value>) -> Outcome {
        if let Some(select) = self.select_action() {
            let outcome = self.run_update(vec![select]);
            if outcome.error.is_some() || actions.is_empty() {
                return outcome;
            }
        }
        self.run_update(actions)
    }

    fn run_update(&mut self, actions: Vec<Value>) -> Outcome {
        let mut step = Object::new();
        step.insert("update", Value::Array(actions));
        self.run(Value::Object(step))
    }

    fn select_action(&self) -> Option<Value> {
        let sel = self.selection?;
        let anchor = self.document.point_at(sel.anchor).to_json();
        let focus = self.document.point_at(sel.focus).to_json();
        let current = self.model_selection.as_array();
        if let Some(c) = current
            && c.first().and_then(Value::as_str) == Some("range")
            && c.get(1) == Some(&anchor)
            && c.get(2) == Some(&focus)
        {
            return None;
        }
        let mut action = Object::new();
        action.insert("do", "select".into());
        action.insert("anchor", anchor);
        action.insert("focus", focus);
        Some(Value::Object(action))
    }

    fn run(&mut self, step: Value) -> Outcome {
        let mut step = step;
        if let Value::Object(o) = &mut step {
            o.insert("now", (self.clock)().into());
        }
        let result = match self.session.run(&step, true) {
            Ok(r) => r,
            Err(e) => {
                self.last_error = Some(e.clone());
                return Outcome { error: Some(e) };
            }
        };
        self.steps.push(step);
        self.results.push(result.json.clone());
        self.document = self
            .session
            .editor()
            .read(Document::read)
            .unwrap_or_default();
        for entry in result
            .json
            .get("log")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let entry = entry.as_array().map(Vec::as_slice).unwrap_or_default();
            if let [kind, id, can] = entry
                && kind.as_str() == Some("handler")
                && let Some(can) = can.as_bool()
            {
                match id.as_str() {
                    Some("canUndo") => self.can_undo = can,
                    Some("canRedo") => self.can_redo = can,
                    _ => {}
                }
            }
        }
        self.model_selection = result.json.get("selection").cloned().unwrap_or(Value::Null);
        self.selection = self.selection_from_model();
        self.last_error = result.error.map(|e| e.to_string());
        Outcome {
            error: self.last_error.clone(),
        }
    }

    fn selection_from_model(&self) -> Option<Sel> {
        let s = self.model_selection.as_array()?;
        if s.first()?.as_str()? != "range" {
            return None;
        }
        let anchor = self.document.pos_of(&Point::from_json(s.get(1)?)?)?;
        let focus = self.document.pos_of(&Point::from_json(s.get(2)?)?)?;
        Some(Sel { anchor, focus })
    }

    /// The session as a `scripts/*` vector case: its steps, and the result
    /// of each as this editor recorded it. JS Lexical replays it and
    /// compares (`packages/lexical-conformance/src/replay.test.ts`).
    pub fn export_case(&self, name: &str) -> Value {
        let mut case = self.script.clone();
        case.insert("name", name.into());
        case.insert("steps", Value::Array(self.steps.clone()));
        case.insert("results", Value::Array(self.results.clone()));
        Value::Object(case)
    }
}

impl Playground {
    /// [`Playground::export_case`] as a `scripts/*` vector file, which
    /// `LEXICAL_REPLAY=<file> pnpm exec vitest run src/replay.test.ts`, in
    /// `packages/lexical-conformance`, replays in JS.
    pub fn export_vector_file(&self, name: &str) -> String {
        let mut file = Object::new();
        file.insert(
            "description",
            "A playground session, recorded by lexical-editor.".into(),
        );
        file.insert("formatVersion", 1.0.into());
        file.insert("kind", "script".into());
        file.insert("cases", Value::Array(vec![self.export_case(name)]));
        pretty_json(&Value::Object(file))
    }
}

/// Replays the conformance vectors (`packages/lexical-conformance/vectors`,
/// or `$LEXICAL_CONFORMANCE_VECTORS`) and returns a plain-text summary sized
/// for an app's side panel, and whether nothing failed.
pub fn replay_vectors() -> Result<(String, bool), String> {
    use lexical_conformance::Status;
    use std::collections::BTreeMap;
    use std::fmt::Write;
    let report = lexical_conformance::run(None)?;
    let replayed: Vec<_> = report
        .files
        .iter()
        .filter(|f| f.status != Status::NotImplemented)
        .collect();
    let sum = |n: fn(&lexical_conformance::FileReport) -> usize| -> usize {
        replayed.iter().map(|f| n(f)).sum()
    };
    let waiting = |f: &lexical_conformance::FileReport| f.not_ported.values().sum::<usize>();
    let mut out = format!(
        "{} cases · {} match JS · {} wait on a package · {} fail\n\n",
        sum(|f| f.cases),
        sum(|f| f.passed + f.exceptions.values().sum::<usize>()),
        sum(waiting),
        sum(|f| f.failed),
    );
    let width = replayed.iter().map(|f| f.file.len()).max().unwrap_or(0);
    for f in &replayed {
        let checked = f.cases - waiting(f);
        let _ = write!(out, "{:width$}  {}/{}", f.file, checked - f.failed, checked);
        if let Status::Unreadable(e) = &f.status {
            let _ = write!(out, "  unreadable: {e}");
        }
        if waiting(f) > 0 {
            let _ = write!(out, "  (+{} waiting)", waiting(f));
        }
        out.push('\n');
    }
    let mut by_package: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &replayed {
        for (package, n) in &f.not_ported {
            *by_package.entry(package).or_default() += n;
        }
    }
    if !by_package.is_empty() {
        let list: Vec<String> = by_package.iter().map(|(p, n)| format!("{p} {n}")).collect();
        let _ = write!(out, "\nWaiting on: {}\n", list.join(", "));
    }
    let _ = writeln!(
        out,
        "Not replayed yet: {} vector files",
        report.files.len() - replayed.len()
    );
    for f in &replayed {
        for message in &f.failures {
            let _ = write!(out, "\n{}: {message}\n", f.file);
        }
    }
    Ok((out, report.ok()))
}

/// The document the playgrounds open with.
pub fn example() -> Vec<Value> {
    let text = |text: &str, format: f64| {
        let mut o = Object::new();
        o.insert("type", "text".into());
        o.insert("text", text.into());
        o.insert("format", format.into());
        Value::Object(o)
    };
    let paragraph = |children: Vec<Value>| {
        let mut o = Object::new();
        o.insert("type", "paragraph".into());
        o.insert("children", Value::Array(children));
        Value::Object(o)
    };
    vec![
        paragraph(vec![
            text("Hello from ", 0.0),
            text("Rust", 1.0),
            text(". This document lives in a ", 0.0),
            text("lexical-editor", 16.0),
            text(" editor.", 0.0),
        ]),
        paragraph(vec![text(
            "Type, delete, press Enter, or select text and make it bold.",
            0.0,
        )]),
    ]
}

#[cfg(test)]
mod tests;

//! An editor surface for UI frameworks that hand the app raw text input,
//! as GPUI does with macOS's `NSTextInputClient` (`EntityInputHandler`):
//! the app draws the text, and the framework asks it for ranges in UTF-16
//! code units, replaces text, and marks an input method's composition.
//!
//! Offsets are bytes in [`Surface::display_text`]: the document's text
//! ([`Document::text`]) with the composition, which Lexical doesn't see
//! until the input method commits it, in place of what it replaces.

use crate::{Outcome, Playground, Sel, Unit};
use std::ops::Range;

/// An input method's composition: `text` shown in place of `replaced`
/// (bytes of the document's text), with its own selection.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Composition {
    replaced: Range<usize>,
    text: String,
    selected: Range<usize>,
}

/// A run of [`Surface::display_text`] to draw: a block's run with its
/// format, the newline between lines, or the composition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayRun {
    pub range: Range<usize>,
    pub format: u32,
    pub composing: bool,
}

pub struct Surface {
    pub playground: Playground,
    /// The selection in the document's text, `anchor` where it started.
    anchor: usize,
    focus: usize,
    composition: Option<Composition>,
}

impl Surface {
    pub fn new(playground: Playground) -> Surface {
        let mut surface = Surface {
            playground,
            anchor: 0,
            focus: 0,
            composition: None,
        };
        surface.sync();
        surface
    }

    /// The text to draw.
    pub fn display_text(&self) -> String {
        let text = self.playground.document().text();
        match &self.composition {
            None => text,
            Some(c) => format!(
                "{}{}{}",
                &text[..c.replaced.start],
                c.text,
                &text[c.replaced.end..]
            ),
        }
    }

    /// The selection in the display text, in document order, and whether
    /// the focus is at its start.
    pub fn selection(&self) -> (Range<usize>, bool) {
        if let Some(c) = &self.composition {
            let start = c.replaced.start;
            return (start + c.selected.start..start + c.selected.end, false);
        }
        let (a, f) = (self.anchor, self.focus);
        (a.min(f)..a.max(f), f < a)
    }

    /// Where the caret is drawn: the selection's focus.
    pub fn caret(&self) -> usize {
        let (range, reversed) = self.selection();
        if reversed { range.start } else { range.end }
    }

    /// The composition's range in the display text.
    pub fn marked_range(&self) -> Option<Range<usize>> {
        let c = self.composition.as_ref()?;
        Some(c.replaced.start..c.replaced.start + c.text.len())
    }

    /// The display text in runs to draw: each text run's format, the
    /// newlines between lines, and the composition, in order.
    pub fn runs(&self) -> Vec<DisplayRun> {
        let mut runs = Vec::new();
        let mut offset = 0;
        let blocks = &self.playground.document().blocks;
        for (i, block) in blocks.iter().enumerate() {
            for run in &block.runs {
                runs.push(DisplayRun {
                    range: offset + run.start..offset + run.end(),
                    format: run.format,
                    composing: false,
                });
            }
            offset += block.text.len();
            if i + 1 < blocks.len() {
                runs.push(DisplayRun {
                    range: offset..offset + 1,
                    format: 0,
                    composing: false,
                });
                offset += 1;
            }
        }
        let Some(c) = &self.composition else {
            return runs;
        };
        // Cut the composition's text in where it replaces the document's.
        let delta = c.text.len() as isize - c.replaced.len() as isize;
        let shift = |x: usize| (x as isize + delta) as usize;
        let mut out = Vec::new();
        for run in runs {
            let before = run.range.start..run.range.end.min(c.replaced.start);
            let after = run.range.start.max(c.replaced.end)..run.range.end;
            if before.start < before.end {
                out.push(DisplayRun {
                    range: before,
                    ..run.clone()
                });
            }
            if run.range.end >= c.replaced.start
                && !out.iter().any(|r: &DisplayRun| r.composing)
                && run.range.start <= c.replaced.start
            {
                out.push(DisplayRun {
                    range: c.replaced.start..c.replaced.start + c.text.len(),
                    format: run.format,
                    composing: true,
                });
            }
            if after.start < after.end {
                out.push(DisplayRun {
                    range: shift(after.start)..shift(after.end),
                    ..run
                });
            }
        }
        if !out.iter().any(|r| r.composing) {
            out.push(DisplayRun {
                range: c.replaced.start..c.replaced.start + c.text.len(),
                format: 0,
                composing: true,
            });
        }
        out.retain(|r| r.range.start < r.range.end);
        out
    }

    /// Takes the selection from the editor, after an edit.
    fn sync(&mut self) {
        let doc = self.playground.document();
        match self.playground.selection() {
            Some(sel) => {
                self.anchor = doc.offset_of(sel.anchor);
                self.focus = doc.offset_of(sel.focus);
            }
            None => {
                let end = doc.text().len();
                self.anchor = self.anchor.min(end);
                self.focus = self.focus.min(end);
            }
        }
    }

    /// Runs an edit at the selection.
    pub fn edit(&mut self, f: impl FnOnce(&mut Playground) -> Outcome) -> Outcome {
        self.commit_composition();
        self.push_selection();
        let outcome = f(&mut self.playground);
        self.sync();
        outcome
    }

    fn push_selection(&mut self) {
        let doc = self.playground.document();
        let sel = Sel {
            anchor: doc.pos_at(self.anchor),
            focus: doc.pos_at(self.focus),
        };
        self.playground.set_selection(Some(sel));
    }

    /// Moves the caret to `offset` (in the display text, which is the
    /// document's text outside a composition), or with `extend` the focus.
    pub fn move_to(&mut self, offset: usize, extend: bool) {
        self.commit_composition();
        let text = self.playground.document().text();
        let offset = floor_char_boundary(&text, offset);
        self.focus = offset;
        if !extend {
            self.anchor = offset;
        }
        self.push_selection();
    }

    pub fn select_all(&mut self) {
        self.commit_composition();
        self.anchor = 0;
        self.focus = self.playground.document().text().len();
        self.push_selection();
    }

    /// The selected text.
    pub fn selected_text(&self) -> String {
        let (range, _) = self.selection();
        self.display_text()[range].to_owned()
    }

    /// Backspace or Delete, by `unit`.
    pub fn delete(&mut self, unit: Unit, backward: bool) -> Outcome {
        self.edit(|p| p.delete(unit, backward))
    }

    // The text input client, in UTF-16 code units of the display text.

    pub fn text_length_utf16(&self) -> usize {
        self.display_text().encode_utf16().count()
    }

    /// `text_for_range`: the text, and the range it really covers.
    pub fn text_for_range_utf16(&self, range: Range<usize>) -> (String, Range<usize>) {
        let text = self.display_text();
        let bytes = from_utf16_range(&text, &range);
        let actual = to_utf16_range(&text, &bytes);
        (text[bytes].to_owned(), actual)
    }

    /// `selected_text_range`: the selection and whether it's reversed.
    pub fn selected_range_utf16(&self) -> (Range<usize>, bool) {
        let (range, reversed) = self.selection();
        (to_utf16_range(&self.display_text(), &range), reversed)
    }

    pub fn set_selected_range_utf16(&mut self, range: Range<usize>) {
        if self.composition.is_some() {
            let text = self.display_text();
            let bytes = from_utf16_range(&text, &range);
            if let Some(c) = &mut self.composition {
                let start = c.replaced.start;
                let end = start + c.text.len();
                if bytes.start >= start && bytes.end <= end {
                    c.selected = bytes.start - start..bytes.end - start;
                    return;
                }
            }
        }
        let text = self.playground.document().text();
        let bytes = from_utf16_range(&text, &range);
        self.move_to(bytes.start, false);
        self.move_to(bytes.end, true);
    }

    pub fn marked_range_utf16(&self) -> Option<Range<usize>> {
        let range = self.marked_range()?;
        Some(to_utf16_range(&self.display_text(), &range))
    }

    /// `unmark_text`: the input method keeps its composition as it is.
    pub fn unmark(&mut self) -> Option<Outcome> {
        self.commit_composition()
    }

    /// `replace_text_in_range`: typed or committed text. `range` (else the
    /// composition, else the selection) is replaced by `text` in Lexical:
    /// `insertText`, `insertRawText` for several lines, `removeText` for
    /// none.
    pub fn replace_text_utf16(&mut self, range: Option<Range<usize>>, text: &str) -> Outcome {
        let replaced = match (self.composition.take(), range) {
            (Some(c), _) => c.replaced,
            (None, Some(range)) => from_utf16_range(&self.playground.document().text(), &range),
            (None, None) => self.selection().0,
        };
        self.anchor = replaced.start;
        self.focus = replaced.end;
        self.edit(|p| {
            if text.is_empty() {
                p.remove_text()
            } else if text.contains('\n') {
                p.insert_raw_text(text)
            } else {
                p.insert_text(text)
            }
        })
    }

    /// `replace_and_mark_text_in_range`: an input method's composition.
    /// Lexical doesn't see it until it's committed; an empty one ends it.
    pub fn replace_and_mark_utf16(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) {
        let replaced = match (&self.composition, range) {
            (Some(c), _) => c.replaced.clone(),
            (None, Some(range)) => from_utf16_range(&self.playground.document().text(), &range),
            (None, None) => self.selection().0,
        };
        if text.is_empty() {
            self.composition = None;
            self.anchor = replaced.start;
            self.focus = replaced.start;
            return;
        }
        let selected = selected.map_or(text.len()..text.len(), |r| from_utf16_range(text, &r));
        self.composition = Some(Composition {
            replaced,
            text: text.to_owned(),
            selected,
        });
    }

    fn commit_composition(&mut self) -> Option<Outcome> {
        let c = self.composition.take()?;
        self.anchor = c.replaced.start;
        self.focus = c.replaced.end;
        self.push_selection();
        let outcome = self.playground.insert_text(&c.text);
        self.sync();
        Some(outcome)
    }
}

fn floor_char_boundary(s: &str, byte: usize) -> usize {
    let mut byte = byte.min(s.len());
    while !s.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// The byte offset of the UTF-16 offset `units` (clamped, rounded down to
/// a character).
pub fn byte_of_utf16(s: &str, units: usize) -> usize {
    let mut seen = 0;
    for (i, c) in s.char_indices() {
        if seen + c.len_utf16() > units {
            return i;
        }
        seen += c.len_utf16();
    }
    s.len()
}

/// The UTF-16 offset of the byte offset `byte`.
pub fn utf16_of_byte(s: &str, byte: usize) -> usize {
    s[..floor_char_boundary(s, byte)].encode_utf16().count()
}

fn from_utf16_range(s: &str, range: &Range<usize>) -> Range<usize> {
    byte_of_utf16(s, range.start)..byte_of_utf16(s, range.end.max(range.start))
}

fn to_utf16_range(s: &str, range: &Range<usize>) -> Range<usize> {
    utf16_of_byte(s, range.start)..utf16_of_byte(s, range.end)
}

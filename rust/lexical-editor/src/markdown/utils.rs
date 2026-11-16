//! `utils.ts`, and the JS string operations the markdown port needs on
//! UTF-16 code units.
//!
//! A JS string indexed by `text[i]` is a code unit, which can be half of a
//! surrogate pair; here such a "character" is a `u32` that keeps the
//! surrogate's value, so the flanking rules see what JS sees. `characterAt`
//! gives a whole code point.

use super::markdown_transformers::{
    ElementTransformer, MultilineElementTransformer, TextFormatTransformer, TextMatchTransformer,
    Transformer,
};
use super::reg_exp::{RegExp, in_table, is_js_space};
use super::reg_exp_tables::{PUNCTUATION, SPACE_SEPARATOR, SYMBOL};
use crate::NodeRef;
use crate::nodes::line_break::LineBreakNode;
use crate::nodes::paragraph::ParagraphNode;
use crate::nodes::tab::TabNode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A compiled JS regular expression for a pattern of upstream's, compiled
/// once per thread. Each call returns a copy with `lastIndex` 0.
pub(crate) fn re(source: &'static str, flags: &'static str) -> RegExp {
    thread_local! {
        static CACHE: RefCell<HashMap<(&'static str, &'static str), RegExp>> =
            RefCell::new(HashMap::new());
    }
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry((source, flags))
            .or_insert_with(|| RegExp::of(source, flags))
            .clone()
    })
}

/// The UTF-16 code units of `text`.
pub(crate) fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// A string of UTF-16 code units (a lone surrogate as U+FFFD, under the
/// unpaired surrogate exception).
pub(crate) fn string(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// `text[i]`, or `None` (`undefined`) past either end.
pub(crate) fn unit_at(text: &[u16], index: isize) -> Option<u32> {
    if index < 0 {
        return None;
    }
    text.get(index as usize).map(|u| u32::from(*u))
}

/// A character as a JS string, for the few places that write one out.
pub(crate) fn char_string(character: u32) -> String {
    char::from_u32(character).map_or_else(|| "\u{FFFD}".to_owned(), String::from)
}

/// How many code units a character takes.
pub(crate) fn char_units(character: u32) -> usize {
    if character > 0xFFFF { 2 } else { 1 }
}

/// Whether a code unit or code point is in JS's `\s`.
pub(crate) fn is_space(character: u32) -> bool {
    is_js_space(character)
}

/// `text.trimStart()`, with JS's whitespace.
pub(crate) fn trim_start(text: &str) -> &str {
    text.trim_start_matches(|c: char| is_js_space(c as u32))
}

/// `text.trimEnd()`, with JS's whitespace.
pub(crate) fn trim_end(text: &str) -> &str {
    text.trim_end_matches(|c: char| is_js_space(c as u32))
}

/// `text.trim()`, with JS's whitespace.
pub(crate) fn trim(text: &str) -> &str {
    trim_end(trim_start(text))
}

/// `/\S/.test(text)`.
pub(crate) fn has_non_space(text: &str) -> bool {
    text.chars().any(|c| !is_js_space(c as u32))
}

/// What `transformersByType` returns.
#[derive(Clone, Default)]
pub(crate) struct ByType {
    pub element: Vec<Rc<ElementTransformer>>,
    pub multiline_element: Vec<Rc<MultilineElementTransformer>>,
    pub text_format: Vec<Rc<TextFormatTransformer>>,
    pub text_match: Vec<Rc<TextMatchTransformer>>,
}

/// `transformersByType(transformers)`.
pub(crate) fn transformers_by_type(transformers: &[Transformer]) -> ByType {
    let mut by_type = ByType::default();
    for transformer in transformers {
        match transformer {
            Transformer::Element(t) => by_type.element.push(t.clone()),
            Transformer::MultilineElement(t) => by_type.multiline_element.push(t.clone()),
            Transformer::TextFormat(t) => by_type.text_format.push(t.clone()),
            Transformer::TextMatch(t) => by_type.text_match.push(t.clone()),
        }
    }
    by_type
}

// CommonMark counts every character in the Unicode P (punctuation) and S
// (symbol) general categories as punctuation, which includes all of the
// ASCII punctuation characters. Its Unicode whitespace is Zs plus tab, line
// feed, form feed and carriage return; JavaScript's `\s` adds others, such
// as U+FEFF. Delimiter flanking depends on both.

/// `PUNCTUATION`: `/[\p{P}\p{S}]/u`.
fn is_punctuation(character: u32) -> bool {
    in_table(PUNCTUATION, character) || in_table(SYMBOL, character)
}

/// `WHITESPACE`: `/[\p{Zs}\t\n\f\r]/u`.
fn is_whitespace(character: u32) -> bool {
    matches!(character, 0x09 | 0x0A | 0x0C | 0x0D) || in_table(SPACE_SEPARATOR, character)
}

const WHITESPACE_CLASS: u8 = 0;
const PUNCTUATION_CLASS: u8 = 1;
const OTHER_CLASS: u8 = 2;

/// `classify`: how flanking sees `character`; the edge of the line is
/// whitespace.
fn classify(character: Option<u32>) -> u8 {
    match character {
        None => WHITESPACE_CLASS,
        Some(c) if is_whitespace(c) => WHITESPACE_CLASS,
        Some(c) if is_punctuation(c) => PUNCTUATION_CLASS,
        Some(_) => OTHER_CLASS,
    }
}

/// `isFlankedBy(primary, secondary)`: whether a delimiter run with
/// `primary` on the side it faces and `secondary` on the other side is
/// flanking. `None` is the edge of the line.
pub(crate) fn is_flanked_by(primary: Option<u32>, secondary: Option<u32>) -> bool {
    is_flanked_by_class(classify(primary), classify(secondary))
}

fn is_flanked_by_class(primary: u8, secondary: u8) -> bool {
    primary == OTHER_CLASS || (primary == PUNCTUATION_CLASS && secondary != OTHER_CLASS)
}

/// What `getEmphasisSides` returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EmphasisSides {
    pub can_open: bool,
    pub can_close: bool,
}

/// `getEmphasisSides(marker, before, after)`: whether a delimiter run of
/// `marker` between `before` and `after` can open and close emphasis; `_`
/// opens or closes inside a word only beside punctuation.
pub(crate) fn get_emphasis_sides(
    marker: u32,
    before: Option<u32>,
    after: Option<u32>,
) -> EmphasisSides {
    let before_class = classify(before);
    let after_class = classify(after);
    let is_left = is_flanked_by_class(after_class, before_class);
    let is_right = is_flanked_by_class(before_class, after_class);
    if marker == u32::from(b'_') {
        EmphasisSides {
            can_close: is_right && (!is_left || after_class == PUNCTUATION_CLASS),
            can_open: is_left && (!is_right || before_class == PUNCTUATION_CLASS),
        }
    } else {
        EmphasisSides {
            can_close: is_right,
            can_open: is_left,
        }
    }
}

/// `isPunctuationOrSpace(character)`.
pub(crate) fn is_punctuation_or_space(character: Option<u32>) -> bool {
    classify(character) != OTHER_CLASS
}

// Readers built on micromark, such as `@lexical/mdast`, read a character
// outside the Basic Multilingual Plane one UTF-16 unit at a time, so as a
// letter, and count JavaScript's whitespace, U+FEFF among it, as
// whitespace. Export has to open and close for those readers as well.

/// `asMicromarkReads(character)`.
fn as_micromark_reads(character: Option<u32>) -> Option<u32> {
    character.map(|c| {
        if c > 0xFFFF {
            u32::from(b'a')
        } else if matches!(c, 0x0B | 0x2028 | 0x2029 | 0xFEFF) {
            u32::from(b' ')
        } else {
            c
        }
    })
}

/// `isPortablyFlankedBy`: [`is_flanked_by`], both for CommonMark and for
/// micromark.
pub(crate) fn is_portably_flanked_by(primary: Option<u32>, secondary: Option<u32>) -> bool {
    is_flanked_by(primary, secondary)
        && is_flanked_by(as_micromark_reads(primary), as_micromark_reads(secondary))
}

/// `getPortableEmphasisSides`: [`get_emphasis_sides`], both for CommonMark
/// and for micromark.
pub(crate) fn get_portable_emphasis_sides(
    marker: u32,
    before: Option<u32>,
    after: Option<u32>,
) -> EmphasisSides {
    let sides = get_emphasis_sides(marker, before, after);
    let micromark = get_emphasis_sides(
        marker,
        as_micromark_reads(before),
        as_micromark_reads(after),
    );
    EmphasisSides {
        can_close: sides.can_close && micromark.can_close,
        can_open: sides.can_open && micromark.can_open,
    }
}

/// `isLetterForSomeReader(character)`: whether CommonMark or micromark
/// reads `character` as a letter.
pub(crate) fn is_letter_for_some_reader(character: u32) -> bool {
    !is_punctuation_or_space(Some(character))
        || !is_punctuation_or_space(as_micromark_reads(Some(character)))
}

/// `characterAt(text, index)`: the character (code point) that starts at
/// `index`, if any; half of a pair when `index` is inside one.
pub(crate) fn character_at(text: &[u16], index: isize) -> Option<u32> {
    let unit = unit_at(text, index)?;
    if (0xD800..0xDC00).contains(&unit)
        && let Some(low) = unit_at(text, index + 1).filter(|l| (0xDC00..0xE000).contains(l))
    {
        return Some(0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00));
    }
    Some(unit)
}

/// `characterBefore(text, index)`: the character (code point) that ends
/// just before `index`, if any.
pub(crate) fn character_before(text: &[u16], index: isize) -> Option<u32> {
    match character_at(text, index - 2) {
        Some(pair) if pair > 0xFFFF => Some(pair),
        _ => unit_at(text, index - 1),
    }
}

/// `toCharacterReference(character)`: `&#N;`.
pub(crate) fn to_character_reference(character: u32) -> String {
    format!("&#{character};")
}

/// `isEmptyParagraph(node)`.
pub(crate) fn is_empty_paragraph(node: &NodeRef) -> bool {
    if !node.get_latest().is::<ParagraphNode>() {
        return false;
    }
    match node.get_first_child() {
        None => true,
        Some(first) => {
            node.get_children_size() == 1
                && first.is_text()
                && re(r"^\s{0,3}$", "").test(&first.get_text_content())
        }
    }
}

/// `decodeCharacterReference(digits, hex)`: a numeric reference's
/// character, U+FFFD for U+0000, a surrogate or one past U+10FFFF.
fn decode_character_reference(digits: Option<&str>, hex: Option<&str>) -> String {
    let code_point = match (digits, hex) {
        (Some(d), _) => d.parse::<u64>().unwrap_or(u64::MAX),
        (None, Some(h)) => u64::from_str_radix(h, 16).unwrap_or(u64::MAX),
        (None, None) => 0,
    };
    if code_point == 0 || (0xD800..=0xDFFF).contains(&code_point) || code_point > 0x10FFFF {
        "\u{FFFD}".to_owned()
    } else {
        char_string(code_point as u32)
    }
}

/// `unescapeText(value)`: reads backslash escapes and numeric character
/// references.
pub(crate) fn unescape_text(value: &str) -> String {
    if !value.contains('\\') && !value.contains('&') {
        return value.to_owned();
    }
    // Do not decode an escaped ampersand or revisit a decoded reference.
    re(
        r"\\([!-/:-@[-`{-~])|&#(?:(\d{1,7})|[xX]([\da-fA-F]{1,6}));",
        "g",
    )
    .replace_with(value, |m| match m.get(1) {
        Some(escaped) => escaped.to_owned(),
        None => decode_character_reference(m.get(2), m.get(3)),
    })
}

/// `isEscaped(text, index)`: whether an odd run of backslashes comes just
/// before `index`.
pub(crate) fn is_escaped(text: &[u16], index: usize) -> bool {
    let mut backslashes = 0;
    let mut i = index;
    while i > 0 && text.get(i - 1) == Some(&u16::from(b'\\')) {
        backslashes += 1;
        i -= 1;
    }
    backslashes % 2 == 1
}

/// `getDelimiterRunEdge(text, index, step)`: where the delimiter run that
/// `text` is cut in at `index` ends, walking by `step` over the delimiter
/// beside the cut. Walking back, an escaped delimiter is not part of the
/// run.
pub(crate) fn get_delimiter_run_edge(text: &[u16], index: usize, step: isize) -> usize {
    let delimiter = if step == 1 {
        unit_at(text, index as isize - 1)
    } else {
        unit_at(text, index as isize)
    };
    let mut edge = index;
    if step == 1 {
        while edge < text.len() && Some(u32::from(text[edge])) == delimiter {
            edge += 1;
        }
    } else {
        while edge > 0
            && Some(u32::from(text[edge - 1])) == delimiter
            && !is_escaped(text, edge - 1)
        {
            edge -= 1;
        }
    }
    edge
}

/// `$isLineBreakNode(node)`.
pub(crate) fn is_line_break_node(node: &NodeRef) -> bool {
    node.get_latest().is::<LineBreakNode>()
}

/// `$isTabNode(node)`.
pub(crate) fn is_tab_node(node: &NodeRef) -> bool {
    node.get_latest().is::<TabNode>()
}

/// `$isParagraphNode(node)`.
pub(crate) fn is_paragraph_node(node: &NodeRef) -> bool {
    node.get_latest().is::<ParagraphNode>()
}

/// A text node's `getFormat()`.
pub(crate) fn text_format(node: &NodeRef) -> f64 {
    node.text_data().get_format()
}

/// `text.indexOf(search, from)`, in UTF-16 code units.
pub(crate) fn index_of(text: &[u16], search: &[u16], from: usize) -> Option<usize> {
    if search.len() > text.len() || from > text.len() - search.len() {
        return None;
    }
    (from..=text.len() - search.len()).find(|&i| &text[i..i + search.len()] == search)
}

/// `text.lastIndexOf(search)`, in UTF-16 code units.
pub(crate) fn last_index_of(text: &[u16], search: &[u16]) -> Option<usize> {
    if search.len() > text.len() {
        return None;
    }
    (0..=text.len() - search.len())
        .rev()
        .find(|&i| &text[i..i + search.len()] == search)
}

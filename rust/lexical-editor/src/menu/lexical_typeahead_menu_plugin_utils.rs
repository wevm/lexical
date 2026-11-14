//! `LexicalTypeaheadMenuPluginUtils.ts`: `PUNCTUATION` and
//! `useBasicTypeaheadTriggerMatch`, whose `RegExp` is matched by hand
//! (there is no `regex` crate in the default build), over UTF-16 code units
//! as JS's (it has no `u` flag).

use super::{MenuTextMatch, TriggerFn};
use crate::selection_deletion::is_js_whitespace;
use std::rc::Rc;

/// `PUNCTUATION`: the characters, as the body of a character class, that end
/// a typeahead query.
pub const PUNCTUATION: &str = r#"\.,\+\*\?\$\@\|#{}\(\)\^\-\[\]\\/!%'"~=<>_:;"#;

/// `useBasicTypeaheadTriggerMatch(trigger, options)`'s options.
#[derive(Clone, Debug)]
pub struct TypeaheadTriggerOptions {
    /// `minLength`: the fewest UTF-16 code units of a query (1).
    pub min_length: usize,
    /// `maxLength`: the most UTF-16 code units of a query (75).
    pub max_length: usize,
    /// `punctuation`: the body of a character class of what ends a query
    /// ([`PUNCTUATION`]).
    pub punctuation: String,
    /// `allowWhitespace`: a query may hold whitespace (false).
    pub allow_whitespace: bool,
}

impl Default for TypeaheadTriggerOptions {
    fn default() -> Self {
        TypeaheadTriggerOptions {
            min_length: 1,
            max_length: 75,
            punctuation: PUNCTUATION.to_owned(),
            allow_whitespace: false,
        }
    }
}

/// One item of a character class: a code unit range, or an escape class.
#[derive(Clone, Copy, Debug)]
enum ClassItem {
    Range(u16, u16),
    /// `\s`, `\d`, `\w`, and with `true` their negations.
    Whitespace(bool),
    Digit(bool),
    Word(bool),
}

/// A character class, `[...]` or `[^...]`, over UTF-16 code units.
#[derive(Clone, Debug)]
struct CharClass {
    negated: bool,
    items: Vec<ClassItem>,
}

fn is_word(unit: u16) -> bool {
    matches!(unit, 0x30..=0x39 | 0x41..=0x5A | 0x61..=0x7A | 0x5F)
}

fn is_whitespace_unit(unit: u16) -> bool {
    char::from_u32(u32::from(unit)).is_some_and(is_js_whitespace)
}

impl CharClass {
    /// The class whose body is `body` (what is between `[` or `[^` and
    /// `]`), as a JS `RegExp` without the `u` flag reads it: escapes,
    /// `\s`/`\d`/`\w` and their negations, and `a-b` ranges.
    fn parse(body: &str, negated: bool) -> CharClass {
        let units: Vec<u16> = body.encode_utf16().collect();
        let mut i = 0;
        // One atom: a code unit, or an escape class.
        let atom = |i: &mut usize| -> ClassItem {
            let unit = units[*i];
            *i += 1;
            if unit != u16::from(b'\\') || *i >= units.len() {
                return ClassItem::Range(unit, unit);
            }
            let escaped = units[*i];
            *i += 1;
            let hex = |i: &mut usize, n: usize| -> Option<u16> {
                let digits = units.get(*i..*i + n)?;
                let text = String::from_utf16(digits).ok()?;
                let value = u16::from_str_radix(&text, 16).ok()?;
                *i += n;
                Some(value)
            };
            let unit = match u8::try_from(escaped).map(char::from) {
                Ok('s') => return ClassItem::Whitespace(false),
                Ok('S') => return ClassItem::Whitespace(true),
                Ok('d') => return ClassItem::Digit(false),
                Ok('D') => return ClassItem::Digit(true),
                Ok('w') => return ClassItem::Word(false),
                Ok('W') => return ClassItem::Word(true),
                Ok('n') => 0x0A,
                Ok('r') => 0x0D,
                Ok('t') => 0x09,
                Ok('v') => 0x0B,
                Ok('f') => 0x0C,
                Ok('b') => 0x08,
                Ok('0') => 0,
                Ok('x') => hex(i, 2).unwrap_or(escaped),
                Ok('u') => hex(i, 4).unwrap_or(escaped),
                // An identity escape.
                _ => escaped,
            };
            ClassItem::Range(unit, unit)
        };
        let mut items = Vec::new();
        while i < units.len() {
            let first = atom(&mut i);
            // `a-b`, unless the `-` ends the class or a side is a class.
            if let ClassItem::Range(low, _) = first
                && units.get(i) == Some(&u16::from(b'-'))
                && i + 1 < units.len()
            {
                let mut j = i + 1;
                if let ClassItem::Range(high, _) = atom(&mut j) {
                    items.push(ClassItem::Range(low, high));
                    i = j;
                    continue;
                }
            }
            items.push(first);
        }
        CharClass { negated, items }
    }

    fn matches(&self, unit: u16) -> bool {
        let found = self.items.iter().any(|item| match *item {
            ClassItem::Range(low, high) => (low..=high).contains(&unit),
            ClassItem::Whitespace(not) => is_whitespace_unit(unit) != not,
            ClassItem::Digit(not) => (0x30..=0x39).contains(&unit) != not,
            ClassItem::Word(not) => is_word(unit) != not,
        });
        found != self.negated
    }
}

/// `useBasicTypeaheadTriggerMatch(trigger, options)`: a trigger function
/// for `trigger` (the body of a character class, usually one character such
/// as `@` or `/`) after whitespace, an open parenthesis or the start of the
/// text, followed by `min_length` to `max_length` code units up to the end
/// of the text that are not the trigger, punctuation or (unless
/// `allow_whitespace`) whitespace. Its `RegExp`,
/// `(^|\s|\()([trigger]((?:[^trigger punctuation \s]){0,maxLength}))$`, is
/// matched as JS's `exec` matches it.
///
/// JS's hook memoizes the function for its arguments; this builds it once.
pub fn basic_typeahead_trigger_match(trigger: &str, options: TypeaheadTriggerOptions) -> TriggerFn {
    let TypeaheadTriggerOptions {
        min_length,
        max_length,
        punctuation,
        allow_whitespace,
    } = options;
    let valid_chars_suffix = if allow_whitespace { "" } else { r"\s" };
    let trigger_class = CharClass::parse(trigger, false);
    let valid_chars =
        CharClass::parse(&format!("{trigger}{punctuation}{valid_chars_suffix}"), true);
    Rc::new(move |text, _editor| {
        let units: Vec<u16> = text.encode_utf16().collect();
        // `match[3]` for a trigger at `at`: the rest of the text, when every
        // unit of it is valid and there are at most `max_length`.
        let query_after = |at: usize| -> Option<usize> {
            if !units.get(at).is_some_and(|&u| trigger_class.matches(u)) {
                return None;
            }
            let rest = &units[at + 1..];
            (rest.len() <= max_length && rest.iter().all(|&u| valid_chars.matches(u)))
                .then_some(at + 1)
        };
        // `exec`: the first index a match starts at; at each, the
        // alternatives of group 1 in order: `^`, `\s`, `\(`.
        let found = (0..units.len()).find_map(|index| {
            if index == 0
                && let Some(query) = query_after(0)
            {
                return Some((index, 0, query));
            }
            let unit = units[index];
            if is_whitespace_unit(unit) || unit == u16::from(b'(') {
                return query_after(index + 1).map(|query| (index, 1, query));
            }
            None
        });
        let (index, lead, query) = found?;
        let matching_string = String::from_utf16_lossy(&units[query..]);
        if units.len() - query < min_length {
            return None;
        }
        Some(MenuTextMatch {
            lead_offset: index + lead,
            matching_string,
            replaceable_string: String::from_utf16_lossy(&units[index + lead..]),
        })
    })
}

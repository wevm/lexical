//! `LexicalHashtagExtension.ts`: the default matcher (`getHashtagMatch`
//! over `getHashtagRegexString()`), `HashtagConfig` and
//! `registerLexicalHashtag`.

use super::lexical_hashtag_node::create_hashtag_node;
use super::tables::{HASH_CHARS, HASHTAG_ALPHA, HASHTAG_ALPHANUMERIC, HASHTAG_BOUNDARY};
use crate::text::{EntityMatch, register_lexical_text_entity};
use crate::{Editor, Unregister};
use std::cmp::Ordering;
use std::rc::Rc;

/// Whether `unit` is in a class of `tables.rs`.
fn matches(class: &[(u16, u16)], unit: u16) -> bool {
    class
        .binary_search_by(|&(low, high)| {
            if high < unit {
                Ordering::Less
            } else if low > unit {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        })
        .is_ok()
}

/// `REGEX.exec(text)` for `getHashtagRegexString()`,
/// `(^|$|[^&/alphanumeric])([hashChars])([alphanumeric]*[alpha][alphanumeric]*)`
/// with the `i` flag and without `u`, over the UTF-16 code units of the
/// text: the first match, as its index, the length of group 1 and the
/// length of group 3.
fn exec(units: &[u16]) -> Option<(usize, usize, usize)> {
    let alphanumeric_run = |from: usize| {
        units[from..]
            .iter()
            .take_while(|&&unit| matches(HASHTAG_ALPHANUMERIC, unit))
            .count()
    };
    // Group 3 from `start`, or `None`: the first `*` takes the alphanumeric
    // run there and gives units back until `[alpha]` matches the next one;
    // the second `*` takes the run after it.
    let hashtag = |start: usize| {
        let run_end = start + alphanumeric_run(start);
        (start..=run_end)
            .rev()
            .find(|&i| i < units.len() && matches(HASHTAG_ALPHA, units[i]))
            .map(|alpha| alpha + 1 + alphanumeric_run(alpha + 1) - start)
    };
    let hash_at = |i: usize| i < units.len() && matches(HASH_CHARS, units[i]);
    for index in 0..=units.len() {
        // `^`
        if index == 0
            && hash_at(0)
            && let Some(length) = hashtag(1)
        {
            return Some((index, 0, length));
        }
        // `$` is never followed by a hash char.
        if index < units.len()
            && matches(HASHTAG_BOUNDARY, units[index])
            && hash_at(index + 1)
            && let Some(length) = hashtag(index + 2)
        {
            return Some((index, 1, length));
        }
    }
    None
}

/// `getHashtagMatch(text)`, `HashtagExtension`'s default
/// `getHashtagMatch`: the first `#hashtag` in `text`, from its `#` (or the
/// full-width `＃`), in UTF-16 code units. A hashtag is a run of
/// alphanumeric characters with at least one letter, after the start of the
/// text or a character that is neither alphanumeric nor `&` or `/`.
pub fn get_hashtag_match(text: &str) -> Option<EntityMatch> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let (index, boundary_length, hashtag_length) = exec(&units)?;

    let hashtag_length = hashtag_length + 1;
    let start_offset = index + boundary_length;
    let end_offset = start_offset + hashtag_length;
    Some(EntityMatch {
        end: end_offset,
        start: start_offset,
    })
}

/// The type of `HashtagConfig.getHashtagMatch`: `None` if no match,
/// otherwise the start and end of the first `#hashtag` match.
pub type GetHashtagMatch = Rc<dyn Fn(&str) -> Option<EntityMatch>>;

/// `HashtagConfig`.
#[derive(Clone)]
pub struct HashtagConfig {
    /// The matching function, [`get_hashtag_match`] by default: `None` if
    /// no match, otherwise the start and end of the first `#hashtag` match.
    pub get_hashtag_match: GetHashtagMatch,
}

impl Default for HashtagConfig {
    /// `defaultHashtagConfig`.
    fn default() -> Self {
        Self {
            get_hashtag_match: Rc::new(get_hashtag_match),
        }
    }
}

/// `registerLexicalHashtag(editor, config)`: `registerLexicalTextEntity`
/// with the config's matcher, making a `HashtagNode` of each match. Returns
/// what unregisters it.
pub fn register_lexical_hashtag(editor: &Editor, config: HashtagConfig) -> Unregister {
    let get_hashtag_match = config.get_hashtag_match;
    let unregisters = register_lexical_text_entity(
        editor,
        move |text| get_hashtag_match(text),
        "hashtag",
        |text_node| create_hashtag_node(&text_node.get_text_content()),
    );
    // `mergeRegister`
    Box::new(move || {
        for unregister in unregisters.into_iter().rev() {
            unregister();
        }
    })
}

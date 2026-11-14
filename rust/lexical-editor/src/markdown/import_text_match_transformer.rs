//! `importTextMatchTransformer.ts`: the outermost text match (a link, or a
//! custom transformer's) in a text, and importing it.

use super::import_text_format_transformer::{FormatMatch, scan_code_spans};
use super::markdown_transformers::{
    LINK_IMPORT_REG_EXP, TextMatchTransformer, get_link_label_opener,
};
use super::reg_exp::{RegExp, RegExpMatch};
use super::utils::{index_of, is_escaped, last_index_of, units};
use crate::{LexicalError, NodeRef};
use std::rc::Rc;

/// `moveMatch(match, text, distance)`: a copy of `match` for `text`, what
/// is left of the text it was found in after `distance` code units.
fn move_match(m: &RegExpMatch, text: &str, distance: usize) -> RegExpMatch {
    let mut moved = m.clone();
    moved.index = m.index - distance;
    moved.input = text.to_owned();
    moved.ranges = m
        .ranges
        .iter()
        .map(|r| r.map(|(s, e)| (s - distance, e - distance)))
        .collect();
    moved
}

/// `CodeSpans`: code spans in order, as (start, end).
pub(crate) type CodeSpans<'a> = &'a [FormatMatch];

/// `matchLink(text, regExp, codeSpans, offset)`: the first link in `text`,
/// starting at the `[` that really opens its text, with none of its syntax
/// inside a code span.
fn match_link(
    text: &str,
    reg_exp: &RegExp,
    code_spans: Option<CodeSpans<'_>>,
    offset: usize,
) -> Option<RegExpMatch> {
    let flags: String = reg_exp
        .flags()
        .chars()
        .filter(|f| *f != 'g' && *f != 'y')
        .chain(std::iter::once('g'))
        .collect();
    let search = RegExp::of(reg_exp.source(), &flags);
    let u = units(text);
    // Code spans bind tighter than link brackets.
    let own_spans: Vec<(usize, usize)>;
    let (spans, span_offset): (Vec<(usize, usize)>, usize) = match code_spans {
        Some(spans) => (
            spans.iter().map(|s| (s.start_index, s.end_index)).collect(),
            offset,
        ),
        None => {
            own_spans = if text.contains('`') {
                scan_code_spans(&u)
                    .iter()
                    .map(|s| (s.start_index, s.end_index))
                    .collect()
            } else {
                Vec::new()
            };
            (own_spans, 0)
        }
    };
    let code_span_at = |index: usize| -> Option<usize> {
        let at = index + span_offset;
        let (mut low, mut high) = (0isize, spans.len() as isize - 1);
        while low <= high {
            let middle = ((low + high) >> 1) as usize;
            let (start, end) = spans[middle];
            if at < start {
                high = middle as isize - 1;
            } else if at >= end {
                low = middle as isize + 1;
            } else {
                return Some(middle);
            }
        }
        None
    };
    // No link starts after the last `](`.
    let last_closer = last_index_of(&u, &units("](")).map_or(-1, |i| i as isize);
    let open = u16::from(b'[');
    while (search.last_index() as isize) < last_closer {
        let m = search.exec(text)?;
        if let Some(opener_span) = code_span_at(m.index) {
            // No link can open inside this code span.
            search.set_last_index(spans[opener_span].1 - span_offset);
            continue;
        }
        let label_end = m.index + 1 + crate::utf16::len(m.str(1));
        if code_span_at(label_end).is_some() || code_span_at(m.index + m.match_len() - 1).is_some()
        {
            search.set_last_index(label_end - 1);
            continue;
        }
        // A `[` the label holds outside a code span is the link's real
        // opener: search on from there.
        let opener = get_link_label_opener(m.str(1));
        if let Some(o) = opener.filter(|o| *o >= 0) {
            search.set_last_index(m.index + 1 + o as usize);
            continue;
        }
        if !is_escaped(&u, m.index) {
            return Some(m);
        }
        // Each escaped `[` up to the next unescaped one finds what the last
        // of them finds.
        let mut next = m.index;
        loop {
            match index_of(&u, &[open], next + 1) {
                Some(n) if is_escaped(&u, n) => next = n,
                Some(n) => {
                    next = n;
                    break;
                }
                None => return None,
            }
        }
        let last = u[..next].iter().rposition(|c| *c == open);
        search.set_last_index(match last {
            Some(l) if l > m.index => l,
            _ => next,
        });
    }
    None
}

/// `looksOnlyAhead(regExp)`: whether a match depends only on the text from
/// where it starts: no `^`, word boundary or lookbehind outside a class,
/// and no flag that keeps state.
fn looks_only_ahead(reg_exp: &RegExp) -> bool {
    if reg_exp.flags().contains(['g', 'y', 'v']) {
        return false;
    }
    let source: Vec<char> = reg_exp.source().chars().collect();
    let mut in_class = false;
    let mut i = 0;
    while i < source.len() {
        let c = source[i];
        if c == '\\' {
            i += 1;
            if !in_class && matches!(source.get(i), Some('b' | 'B')) {
                return false;
            }
        } else if in_class {
            in_class = c != ']';
        } else if c == '[' {
            in_class = true;
        } else {
            let rest: String = source[i..source.len().min(i + 4)].iter().collect();
            let is_lookbehind_or_named = rest.starts_with("(?<");
            let is_named =
                is_lookbehind_or_named && rest.chars().nth(3).is_some_and(|c| c != '=' && c != '!');
            if (c == '^' || is_lookbehind_or_named) && !is_named {
                return false;
            }
        }
        i += 1;
    }
    true
}

/// `TextMatches`: the match each text match transformer found in a text and
/// the offset of the text it was found in, or `None` when it found none.
#[derive(Default)]
pub(crate) struct TextMatches(Vec<(Rc<TextMatchTransformer>, Option<FoundMatch>)>);

/// A match and the offset of the text it was found in.
type FoundMatch = (RegExpMatch, usize);

impl TextMatches {
    fn get(&self, t: &Rc<TextMatchTransformer>) -> Option<&Option<(RegExpMatch, usize)>> {
        self.0
            .iter()
            .find(|(k, _)| Rc::ptr_eq(k, t))
            .map(|(_, v)| v)
    }

    fn set(&mut self, t: &Rc<TextMatchTransformer>, value: Option<(RegExpMatch, usize)>) {
        match self.0.iter_mut().find(|(k, _)| Rc::ptr_eq(k, t)) {
            Some((_, v)) => *v = value,
            None => self.0.push((t.clone(), value)),
        }
    }
}

/// What `findOutermostTextMatchTransformer` returns.
pub(crate) struct FoundTextMatch {
    pub start_index: usize,
    pub end_index: usize,
    pub transformer: Rc<TextMatchTransformer>,
    pub m: RegExpMatch,
}

/// `findOutermostTextMatchTransformer(textNode, textMatchTransformers,
/// matches, offset, codeSpans)`.
pub(crate) fn find_outermost_text_match_transformer(
    text_node: &NodeRef,
    text_match_transformers: &[Rc<TextMatchTransformer>],
    matches: &mut TextMatches,
    offset: usize,
    code_spans: Option<CodeSpans<'_>>,
) -> Result<Option<FoundTextMatch>, LexicalError> {
    let mut found: Option<FoundTextMatch> = None;
    for transformer in text_match_transformers {
        let (Some(_), Some(import_reg_exp)) = (&transformer.replace, &transformer.import_reg_exp)
        else {
            continue;
        };
        let text = text_node.get_text_content();
        // Transformers sharing the built-in link syntax need the same
        // candidate rules.
        let is_link = import_reg_exp.source() == LINK_IMPORT_REG_EXP;
        let m = if is_link || looks_only_ahead(import_reg_exp) {
            match matches.get(transformer) {
                Some(Some((m, found_offset))) if found_offset + m.index >= offset => {
                    Some(move_match(m, &text, offset - found_offset))
                }
                Some(None) => None,
                _ => {
                    let result = if is_link {
                        match_link(&text, import_reg_exp, code_spans, offset)
                    } else {
                        import_reg_exp.exec(&text)
                    };
                    matches.set(transformer, result.clone().map(|m| (m, offset)));
                    result
                }
            }
        } else {
            import_reg_exp.string_match(&text)
        };
        let Some(m) = m else {
            continue;
        };
        let start_index = m.index;
        let end_index = match &transformer.get_end_index {
            Some(get_end_index) => match get_end_index(text_node, &m)? {
                Some(end) => end,
                None => continue,
            },
            None => start_index + m.match_len(),
        };
        let replaces = match &found {
            None => true,
            // Wraps the previous match or is strictly before it.
            Some(f) => {
                start_index < f.start_index
                    && (end_index > f.end_index || end_index <= f.start_index)
            }
        };
        if replaces {
            found = Some(FoundTextMatch {
                start_index,
                end_index,
                transformer: transformer.clone(),
                m,
            });
        }
    }
    Ok(found)
}

/// What `importFoundTextMatchTransformer` returns.
pub(crate) struct ImportedTextMatch {
    pub matched_node: NodeRef,
    pub transformed_node: Option<NodeRef>,
    pub node_before: Option<NodeRef>,
    pub node_after: Option<NodeRef>,
    /// The text of `node_after` before the transformer ran.
    pub text_after: Option<String>,
}

/// `importFoundTextMatchTransformer(textNode, startIndex, endIndex,
/// transformer, match)`.
pub(crate) fn import_found_text_match_transformer(
    text_node: &NodeRef,
    start_index: usize,
    end_index: usize,
    transformer: &TextMatchTransformer,
    m: &RegExpMatch,
) -> Result<Option<ImportedTextMatch>, LexicalError> {
    let (transformed_node, node_before, node_after);
    if start_index == 0 {
        let mut parts = text_node.split_text(&[end_index as i64])?.into_iter();
        transformed_node = parts.next().expect("a part");
        node_after = parts.next();
        node_before = None;
    } else {
        let mut parts = text_node
            .split_text(&[start_index as i64, end_index as i64])?
            .into_iter();
        node_before = parts.next();
        transformed_node = parts.next().expect("a part");
        node_after = parts.next();
    }
    let Some(replace) = &transformer.replace else {
        return Ok(None);
    };
    let text_after = node_after.as_ref().map(NodeRef::get_text_content);
    let potential = replace(&transformed_node, m)?;
    Ok(Some(ImportedTextMatch {
        matched_node: transformed_node,
        node_after,
        node_before,
        text_after,
        transformed_node: potential,
    }))
}

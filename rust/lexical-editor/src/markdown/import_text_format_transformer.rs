//! `importTextFormatTransformer.ts`: code spans and CommonMark emphasis
//! (delimiter runs, flanking, the rule of 3), found once per text and read
//! again for what is left of it after each match. Offsets are UTF-16 code
//! units.

use super::markdown_import::TextFormatTransformersIndex;
use super::markdown_transformers::TextFormatTransformer;
use super::reg_exp::RegExpMatch;
use super::utils::{
    character_at, character_before, get_delimiter_run_edge, get_emphasis_sides, is_escaped, string,
    unit_at,
};
use crate::{LexicalError, NodeRef};
use std::rc::Rc;

const BACKTICK: u16 = b'`' as u16;

#[derive(Clone, Debug)]
struct Delimiter {
    index: usize,
    char: u16,
    length: usize,
    can_open: bool,
    can_close: bool,
    active: bool,
}

/// `FormatMatch`: a code span or emphasis, from `start_index` to
/// `end_index`.
#[derive(Clone, Debug)]
pub(crate) struct FormatMatch {
    pub start_index: usize,
    pub end_index: usize,
    pub tag: String,
    pub content: String,
}

/// `FormatScan`: the code spans and outermost emphasis of a text, read
/// again for what is left of it after each match, at `offset`.
#[derive(Clone, Debug)]
pub(crate) struct FormatScan {
    pub text: Vec<u16>,
    /// The characters that start an emphasis tag.
    pub delimiters: Vec<u16>,
    pub code_spans: Vec<FormatMatch>,
    pub emphasis: Vec<FormatMatch>,
    pub offset: usize,
    pub next_code_span: usize,
    pub next_emphasis: usize,
    /// An emphasis that starts at `offset`, found after the scan.
    pub pending: Option<FormatMatch>,
}

impl FormatScan {
    pub(crate) fn has_delimiter(&self, c: Option<u32>) -> bool {
        c.is_some_and(|c| self.delimiters.iter().any(|d| u32::from(*d) == c))
    }
}

/// `scanTextFormats(text, textFormatTransformersIndex, before, after)`.
pub(crate) fn scan_text_formats(
    text: &[u16],
    index: &TextFormatTransformersIndex,
    before: Option<u32>,
    after: Option<u32>,
) -> FormatScan {
    // Code spans first: emphasis delimiters inside them are not read.
    let code_spans: Vec<FormatMatch> = if index.transformer("`").is_some() {
        scan_code_spans(text)
            .into_iter()
            .map(|span| FormatMatch {
                start_index: span.start_index,
                end_index: span.end_index,
                tag: "`".to_owned(),
                content: span.content,
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut delimiter_chars: Vec<u16> = Vec::new();
    for (tag, _) in &index.transformers_by_tag {
        let first = tag.encode_utf16().next();
        if let Some(first) = first.filter(|f| *f != BACKTICK)
            && !delimiter_chars.contains(&first)
        {
            delimiter_chars.push(first);
        }
    }
    let mut delimiters = scan_delimiters(text, &delimiter_chars, &code_spans, before, after);
    let emphasis = if delimiters.is_empty() {
        Vec::new()
    } else {
        process_emphasis(text, &mut delimiters, index)
    };
    FormatScan {
        text: text.to_vec(),
        delimiters: delimiter_chars,
        code_spans,
        emphasis,
        offset: 0,
        next_code_span: 0,
        next_emphasis: 0,
        pending: None,
    }
}

/// `advanceFormatScan(scan, offset, isFormatEnd)`: moves the scan on to the
/// text after `offset`, where a match ends; false when what it found does
/// not hold for that text on its own.
pub(crate) fn advance_format_scan(
    scan: &mut FormatScan,
    offset: usize,
    is_format_end: bool,
) -> bool {
    scan.offset = offset;
    let pending = scan.pending.take();
    if pending.as_ref().is_some_and(|p| p.end_index > offset) {
        return false;
    }
    while scan.next_code_span < scan.code_spans.len()
        && scan.code_spans[scan.next_code_span].start_index < offset
    {
        let span = &scan.code_spans[scan.next_code_span];
        scan.next_code_span += 1;
        if span.end_index > offset {
            return false;
        }
    }
    while scan.next_emphasis < scan.emphasis.len()
        && scan.emphasis[scan.next_emphasis].start_index < offset
    {
        let e = &scan.emphasis[scan.next_emphasis];
        scan.next_emphasis += 1;
        if e.end_index > offset {
            return false;
        }
    }
    let cut = unit_at(&scan.text, offset as isize - 1);
    let next = unit_at(&scan.text, offset as isize);
    cut != Some(u32::from(b'\\'))
        && ((is_format_end && pending.is_none())
            || cut != next
            || (cut != Some(u32::from(BACKTICK)) && !scan.has_delimiter(cut)))
}

/// `findPairAtEdge(scan, rest, textFormatTransformersIndex)`: lets the
/// scan read the delimiter run that starts `rest` beside the edge of a
/// line; false when the rest has to be scanned again.
pub(crate) fn find_pair_at_edge(
    scan: &mut FormatScan,
    rest: &[u16],
    index: &TextFormatTransformersIndex,
) -> bool {
    let offset = scan.offset;
    let mut span = scan.next_code_span;
    let next = scan.emphasis.get(scan.next_emphasis);
    let end = if span < scan.code_spans.len()
        && next.is_none_or(|n| scan.code_spans[span].start_index < n.start_index)
    {
        scan.code_spans[span].start_index - offset
    } else {
        rest.len()
    };
    let mut length = 64;
    loop {
        // Cut neither a code span nor a run of a delimiter or backtick.
        while span < scan.code_spans.len() && scan.code_spans[span].end_index <= offset + length {
            span += 1;
        }
        if span < scan.code_spans.len() && scan.code_spans[span].start_index < offset + length {
            length = scan.code_spans[span].end_index - offset;
        }
        let last = unit_at(rest, length as isize - 1);
        if length < end && (last == Some(u32::from(BACKTICK)) || scan.has_delimiter(last)) {
            length = get_delimiter_run_edge(rest, length, 1);
        }
        if length >= end {
            if end == rest.len() {
                return false;
            }
            length = end;
        }
        let pair = scan_text_formats(
            &rest[..length],
            index,
            None,
            character_at(rest, length as isize),
        )
        .emphasis
        .into_iter()
        .next();
        if let Some(pair) = pair.filter(|p| p.start_index == 0) {
            scan.pending = Some(FormatMatch {
                end_index: pair.end_index + offset,
                start_index: offset,
                ..pair
            });
            return true;
        }
        if length == end {
            return true;
        }
        length *= 2;
    }
}

/// `partlyHoldsCodeSpan(scan, startIndex, endIndex)`.
pub(crate) fn partly_holds_code_span(
    scan: &FormatScan,
    start_index: usize,
    end_index: usize,
) -> bool {
    let offset = scan.offset as isize;
    for span in &scan.code_spans[scan.next_code_span.min(scan.code_spans.len())..] {
        let span_start = span.start_index as isize - offset;
        let span_end = span.end_index as isize - offset;
        if span_start >= end_index as isize {
            break;
        }
        if span_end > start_index as isize
            && (span_start < start_index as isize || span_end > end_index as isize)
        {
            return true;
        }
    }
    false
}

/// What `findOutermostTextFormatTransformer` returns.
pub(crate) struct FoundTextFormat {
    pub start_index: usize,
    pub end_index: usize,
    pub transformer: Rc<TextFormatTransformer>,
    pub m: RegExpMatch,
    /// Whether the outermost format is a code span, whose content is never
    /// read again.
    pub is_code_span: bool,
}

/// `findOutermostTextFormatTransformer(text, index, scan)`: the outermost
/// format match in `text`, what is left after the scan's offset of the
/// text it scanned.
pub(crate) fn find_outermost_text_format_transformer(
    text: &[u16],
    index: &TextFormatTransformersIndex,
    scan: &FormatScan,
) -> Option<FoundTextFormat> {
    let code_match = scan.code_spans.get(scan.next_code_span);
    let emphasis_match = scan
        .pending
        .as_ref()
        .or_else(|| scan.emphasis.get(scan.next_emphasis));
    let (result, is_code_span) = match (emphasis_match, code_match) {
        (Some(e), Some(c)) if e.start_index <= c.start_index => (e, false),
        (Some(e), None) => (e, false),
        (_, Some(c)) => (c, true),
        (None, None) => return None,
    };
    let transformer = index.transformer(&result.tag)?;
    let start_index = result.start_index - scan.offset;
    let end_index = result.end_index - scan.offset;
    let m = RegExpMatch::new(
        vec![
            Some(string(&text[start_index..end_index])),
            Some(result.tag.clone()),
            Some(result.content.clone()),
        ],
        start_index,
        &string(text),
    );
    Some(FoundTextFormat {
        start_index,
        end_index,
        transformer,
        m,
        is_code_span,
    })
}

/// A code span that `scanCodeSpans` finds.
#[derive(Clone, Debug)]
pub(crate) struct CodeSpan {
    pub start_index: usize,
    pub end_index: usize,
    pub content: String,
}

/// `scanCodeSpans(text)`: every inline code span, left to right and
/// non-overlapping. A run opens a span and the next run of equal length
/// closes it; an escaped backtick is literal and the rest of its run may
/// open one.
pub(crate) fn scan_code_spans(text: &[u16]) -> Vec<CodeSpan> {
    // Maximal backtick runs, as (index, length).
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < text.len() {
        if text[i] == BACKTICK {
            let mut length = 1;
            while i + length < text.len() && text[i + length] == BACKTICK {
                length += 1;
            }
            runs.push((i, length));
            i += length;
        } else {
            i += 1;
        }
    }
    // The runs of each length, and how many of them the openers passed.
    let mut runs_by_length: Vec<(usize, Vec<usize>, usize)> = Vec::new();
    for (index, &(_, length)) in runs.iter().enumerate() {
        match runs_by_length.iter_mut().find(|(l, _, _)| *l == length) {
            Some((_, indexes, _)) => indexes.push(index),
            None => runs_by_length.push((length, vec![index], 0)),
        }
    }
    let mut find_closer = |index: usize, length: usize| -> Option<usize> {
        let (_, indexes, passed) = runs_by_length.iter_mut().find(|(l, _, _)| *l == length)?;
        let mut next = *passed;
        while next < indexes.len() && indexes[next] <= index {
            next += 1;
        }
        *passed = next;
        indexes.get(next).copied()
    };
    let mut spans = Vec::new();
    let mut open_idx = 0;
    while open_idx < runs.len() {
        let mut opener = runs[open_idx];
        if is_escaped(text, opener.0) {
            if opener.1 == 1 {
                open_idx += 1;
                continue;
            }
            opener = (opener.0 + 1, opener.1 - 1);
        }
        let Some(close_idx) = find_closer(open_idx, opener.1) else {
            open_idx += 1;
            continue;
        };
        let closer = runs[close_idx];
        let mut content = &text[opener.0 + opener.1..closer.0];
        let space = u16::from(b' ');
        if content.len() >= 2
            && content[0] == space
            && content[content.len() - 1] == space
            && content.iter().any(|c| *c != space)
        {
            content = &content[1..content.len() - 1];
        }
        spans.push(CodeSpan {
            content: string(content),
            end_index: closer.0 + closer.1,
            start_index: opener.0,
        });
        open_idx = close_idx + 1;
    }
    spans
}

/// `scanDelimiters(text, delimiterChars, codeSpans, before, after)`.
fn scan_delimiters(
    text: &[u16],
    delimiter_chars: &[u16],
    code_spans: &[FormatMatch],
    before: Option<u32>,
    after: Option<u32>,
) -> Vec<Delimiter> {
    let mut delimiters = Vec::new();
    let mut span = 0;
    let mut is_in_code_span = |index: usize| -> bool {
        while span < code_spans.len() && code_spans[span].end_index <= index {
            span += 1;
        }
        span < code_spans.len() && index >= code_spans[span].start_index
    };
    let mut i = 0;
    while i < text.len() {
        let c = text[i];
        if !delimiter_chars.contains(&c) || is_escaped(text, i) || is_in_code_span(i) {
            i += 1;
            continue;
        }
        let len = get_delimiter_run_edge(text, i + 1, 1) - i;
        let sides = get_emphasis_sides(
            u32::from(c),
            if i == 0 {
                before
            } else {
                character_before(text, i as isize)
            },
            if i + len == text.len() {
                after
            } else {
                character_at(text, (i + len) as isize)
            },
        );
        if sides.can_open || sides.can_close {
            delimiters.push(Delimiter {
                active: true,
                can_close: sides.can_close,
                can_open: sides.can_open,
                char: c,
                index: i,
                length: len,
            });
        }
        i += len;
    }
    delimiters
}

/// `processEmphasis(text, delimiters, transformersIndex)`: the outermost
/// emphasis matches, in order.
fn process_emphasis(
    text: &[u16],
    delimiters: &mut [Delimiter],
    index: &TextFormatTransformersIndex,
) -> Vec<FormatMatch> {
    // Keyed by marker, whether the closer can open, and its length mod 3.
    let mut openers_bottom: Vec<((u16, bool, usize), isize)> = Vec::new();
    let mut current_pos = 0;
    let mut matches: Vec<FormatMatch> = Vec::new();
    while current_pos < delimiters.len() {
        let closer = delimiters[current_pos].clone();
        if !closer.active || !closer.can_close || closer.length == 0 {
            current_pos += 1;
            continue;
        }
        let bottom_key = (closer.char, closer.can_open, closer.length % 3);
        let bottom = openers_bottom
            .iter()
            .find(|(k, _)| *k == bottom_key)
            .map_or(-1, |(_, b)| *b);
        let mut found_opener = false;
        let mut open_idx = current_pos as isize - 1;
        while open_idx > bottom {
            let oi = open_idx as usize;
            open_idx -= 1;
            let opener = delimiters[oi].clone();
            if !opener.active
                || !opener.can_open
                || opener.length == 0
                || opener.char != closer.char
            {
                continue;
            }
            // Rule of 3, on the remaining lengths.
            if opener.can_close || closer.can_open {
                let sum = opener.length + closer.length;
                if sum.is_multiple_of(3)
                    && !opener.length.is_multiple_of(3)
                    && !closer.length.is_multiple_of(3)
                {
                    continue;
                }
            }
            let max_len = opener.length.min(closer.length);
            // The longest tag of this marker that fits, the first of equal
            // ones (a stable sort).
            let mut matched_tag: Option<&String> = None;
            for (tag, _) in &index.transformers_by_tag {
                let units: Vec<u16> = tag.encode_utf16().collect();
                if units.first() == Some(&opener.char)
                    && units.len() <= max_len
                    && matched_tag.is_none_or(|m| units.len() > m.encode_utf16().count())
                {
                    matched_tag = Some(tag);
                }
            }
            let Some(matched_tag) = matched_tag else {
                continue;
            };
            found_opener = true;
            let match_len = matched_tag.encode_utf16().count();
            matches.push(FormatMatch {
                content: string(&text[opener.index + opener.length..closer.index]),
                end_index: closer.index + match_len,
                start_index: opener.index + (opener.length - match_len),
                tag: matched_tag.clone(),
            });
            for d in &mut delimiters[oi + 1..current_pos] {
                d.active = false;
            }
            let o = &mut delimiters[oi];
            o.length -= match_len;
            o.active = o.length > 0;
            let c = &mut delimiters[current_pos];
            c.length -= match_len;
            if c.length > 0 {
                c.index += match_len;
            } else {
                c.active = false;
                current_pos += 1;
            }
            break;
        }
        if !found_opener {
            let value = current_pos as isize - 1;
            match openers_bottom.iter_mut().find(|(k, _)| *k == bottom_key) {
                Some((_, b)) => *b = value,
                None => openers_bottom.push((bottom_key, value)),
            }
            if !closer.can_open {
                delimiters[current_pos].active = false;
            }
            current_pos += 1;
        }
    }
    // Pairs nest, so the outermost ones are those no earlier one holds.
    matches.sort_by(|a, b| {
        a.start_index
            .cmp(&b.start_index)
            .then(b.end_index.cmp(&a.end_index))
    });
    let mut outermost: Vec<FormatMatch> = Vec::new();
    for m in matches {
        if outermost
            .last()
            .is_none_or(|last| m.start_index >= last.end_index)
        {
            outermost.push(m);
        }
    }
    outermost
}

/// What `importTextFormatTransformer` returns.
pub(crate) struct ImportedFormat {
    pub transformed_node: NodeRef,
    pub node_before: Option<NodeRef>,
    pub node_after: Option<NodeRef>,
}

/// `importTextFormatTransformer(textNode, startIndex, endIndex,
/// transformer, match)`.
pub(crate) fn import_text_format_transformer(
    text_node: &NodeRef,
    start_index: usize,
    end_index: usize,
    transformer: &TextFormatTransformer,
    m: &RegExpMatch,
) -> Result<ImportedFormat, LexicalError> {
    let text_content = text_node.get_text_content();
    let (transformed_node, node_before, node_after);
    // Matching the whole content reuses the node.
    if m.text() == text_content {
        transformed_node = text_node.clone();
        node_before = None;
        node_after = None;
    } else if start_index == 0 {
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
    transformed_node.set_text_content(m.str(2));
    for format in &transformer.format {
        if !transformed_node.has_format(format) {
            transformed_node.toggle_format(format);
        }
    }
    Ok(ImportedFormat {
        transformed_node,
        node_before,
        node_after,
    })
}

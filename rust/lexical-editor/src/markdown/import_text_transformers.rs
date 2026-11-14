//! `importTextTransformers.ts`: applies the text format and text match
//! transformers to a line's text, outermost match first.

use super::import_text_format_transformer::{
    FormatScan, advance_format_scan, find_outermost_text_format_transformer, find_pair_at_edge,
    import_text_format_transformer, partly_holds_code_span, scan_text_formats,
};
use super::import_text_match_transformer::{
    TextMatches, find_outermost_text_match_transformer, import_found_text_match_transformer,
};
use super::markdown_import::TextFormatTransformersIndex;
use super::markdown_transformers::TextMatchTransformer;
use super::utils::{
    character_at, character_before, get_delimiter_run_edge, get_emphasis_sides, unescape_text,
    unit_at, units,
};
use crate::{LexicalError, NodeRef};
use std::rc::Rc;

/// `canContainTransformableMarkdown(node)`: a text node without the code
/// format.
pub(crate) fn can_contain_transformable_markdown(node: Option<&NodeRef>) -> bool {
    node.is_some_and(|n| n.is_text() && !n.has_format("code"))
}

/// `readsOtherwiseBesideEdge(text, before, outside)`.
fn reads_otherwise_beside_edge(text: &[u16], before: Option<u32>, outside: Option<u32>) -> bool {
    let edge = get_delimiter_run_edge(text, 1, 1);
    let after = if edge < text.len() {
        character_at(text, edge as isize)
    } else {
        outside
    };
    let marker = u32::from(text[0]);
    let at_edge = get_emphasis_sides(marker, None, after);
    let beside_before = get_emphasis_sides(marker, before, after);
    at_edge != beside_before
}

/// `SPEC_MARKERS`: the markers whose runs CommonMark and GFM define.
const SPEC_MARKERS: &[u16] = &[b'*' as u16, b'_' as u16, b'~' as u16];

/// `characterBeyondRun(text, index, step, outside, delimiters,
/// isBesideTextMatch)`.
fn character_beyond_run(
    text: &[u16],
    index: usize,
    step: isize,
    outside: Option<u32>,
    scan: &FormatScan,
    is_beside_text_match: bool,
) -> Option<u32> {
    let beside = if step == 1 {
        unit_at(text, index as isize - 1)
    } else {
        unit_at(text, index as isize)
    };
    let edge = if beside == Some(u32::from(b'`')) || scan.has_delimiter(beside) {
        get_delimiter_run_edge(text, index, step)
    } else {
        index
    };
    if is_beside_text_match && edge == index {
        return None;
    }
    if step == 1 {
        if edge < text.len() {
            character_at(text, edge as isize)
        } else {
            outside
        }
    } else if edge > 0 {
        character_before(text, edge as isize)
    } else {
        outside
    }
}

/// `TextPart`: text to import, with the characters around it.
struct TextPart {
    node: Option<NodeRef>,
    before: Option<u32>,
    after: Option<u32>,
}

/// `Deferred`.
struct Deferred {
    outside: TextPart,
    inside: TextPart,
    matched: Option<NodeRef>,
}

/// `TextScan`: the formats and text matches found in a text, for what is
/// left of it, `rest`.
struct TextScan {
    formats: FormatScan,
    matches: TextMatches,
    rest: String,
}

/// `TextRest`.
struct TextRest {
    node: NodeRef,
    before: Option<u32>,
    after: Option<u32>,
    scan: Option<TextScan>,
}

/// `importTextTransformers(textNode, textFormatTransformersIndex,
/// textMatchTransformers, before, after)`: finds the outermost text format
/// or text match and applies it, goes on with the text after it, then,
/// from the last match back to the first, imports the text before and
/// inside each.
pub(crate) fn import_text_transformers(
    text_node: &NodeRef,
    index: &TextFormatTransformersIndex,
    text_match_transformers: &[Rc<TextMatchTransformer>],
    before: Option<u32>,
    after: Option<u32>,
) -> Result<(), LexicalError> {
    let mut deferred: Vec<Deferred> = Vec::new();
    let mut rest = Some(TextRest {
        after,
        before,
        node: text_node.clone(),
        scan: None,
    });
    while let Some(r) = rest {
        rest = import_outermost(r, index, text_match_transformers, &mut deferred)?;
    }
    for d in deferred.into_iter().rev() {
        import_text_part(&d.outside, index, text_match_transformers)?;
        import_text_part(&d.inside, index, text_match_transformers)?;
        if let Some(matched) = d.matched.as_ref()
            && !can_contain_transformable_markdown(d.inside.node.as_ref())
            && can_contain_transformable_markdown(Some(matched))
            && matched.get_parent().is_some()
        {
            // A text match that returned no node: read its escapes once.
            matched.set_text_content(&unescape_text(&matched.get_text_content()));
        }
    }
    Ok(())
}

fn import_text_part(
    part: &TextPart,
    index: &TextFormatTransformersIndex,
    text_match_transformers: &[Rc<TextMatchTransformer>],
) -> Result<(), LexicalError> {
    if let Some(node) = part.node.as_ref()
        && can_contain_transformable_markdown(Some(node))
    {
        import_text_transformers(
            node,
            index,
            text_match_transformers,
            part.before,
            part.after,
        )?;
    }
    Ok(())
}

/// `importOutermost(rest, ...)`: applies the outermost transformer, defers
/// the import of the text before and inside its match, and returns the
/// text after it.
fn import_outermost(
    rest_in: TextRest,
    index: &TextFormatTransformersIndex,
    text_match_transformers: &[Rc<TextMatchTransformer>],
    deferred: &mut Vec<Deferred>,
) -> Result<Option<TextRest>, LexicalError> {
    let TextRest {
        node: text_node,
        before,
        after,
        scan: previous_scan,
    } = rest_in;
    let text_string = text_node.get_text_content();
    let text = units(&text_string);
    let mut scan = match previous_scan {
        Some(s) if s.rest == text_string => s,
        _ => TextScan {
            formats: scan_text_formats(&text, index, before, after),
            matches: TextMatches::default(),
            rest: text_string.clone(),
        },
    };
    let mut found_text_format = find_outermost_text_format_transformer(&text, index, &scan.formats);
    let code_spans = if index.transformer("`").is_some() {
        Some(scan.formats.code_spans.clone())
    } else {
        None
    };
    let mut found_text_match = find_outermost_text_match_transformer(
        &text_node,
        text_match_transformers,
        &mut scan.matches,
        scan.formats.offset,
        code_spans.as_deref(),
    )?;

    if let (Some(format), Some(text_match)) = (&found_text_format, &found_text_match) {
        // Code spans bind tighter than text matches: a match goes only when
        // it wraps the whole span.
        let (start_index, end_index) = (text_match.start_index, text_match.end_index);
        if partly_holds_code_span(&scan.formats, start_index, end_index) {
            found_text_match = None;
        } else if format.is_code_span {
            if (start_index <= format.start_index && end_index >= format.end_index)
                || end_index <= format.start_index
            {
                found_text_format = None;
            } else {
                found_text_match = None;
            }
        } else if (format.start_index <= start_index && format.end_index >= end_index)
            || start_index > format.end_index
        {
            found_text_match = None;
        } else {
            found_text_format = None;
        }
    }

    // `importAround`.
    let import_around = |mut scan: TextScan,
                         deferred: &mut Vec<Deferred>,
                         node_before: Option<NodeRef>,
                         node_after: Option<NodeRef>,
                         text_after: Option<Option<String>>,
                         start_index: usize,
                         end_index: usize,
                         inside: TextPart,
                         matched: Option<NodeRef>|
     -> Option<TextRest> {
        let is_text_match = text_after.is_some();
        deferred.push(Deferred {
            inside,
            matched,
            outside: TextPart {
                after: character_beyond_run(
                    &text,
                    start_index,
                    1,
                    after,
                    &scan.formats,
                    is_text_match,
                ),
                before,
                node: node_before,
            },
        });
        let node_after = node_after.filter(|n| can_contain_transformable_markdown(Some(n)))?;
        let rest = node_after.get_text_content();
        if let Some(text_after) = &text_after
            && text_after.as_deref() != Some(rest.as_str())
        {
            return Some(TextRest {
                after: None,
                before: None,
                node: node_after,
                scan: None,
            });
        }
        let rest_before =
            character_beyond_run(&text, end_index, -1, before, &scan.formats, is_text_match);
        // The scan read a run at the start of the rest beside the end of
        // the match; read beside a line's edge it can only change the pair
        // it opens.
        let offset = scan.formats.offset + end_index;
        let mut is_reusable = advance_format_scan(&mut scan.formats, offset, !is_text_match);
        let rest_units = units(&rest);
        if is_reusable
            && is_text_match
            && rest_before.is_none()
            && scan
                .formats
                .has_delimiter(rest_units.first().map(|u| u32::from(*u)))
            && reads_otherwise_beside_edge(
                &rest_units,
                character_before(&text, end_index as isize),
                after,
            )
        {
            is_reusable = find_pair_at_edge(&mut scan.formats, &rest_units, index);
        }
        scan.rest = rest;
        Some(TextRest {
            after,
            before: rest_before,
            node: node_after,
            scan: is_reusable.then_some(scan),
        })
    };

    if let Some(format) = found_text_format {
        let result = import_text_format_transformer(
            &text_node,
            format.start_index,
            format.end_index,
            &format.transformer,
            &format.m,
        )?;
        let (start_index, end_index, is_code_span) =
            (format.start_index, format.end_index, format.is_code_span);
        let tag = units(format.m.str(1));
        let tag_len = tag.len();
        // The content reads a run at its edge beside the tag when it is of
        // a spec marker the tag does not cut; otherwise beside the line's
        // edge. A code span's content is not read for emphasis.
        let beside = |c: Option<u32>| -> Option<u32> {
            let c = c?;
            (!is_code_span
                && SPEC_MARKERS.iter().any(|m| u32::from(*m) == c)
                && c != u32::from(tag[0]))
            .then_some(u32::from(tag[0]))
        };
        let inside = TextPart {
            after: beside(unit_at(&text, end_index as isize - tag_len as isize - 1)),
            before: beside(unit_at(&text, (start_index + tag_len) as isize)),
            node: Some(result.transformed_node),
        };
        return Ok(import_around(
            scan,
            deferred,
            result.node_before,
            result.node_after,
            None,
            start_index,
            end_index,
            inside,
            None,
        ));
    } else if let Some(text_match) = found_text_match {
        let Some(result) = import_found_text_match_transformer(
            &text_node,
            text_match.start_index,
            text_match.end_index,
            &text_match.transformer,
            &text_match.m,
        )?
        else {
            return Ok(None);
        };
        let matched = if result.transformed_node.is_none() {
            Some(result.matched_node.clone())
        } else {
            None
        };
        return Ok(import_around(
            scan,
            deferred,
            result.node_before,
            result.node_after,
            Some(result.text_after),
            text_match.start_index,
            text_match.end_index,
            TextPart {
                after: None,
                before: None,
                node: result.transformed_node,
            },
            matched,
        ));
    }

    // Escapes, for a node no transformer matched.
    text_node.set_text_content(&unescape_text(&text_string));
    Ok(None)
}

//! `MarkdownExport.ts`: `createMarkdownExport` and
//! `createSelectionMarkdownExport`, with the flanking, escaping and
//! bracket rules that make the output read back as the document.

use super::markdown_transformers::{
    ElementExport, TextFormatTransformer, TextMatchTransformer, Transformer,
    get_code_span_delimiter, hard_line_break_state,
};
use super::utils::{
    char_units, character_at, character_before, get_portable_emphasis_sides, has_non_space,
    is_empty_paragraph, is_escaped, is_letter_for_some_reader, is_line_break_node,
    is_portably_flanked_by, is_space, re, string, to_character_reference, transformers_by_type,
    trim_end, trim_start, units,
};
use crate::link::{is_auto_link_node, is_link_node};
use crate::{
    LexicalError, NodeKey, NodeRef, PointType, SelectionRef, caret_range_from_selection, get_root,
    get_selection, get_state, get_text_point_caret_slice_for_node,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// `LexicalNode.isSelected(selection)`: whether the selection (the
/// current one when `None`) holds the node.
pub(crate) fn is_selected(
    node: &NodeRef,
    selection: Option<&SelectionRef>,
) -> Result<bool, LexicalError> {
    let target = match selection {
        Some(s) => s.clone(),
        None => match get_selection() {
            Some(s) => s,
            None => return Ok(false),
        },
    };
    let key = node.get_key();
    let is_selected = target.get_nodes()?.iter().any(|n| n.get_key() == key);
    if !is_selected || node.is_text() {
        return Ok(is_selected);
    }
    if let Some(range) = target.range()
        && range.anchor.point_type == PointType::Element
        && range.focus.point_type == PointType::Element
    {
        if range.is_collapsed() {
            return Ok(false);
        }
        if let Some(parent) = node.get_parent()
            && node.is_decorator()
            && node.is_inline()
        {
            let first_point = if target.is_backward()? {
                &range.focus
            } else {
                &range.anchor
            };
            if parent.get_key() == first_point.key
                && first_point.offset == parent.get_children_size()
                && parent.get_last_child().as_ref() == Some(node)
            {
                return Ok(false);
            }
        }
    }
    Ok(is_selected)
}

/// The text `$sliceSelectedTextNodeContent(selection, node, 'clone')`
/// leaves the node with.
fn slice_selected_text(node: &NodeRef, selection: &SelectionRef) -> Result<String, LexicalError> {
    let text = node.get_text_content();
    let Some(range) = target_points(selection) else {
        return Ok(text);
    };
    let key = node.get_key();
    if (range.0.key != key && range.1.key != key)
        || !is_selected(node, Some(selection))?
        || node.is_token()
        || node.is_segmented()
    {
        return Ok(text);
    }
    let (start, end) = if selection.is_backward()? {
        (range.1, range.0)
    } else {
        (range.0, range.1)
    };
    let u = units(&text);
    let (start_offset, end_offset) = if selection.is_range()
        && (start.point_type == PointType::Element || end.point_type == PointType::Element)
    {
        let range = selection.range().expect("a range selection");
        let slices = caret_range_from_selection(&range)?.get_text_slices();
        match get_text_point_caret_slice_for_node(&slices, node) {
            Some(slice) => {
                let (a, b) = slice.get_slice_indices();
                (a, Some(b))
            }
            None => (
                if start.key == key { start.offset } else { 0 },
                (end.key == key).then_some(end.offset),
            ),
        }
    } else {
        (
            if start.key == key { start.offset } else { 0 },
            (end.key == key).then_some(end.offset),
        )
    };
    let end_offset = end_offset.unwrap_or(u.len()).min(u.len());
    let start_offset = start_offset.min(u.len());
    Ok(if end_offset > start_offset {
        string(&u[start_offset..end_offset])
    } else {
        String::new()
    })
}

/// `selection.getStartEndPoints()`.
fn target_points(selection: &SelectionRef) -> Option<(crate::Point, crate::Point)> {
    let range = selection.range()?;
    Some((range.anchor.clone(), range.focus.clone()))
}

/// The exported text of `node`: its selected slice, if any
/// (`$exportedText`).
fn exported_text(node: &NodeRef, selection: Option<&SelectionRef>) -> Result<String, LexicalError> {
    match selection {
        Some(s) => slice_selected_text(node, s),
        None => Ok(node.get_text_content()),
    }
}

/// What export uses of the transformers.
struct Exporter {
    element_exports: Vec<ElementExport>,
    text_formats: Vec<Rc<TextFormatTransformer>>,
    text_matches: Vec<Rc<TextMatchTransformer>>,
    should_preserve_new_lines: bool,
}

impl Exporter {
    fn new(transformers: &[Transformer], should_preserve_new_lines: bool) -> Self {
        let by_type = transformers_by_type(transformers);
        let mut element_exports: Vec<ElementExport> = by_type
            .multiline_element
            .iter()
            .filter_map(|t| t.export.clone())
            .collect();
        element_exports.extend(by_type.element.iter().map(|t| t.export.clone()));
        // Formats of a single format only, with code last, so `**`code`**`
        // gets its bold.
        let mut text_formats: Vec<Rc<TextFormatTransformer>> = by_type
            .text_format
            .into_iter()
            .filter(|t| t.format.len() == 1)
            .collect();
        text_formats.sort_by_key(|t| t.format.contains(&"code"));
        Self {
            element_exports,
            text_formats,
            text_matches: by_type.text_match,
            should_preserve_new_lines,
        }
    }
}

/// `createMarkdownExport(transformers, shouldPreserveNewLines)(node)`.
pub(crate) fn export_markdown(
    transformers: &[Transformer],
    should_preserve_new_lines: bool,
    node: Option<&NodeRef>,
) -> Result<String, LexicalError> {
    let exporter = Exporter::new(transformers, should_preserve_new_lines);
    let is_newline_delimited = !should_preserve_new_lines;
    with_bracket_context(exporter.text_matches.clone(), || {
        let mut output: Vec<String> = Vec::new();
        let children = node.cloned().unwrap_or_else(get_root).get_children();
        for (i, child) in children.iter().enumerate() {
            if let Some(result) = export_top_level_elements(child, &exporter)? {
                output.push(
                    if is_newline_delimited
                        && i > 0
                        && !is_empty_paragraph(child)
                        && !is_empty_paragraph(&children[i - 1])
                    {
                        format!("\n{result}")
                    } else {
                        result
                    },
                );
            }
        }
        Ok(output.join("\n"))
    })
}

/// `createSelectionMarkdownExport(transformers,
/// shouldPreserveNewLines)(selection)`.
pub(crate) fn export_selection_markdown(
    transformers: &[Transformer],
    should_preserve_new_lines: bool,
    selection: &SelectionRef,
) -> Result<String, LexicalError> {
    let exporter = Exporter::new(transformers, should_preserve_new_lines);
    let is_newline_delimited = !should_preserve_new_lines;
    with_bracket_context(exporter.text_matches.clone(), || {
        // A copy that caches its nodes, which every node asks about.
        let nodes = selection.get_nodes()?;
        let selection = selection.clone_selection();
        selection.set_cached_nodes(Some(nodes));
        let mut output: Vec<String> = Vec::new();
        let mut previous_exported: Option<NodeRef> = None;
        for child in get_root().get_children() {
            let (should_include, markdown) =
                process_node_for_selection(&child, &selection, &exporter)?;
            if should_include && let Some(markdown) = markdown {
                output.push(
                    if is_newline_delimited
                        && previous_exported
                            .as_ref()
                            .is_some_and(|p| !is_empty_paragraph(&child) && !is_empty_paragraph(p))
                    {
                        format!("\n{markdown}")
                    } else {
                        markdown
                    },
                );
                previous_exported = Some(child);
            }
        }
        Ok(output.join("\n"))
    })
}

/// `$processNodeForSelection`.
fn process_node_for_selection(
    node: &NodeRef,
    selection: &SelectionRef,
    exporter: &Exporter,
) -> Result<(bool, Option<String>), LexicalError> {
    let mut should_include = is_selected(node, Some(selection))?;
    for export in &exporter.element_exports {
        let export_children = |n: &NodeRef| -> Result<String, LexicalError> {
            let tags = RefCell::new(Vec::new());
            let mut pieces = Pieces::default();
            export_children_for_selection(n, selection, exporter, &tags, &[], &mut pieces)?;
            Ok(join_pieces(&pieces))
        };
        if let Some(result) = export(node, &export_children, Some(selection))? {
            if !should_include && node.is_element() {
                let tags = RefCell::new(Vec::new());
                let mut pieces = Pieces::default();
                if export_children_for_selection(
                    node,
                    selection,
                    exporter,
                    &tags,
                    &[],
                    &mut pieces,
                )? {
                    should_include = true;
                }
            }
            return Ok((should_include, Some(result)));
        }
    }
    if node.is_element() {
        let tags = RefCell::new(Vec::new());
        let mut pieces = Pieces::default();
        let included =
            export_children_for_selection(node, selection, exporter, &tags, &[], &mut pieces)?;
        Ok((should_include || included, Some(join_pieces(&pieces))))
    } else if node.is_decorator() {
        Ok((should_include, Some(node.get_text_content())))
    } else {
        Ok((should_include, None))
    }
}

/// An open format tag (`OpenTag`), shared by the lists that name it, with
/// `$continuesTag`'s answers so far.
struct OpenTag {
    format: &'static str,
    tag: String,
    answers: RefCell<HashMap<NodeKey, bool>>,
}

type Tags = RefCell<Vec<Rc<OpenTag>>>;

/// `$exportChildrenForSelection`: whether any child was included.
fn export_children_for_selection(
    node: &NodeRef,
    selection: &SelectionRef,
    exporter: &Exporter,
    unclosed_tags: &Tags,
    unclosable_tags: &[Rc<OpenTag>],
    output: &mut Pieces,
) -> Result<bool, LexicalError> {
    let mut any_child_included = false;
    'main: for child in node.get_children() {
        let mut child_included = is_selected(&child, Some(selection))?;
        for transformer in &exporter.text_matches {
            let Some(export) = &transformer.export else {
                continue;
            };
            let unclosable: Vec<Rc<OpenTag>> = unclosable_tags
                .iter()
                .chain(unclosed_tags.borrow().iter())
                .cloned()
                .collect();
            let export_children = |n: &NodeRef| -> Result<String, LexicalError> {
                let mut pieces = Pieces::default();
                export_children_for_selection(
                    n,
                    selection,
                    exporter,
                    unclosed_tags,
                    &unclosable,
                    &mut pieces,
                )?;
                Ok(join_pieces(&pieces))
            };
            let export_format = |n: &NodeRef, _text: &str| -> Result<String, LexicalError> {
                let mut pieces = Pieces::default();
                export_text_format(
                    n,
                    &exported_text(n, Some(selection))?,
                    exporter,
                    unclosed_tags,
                    unclosable_tags,
                    Some(selection),
                    &mut pieces,
                )?;
                Ok(join_pieces(&pieces))
            };
            if let Some(result) = export(&child, &export_children, &export_format)? {
                if !child_included && child.is_element() {
                    let mut any = false;
                    for c in child.get_children() {
                        if is_selected(&c, Some(selection))? {
                            any = true;
                            break;
                        }
                    }
                    if any && child.extract_with_child(&child, Some(selection), "html")? {
                        child_included = true;
                    }
                }
                if child_included {
                    output.push(Kind::Raw, &result);
                    any_child_included = true;
                }
                continue 'main;
            }
        }
        if is_line_break_node(&child) {
            if child_included {
                output.push(Kind::Raw, &export_line_break(&child)?);
                any_child_included = true;
            }
        } else if child.is_text() {
            if child_included {
                export_text_format(
                    &child,
                    &exported_text(&child, Some(selection))?,
                    exporter,
                    unclosed_tags,
                    unclosable_tags,
                    Some(selection),
                    output,
                )?;
                any_child_included = true;
            }
        } else if child.is_element() {
            let included = export_children_for_selection(
                &child,
                selection,
                exporter,
                unclosed_tags,
                unclosable_tags,
                output,
            )?;
            if !child_included
                && included
                && child.extract_with_child(&child, Some(selection), "html")?
            {
                child_included = true;
            }
            if child_included || included {
                any_child_included = true;
            }
        } else if child.is_decorator() && child_included {
            output.push(Kind::Text, &child.get_text_content());
            any_child_included = true;
        }
    }
    Ok(any_child_included)
}

/// `$exportTopLevelElements`.
fn export_top_level_elements(
    node: &NodeRef,
    exporter: &Exporter,
) -> Result<Option<String>, LexicalError> {
    for export in &exporter.element_exports {
        let export_children = |n: &NodeRef| export_children(n, exporter, None, &[]);
        if let Some(result) = export(node, &export_children, None)? {
            return Ok(Some(result));
        }
    }
    if node.is_element() {
        export_children(node, exporter, None, &[]).map(Some)
    } else if node.is_decorator() {
        Ok(Some(node.get_text_content()))
    } else {
        Ok(None)
    }
}

/// `$exportChildren`.
fn export_children(
    node: &NodeRef,
    exporter: &Exporter,
    unclosed_tags: Option<&Tags>,
    unclosable_tags: &[Rc<OpenTag>],
) -> Result<String, LexicalError> {
    let own = RefCell::new(Vec::new());
    let unclosed_tags = unclosed_tags.unwrap_or(&own);
    let mut pieces = Pieces::default();
    export_child_pieces(node, exporter, unclosed_tags, unclosable_tags, &mut pieces)?;
    Ok(join_pieces(&pieces))
}

/// `$exportChildPieces`.
fn export_child_pieces(
    node: &NodeRef,
    exporter: &Exporter,
    unclosed_tags: &Tags,
    unclosable_tags: &[Rc<OpenTag>],
    output: &mut Pieces,
) -> Result<(), LexicalError> {
    'main: for child in node.get_children() {
        for transformer in &exporter.text_matches {
            let Some(export) = &transformer.export else {
                continue;
            };
            // Tags open now can't be closed by nested text match output:
            // `**text [text**](url)` is invalid markdown.
            let unclosable: Vec<Rc<OpenTag>> = unclosable_tags
                .iter()
                .chain(unclosed_tags.borrow().iter())
                .cloned()
                .collect();
            let export_children_fn =
                |n: &NodeRef| export_children(n, exporter, Some(unclosed_tags), &unclosable);
            let export_format = |n: &NodeRef, text: &str| -> Result<String, LexicalError> {
                let mut pieces = Pieces::default();
                export_text_format(
                    n,
                    text,
                    exporter,
                    unclosed_tags,
                    unclosable_tags,
                    None,
                    &mut pieces,
                )?;
                Ok(join_pieces(&pieces))
            };
            if let Some(result) = export(&child, &export_children_fn, &export_format)? {
                output.push(Kind::Raw, &result);
                continue 'main;
            }
        }
        if is_line_break_node(&child) {
            output.push(Kind::Raw, &export_line_break(&child)?);
        } else if child.is_text() {
            export_text_format(
                &child,
                &child.get_text_content(),
                exporter,
                unclosed_tags,
                unclosable_tags,
                None,
                output,
            )?;
        } else if child.is_element() {
            export_child_pieces(&child, exporter, unclosed_tags, unclosable_tags, output)?;
        } else if child.is_decorator() {
            output.push(Kind::Text, &child.get_text_content());
        }
    }
    Ok(())
}

/// `$exportLineBreak(node)`.
fn export_line_break(node: &NodeRef) -> Result<String, LexicalError> {
    Ok(get_state(node, &hard_line_break_state())? + "\n")
}

/// `$exportTextFormat`: a text node's text with its format tags, the tags
/// placed inside its whitespace (`   **foo**   `).
fn export_text_format(
    node: &NodeRef,
    text_content: &str,
    exporter: &Exporter,
    unclosed_tags: &Tags,
    unclosable_tags: &[Rc<OpenTag>],
    selection: Option<&SelectionRef>,
    pieces: &mut Pieces,
) -> Result<(), LexicalError> {
    let is_code = node.has_format("code");
    let mut output = text_content.to_owned();
    if !is_code {
        if exporter.should_preserve_new_lines {
            // A whole run of backslashes is matched, so one before no
            // marker is not tried again from each of its backslashes.
            output = re(r"\\+|[*_`~]", "g").replace_with(&output, |m| {
                let escaped = m.text();
                if !escaped.starts_with('\\') {
                    return format!("\\{escaped}");
                }
                let input = units(&m.input);
                let after = input.get(m.index + m.match_len()).copied();
                if after.is_some_and(|c| matches!(c, 0x2A | 0x5F | 0x60 | 0x7E)) {
                    format!("{escaped}{escaped}")
                } else {
                    escaped.to_owned()
                }
            });
        } else {
            let replaced = re(r"([*_`~\\]|<(?=\S|$)|&(?=#|$))", "g").replace_with(&output, |m| {
                let c = m.text();
                if matches!(c, "*" | "_" | "`" | "~" | "\\") {
                    format!("\\{c}")
                } else {
                    to_character_reference(c.chars().next().map_or(0, |c| c as u32))
                }
            });
            output = escape_literal_brackets(node, &replaced, selection)?;
        }
    }

    let holds_tag = can_hold_tag(text_content);
    let mut opening_tags = String::new();
    let mut closing_tags_before = String::new();
    let mut closing_tags_after = String::new();
    let prev_node = get_text_sibling(node, true);
    let mut applied: Vec<&'static str> = Vec::new();
    for transformer in &exporter.text_formats {
        let format = transformer.format[0];
        let tag = &transformer.tag;
        // Inline code uses a fence of its own, below.
        if format == "code" {
            continue;
        }
        if holds_tag && node.has_format(format) && !applied.contains(&format) {
            applied.push(format);
            // Open the tag unless the previous node has the format and the
            // tag is open.
            let is_open = unclosed_tags.borrow().iter().any(|t| t.tag == *tag);
            if !has_format(prev_node.as_ref(), format) || !is_open {
                unclosed_tags.borrow_mut().push(Rc::new(OpenTag {
                    answers: RefCell::new(HashMap::new()),
                    format,
                    tag: tag.clone(),
                }));
                opening_tags += tag;
            }
        }
    }

    // Close the tags in the order they were applied, where needed.
    let count = unclosed_tags.borrow().len();
    for i in 0..count {
        let tag_i = unclosed_tags.borrow()[i].clone();
        let node_has_format = has_format(Some(node), tag_i.format);
        if node_has_format && continues_tag(node, &tag_i, selection)? {
            continue;
        }
        let mut unhandled: Vec<Rc<OpenTag>> = unclosed_tags.borrow().clone();
        while unhandled.len() > i {
            let unclosed_tag = unhandled.pop().expect("a tag");
            // An unclosable tag stays open until it no longer is.
            if unclosable_tags.iter().any(|t| t.tag == unclosed_tag.tag) {
                continue;
            }
            if node_has_format {
                closing_tags_after += &unclosed_tag.tag;
            } else {
                closing_tags_before += &unclosed_tag.tag;
            }
            unclosed_tags.borrow_mut().pop();
        }
        break;
    }

    if is_code && !text_content.is_empty() {
        // Inline code is an atomic span with a fence from its content.
        let delimiter = get_code_span_delimiter(text_content);
        output = format!("{}{}{}", delimiter.fence, delimiter.padded, delimiter.fence);
    } else if !holds_tag {
        pieces.push(Kind::Closer, &(closing_tags_before + &closing_tags_after));
        pieces.push(Kind::Text, &output);
        return Ok(());
    }

    // Tags go inside the whitespace, as CommonMark's flanking needs.
    let trimmed_start = trim_start(&output);
    let trimmed_output = trim_end(trimmed_start);
    let leading_space = &output[..output.len() - trimmed_start.len()];
    let trailing_space = &trimmed_start[trimmed_output.len()..];
    pieces.push(Kind::Closer, &closing_tags_before);
    pieces.push(Kind::Text, leading_space);
    pieces.push(Kind::Opener, &opening_tags);
    // Inline code is a literal span: a character reference would show.
    pieces.push(if is_code { Kind::Raw } else { Kind::Text }, trimmed_output);
    pieces.push(Kind::Closer, &closing_tags_after);
    pieces.push(Kind::Text, trailing_space);
    Ok(())
}

/// A piece's kind (`PieceKind`): text and decorator content, which a
/// character reference can stand in for; anything else (`RAW`); or format
/// markers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Text,
    Raw,
    Opener,
    Closer,
}

fn is_marker(kind: Option<Kind>) -> bool {
    matches!(kind, Some(Kind::Opener | Kind::Closer))
}

/// `MarkdownPieces`: exported inline markdown, with the markers kept apart
/// from the text until `joinPieces` sees both sides of each run.
#[derive(Default)]
struct Pieces {
    kinds: Vec<Kind>,
    values: Vec<Vec<u16>>,
}

impl Pieces {
    /// `pushPiece`: adjacent text (and adjacent raw output) is merged.
    fn push(&mut self, kind: Kind, value: &str) {
        if value.is_empty() {
            return;
        }
        if let Some(last) = self.kinds.last()
            && *last == kind
            && !is_marker(Some(kind))
        {
            self.values
                .last_mut()
                .expect("a value")
                .extend(value.encode_utf16());
        } else {
            self.kinds.push(kind);
            self.values.push(units(value));
        }
    }
}

/// `LINK_END`: `/[\s\p{Cc}<]/u` on one code unit.
fn is_link_end(c: u32) -> bool {
    is_space(c) || is_control(c) || c == u32::from(b'<')
}

fn is_control(c: u32) -> bool {
    c < 0x20 || (0x7F..=0x9F).contains(&c)
}

/// `isLinkCharacter(character)`.
fn is_link_character(c: u16) -> bool {
    !is_link_end(u32::from(c))
}

/// `linkRunStart(text)`: where the run of URL characters that ends `text`
/// starts.
fn link_run_start(text: &[u16]) -> usize {
    let mut start = text.len();
    while start > 0 && is_link_character(text[start - 1]) {
        start -= 1;
    }
    start
}

/// `LINK_PREFIX`'s source.
const LINK_PREFIX: &str = r"(?:https?|ftp):\/\/(?![\s\p{Cc}!-,./:-@[-^`{-~])|www\.";

/// `startsLink(prefix, before)`; `before` is `None` for `''`.
fn starts_link(prefix: &str, before: Option<u16>) -> bool {
    let is_letter = |c: u16| (c as u8 as u16 == c) && (c as u8).is_ascii_alphabetic();
    if prefix.starts_with(['w', 'W']) {
        before.is_none_or(|b| matches!(b, 0x2A | 0x5F | 0x7E | 0x28))
    } else {
        !before.is_some_and(is_letter)
    }
}

/// `ADDRESS_CHARACTER`: `/[\w.+@\\-]/`.
fn is_address_character(c: u16) -> bool {
    c < 0x80 && {
        let b = c as u8;
        b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'+' | b'@' | b'\\' | b'-')
    }
}

/// `LINK_TRAIL`: what GFM readers leave out at the end of a URL.
fn is_link_trail(c: u16) -> bool {
    matches!(c, 0x3F | 0x21 | 0x2E | 0x2C | 0x3A | 0x2A | 0x5F | 0x7E)
}

/// `UNREFERENCEABLE`: control characters, surrogates and noncharacters.
fn is_unreferenceable(c: u32) -> bool {
    is_control(c)
        || (0xD800..=0xDFFF).contains(&c)
        || (0xFDD0..=0xFDEF).contains(&c)
        || (c & 0xFFFE) == 0xFFFE
}

/// `isReferenceable(character)`.
fn is_referenceable(character: Option<u32>) -> bool {
    character.is_some_and(|c| is_letter_for_some_reader(c) && !is_unreferenceable(c))
}

/// `ESCAPABLE`: `/^[!-/:-@[-`{-~\n\r]/`.
fn is_escapable(value: &[u16]) -> bool {
    value.first().is_some_and(
        |c| matches!(c, 0x21..=0x2F | 0x3A..=0x40 | 0x5B..=0x60 | 0x7B..=0x7E | 0x0A | 0x0D),
    )
}

/// `joinPieces(pieces)`: the markdown. A word character beside a marker
/// run that could not open or close otherwise goes out as a numeric
/// reference, whose `&` or `;` is punctuation.
fn join_pieces(pieces: &Pieces) -> String {
    let Pieces { kinds, values } = pieces;
    let text: Vec<u16> = values.concat();
    if kinds.iter().all(|k| *k == Kind::Text) {
        return string(&text);
    }
    let (encode_start, encode_end) = decide_references(kinds, values);
    let mut is_reference = vec![false; text.len()];
    let mut base = 0;
    for (index, value) in values.iter().enumerate() {
        let start = if encode_start[index] {
            char_units(character_at(value, 0).expect("a character"))
        } else {
            0
        };
        let mut end = value.len();
        if encode_end[index] && end > start {
            end -= char_units(character_before(value, end as isize).expect("a character"));
        }
        // A backslash left unescaped would escape the reference, marker or
        // raw punctuation after it.
        let next_kind = kinds.get(index + 1).copied();
        if kinds[index] == Kind::Text
            && (is_marker(next_kind)
                || (next_kind == Some(Kind::Raw) && is_escapable(&values[index + 1])))
            && end > start
            && is_escaped(value, end)
        {
            end -= 1;
        }
        for r in &mut is_reference[base..base + start] {
            *r = true;
        }
        for r in &mut is_reference[base + end..base + value.len()] {
            *r = true;
        }
        base += value.len();
    }
    spoil_links(kinds, values, &mut is_reference);
    let mut markdown: Vec<u16> = Vec::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        if is_reference[at] {
            let character = character_at(&text, at as isize).expect("a character");
            markdown.extend(to_character_reference(character).encode_utf16());
            at += char_units(character);
        } else {
            markdown.push(text[at]);
            at += 1;
        }
    }
    string(&markdown)
}

/// `maskRaw(value)`: raw output as the bare-URL scan sees it.
fn mask_raw(value: &[u16]) -> Vec<u16> {
    value
        .iter()
        .map(|&c| {
            if c < 0x80 && (c as u8).is_ascii_alphabetic() {
                u16::from(b'x')
            } else if is_space(u32::from(c)) || matches!(c, 0x3C | 0x2A | 0x5F | 0x7E | 0x28) {
                c
            } else {
                u16::from(b'#')
            }
        })
        .collect()
}

/// `LINKISH`: `/www\.|:\/\/|@/i`.
fn is_linkish(value: &[u16]) -> bool {
    re(r"www\.|:\/\/|@", "i").test(&string(value))
}

/// `spoilLinks(kinds, values, isReference)`: writes the `.` of `www.` or
/// the `:` after a scheme as a reference where a GFM reader would link a
/// URL that a reference lands in or that takes in the markers or raw
/// output after it.
fn spoil_links(kinds: &[Kind], values: &[Vec<u16>], is_reference: &mut [bool]) {
    let has_references = is_reference.iter().any(|r| *r);
    if !values.iter().enumerate().any(|(i, value)| {
        kinds[i] == Kind::Text
            && is_linkish(value)
            && (has_references || is_linkish(&value[link_run_start(value)..]))
    }) {
        return;
    }
    let mut text: Vec<u16> = Vec::new();
    let mut is_format: Vec<bool> = Vec::new();
    for (i, value) in values.iter().enumerate() {
        if kinds[i] == Kind::Raw {
            text.extend(mask_raw(value));
        } else {
            text.extend_from_slice(value);
        }
        is_format.extend(std::iter::repeat_n(kinds[i] != Kind::Text, value.len()));
    }
    let mut references = vec![0u32; text.len() + 1];
    let mut formats = vec![0u32; text.len() + 1];
    for i in 0..text.len() {
        references[i + 1] = references[i] + u32::from(is_reference[i]);
        formats[i + 1] = formats[i] + u32::from(is_format[i]);
    }
    let has = |sums: &[u32], from: usize, to: usize| from < to && sums[to] > sums[from];
    let address = re(r"[\w.+-]@[\w-]+\.[\w-]", "");
    // A reference in an email would cut a micromark reader's link short,
    // so each `@` of one that holds a reference is spoiled.
    let spoil_emails = |is_reference: &mut [bool], from: usize, to: usize| {
        let mut start = from;
        while start < to {
            let mut end = start;
            while end < to && is_address_character(text[end]) {
                end += 1;
            }
            if has(&references, start, end) {
                let plain: Vec<u16> = text[start..end]
                    .iter()
                    .copied()
                    .filter(|c| *c != u16::from(b'\\'))
                    .collect();
                if address.test(&string(&plain)) {
                    for i in start..end {
                        if text[i] == u16::from(b'@')
                            && !is_reference[i]
                            && !is_format[i]
                            && !is_escaped(&text, i)
                        {
                            is_reference[i] = true;
                        }
                    }
                }
            }
            start = end.max(start + 1);
        }
    };
    let link_prefix = re(LINK_PREFIX, "giu");
    let mut start = 0;
    while start < text.len() {
        let mut end = start;
        while end < text.len() && (is_link_character(text[end]) || is_reference[end]) {
            end += 1;
        }
        if end == start {
            start += 1;
            continue;
        }
        // A URL runs to the end of the run, less the punctuation at its end.
        let mut trimmed = end;
        while trimmed > start && is_link_trail(text[trimmed - 1]) {
            trimmed -= 1;
        }
        if is_escaped(&text, trimmed) && has(&formats, trimmed, end) {
            trimmed = end;
        }
        let mut rest = start;
        for prefix in link_prefix.match_all(&string(&text[start..end])) {
            let at = start + prefix.index;
            if at < rest {
                continue;
            }
            let before = (at > start).then(|| text[at - 1]);
            let was_link = at == start || starts_link(prefix.text(), before);
            if at > start
                && !starts_link(
                    prefix.text(),
                    if is_reference[at - 1] {
                        Some(u16::from(b';'))
                    } else {
                        before
                    },
                )
            {
                continue;
            }
            spoil_emails(is_reference, rest, at);
            let prefix_len = prefix.match_len();
            if was_link && !has(&references, at, end) && !has(&formats, at, trimmed) {
                rest = end;
                break;
            }
            // A reference in the prefix spoils it already; a `.` or `:`
            // that is a custom format's marker is left.
            let spoil = at
                + prefix
                    .text()
                    .encode_utf16()
                    .position(|c| c == u16::from(b'.') || c == u16::from(b':'))
                    .unwrap_or(0);
            if !has(&references, at, at + prefix_len) {
                if is_format[spoil] {
                    rest = end;
                    break;
                }
                is_reference[spoil] = true;
            }
            rest = at + prefix_len;
        }
        spoil_emails(is_reference, rest, end);
        start = end;
    }
}

/// `MarkerRun`: the pieces before and after a run of markers, whether its
/// first delimiter run opens, the marker after that run within the markers,
/// whether its last closes, and the marker before that.
#[derive(Clone, Copy)]
struct MarkerRun {
    before: isize,
    after: usize,
    opens: bool,
    after_first: Option<u16>,
    closes: bool,
    before_last: Option<u16>,
}

/// `scanDelimiterRun(kinds, values, from, to, step, kind)`.
fn scan_delimiter_run(
    kinds: &[Kind],
    values: &[Vec<u16>],
    from: usize,
    to: usize,
    step: isize,
    kind: Kind,
) -> (bool, Option<u16>) {
    let edge = &values[from];
    let delimiter = if step == 1 {
        edge[0]
    } else {
        edge[edge.len() - 1]
    };
    let mut has_kind = false;
    let mut i = from as isize;
    loop {
        let value = &values[i as usize];
        let indexes: Box<dyn Iterator<Item = usize>> = if step == 1 {
            Box::new(0..value.len())
        } else {
            Box::new((0..value.len()).rev())
        };
        for c in indexes {
            if value[c] != delimiter {
                return (has_kind, Some(value[c]));
            }
            has_kind |= kinds[i as usize] == kind;
        }
        if i == to as isize {
            break;
        }
        i += step;
    }
    (has_kind, None)
}

/// `decideReferences(kinds, values)`: which text pieces' first and last
/// characters go out as references for the marker run beside them to open
/// or close.
fn decide_references(kinds: &[Kind], values: &[Vec<u16>]) -> (Vec<bool>, Vec<bool>) {
    let count = kinds.len();
    let mut encode_start = vec![false; count];
    let mut encode_end = vec![false; count];
    let mut runs: Vec<MarkerRun> = Vec::new();
    let mut run_after: Vec<Option<usize>> = vec![None; count];
    let mut run_before: Vec<Option<usize>> = vec![None; count];
    let mut start = 0;
    while start < count {
        if is_marker(Some(kinds[start])) {
            let mut end = start;
            while is_marker(kinds.get(end + 1).copied()) {
                end += 1;
            }
            if start > 0 {
                run_after[start - 1] = Some(runs.len());
            }
            if end + 1 < count {
                run_before[end + 1] = Some(runs.len());
            }
            let (opens, after_first) =
                scan_delimiter_run(kinds, values, start, end, 1, Kind::Opener);
            let (closes, before_last) =
                scan_delimiter_run(kinds, values, end, start, -1, Kind::Closer);
            runs.push(MarkerRun {
                before: start as isize - 1,
                after: end + 1,
                opens,
                after_first,
                closes,
                before_last,
            });
            start = end;
        }
        start += 1;
    }
    let first_char = |i: usize| character_at(&values[i], 0).expect("a character");
    let last_char =
        |i: usize| character_before(&values[i], values[i].len() as isize).expect("a character");
    let is_one_character = |i: usize| char_units(first_char(i)) == values[i].len();
    let amp = u32::from(b'&');
    let semi = u32::from(b';');

    // The state the closures read lives in cells, as they are rewritten
    // while runs are decided.
    let encode_start_c = RefCell::new(std::mem::take(&mut encode_start));
    let encode_end_c = RefCell::new(std::mem::take(&mut encode_end));
    let is_encoded =
        |i: usize| (encode_start_c.borrow()[i] || encode_end_c.borrow()[i]) && is_one_character(i);
    let start_char = |i: usize| -> Option<u32> {
        if i >= count {
            None
        } else if encode_start_c.borrow()[i] || is_encoded(i) {
            Some(amp)
        } else {
            Some(first_char(i))
        }
    };
    let end_char = |i: isize| -> Option<u32> {
        if i < 0 {
            None
        } else if encode_end_c.borrow()[i as usize] || is_encoded(i as usize) {
            Some(semi)
        } else {
            Some(last_char(i as usize))
        }
    };
    let can_reference = |i: isize, at_end: bool| -> bool {
        i >= 0
            && (i as usize) < count
            && kinds[i as usize] == Kind::Text
            && is_referenceable(if at_end {
                end_char(i)
            } else {
                start_char(i as usize)
            })
    };
    // 0 while unknown, 1 when it spreads and 2 when it is blocked.
    let spreads = [
        RefCell::new(vec![0u8; count]),
        RefCell::new(vec![0u8; count]),
    ];
    let can_spread = |index: usize, leftward: bool| -> bool {
        let known = &spreads[usize::from(leftward)];
        let mut visited: Vec<usize> = Vec::new();
        let mut result = true;
        let mut piece = index;
        loop {
            let k = known.borrow()[piece];
            if k != 0 {
                result = k == 1;
                break;
            }
            visited.push(piece);
            let run_index = if leftward {
                run_before[piece]
            } else {
                run_after[piece]
            };
            let Some(run_index) = run_index else {
                break;
            };
            let run = runs[run_index];
            let next: isize = if leftward {
                run.before
            } else {
                run.after as isize
            };
            let touches_both = if leftward {
                run.opens && run.after_first.is_none()
            } else {
                run.closes && run.before_last.is_none()
            };
            if !touches_both
                || is_portably_flanked_by(
                    Some(if leftward { amp } else { semi }),
                    if leftward {
                        end_char(next)
                    } else {
                        start_char(next as usize)
                    },
                )
            {
                break;
            }
            if !can_reference(next, leftward) {
                result = false;
                break;
            }
            if !is_one_character(next as usize) {
                break;
            }
            piece = next as usize;
        }
        let mut known = known.borrow_mut();
        for p in visited {
            known[p] = if result { 1 } else { 2 };
        }
        result
    };
    let is_helped_by = |run: &MarkerRun, opening: bool| -> bool {
        let piece: isize = if opening {
            run.before
        } else {
            run.after as isize
        };
        if !(if opening { run.opens } else { run.closes }) || !can_reference(piece, opening) {
            return false;
        }
        let marker = if opening {
            u32::from(values[(run.before + 1) as usize][0])
        } else {
            let v = &values[run.after - 1];
            u32::from(v[v.len() - 1])
        };
        let previous = (if opening {
            None
        } else {
            run.before_last.map(u32::from)
        })
        .or_else(|| end_char(run.before));
        let next = (if opening {
            run.after_first.map(u32::from)
        } else {
            None
        })
        .or_else(|| start_char(run.after));
        let side = |s: super::utils::EmphasisSides| if opening { s.can_open } else { s.can_close };
        !side(get_portable_emphasis_sides(marker, previous, next))
            && side(get_portable_emphasis_sides(
                marker,
                if opening { Some(semi) } else { previous },
                if opening { next } else { Some(amp) },
            ))
            && (!is_one_character(piece as usize) || can_spread(piece as usize, opening))
    };
    // A tag character text doesn't escape (the `=` of `==`) beside a
    // marker of the same character would join its run.
    for i in 0..count {
        let value = &values[i];
        if kinds[i] == Kind::Text {
            if is_marker(kinds.get(i + 1).copied())
                && value[value.len() - 1] == values[i + 1][0]
                && !is_escaped(value, value.len() - 1)
            {
                encode_end_c.borrow_mut()[i] = true;
            }
            if i > 0 && is_marker(Some(kinds[i - 1])) {
                let prev = &values[i - 1];
                if value[0] == prev[prev.len() - 1] {
                    encode_start_c.borrow_mut()[i] = true;
                }
            }
        }
    }
    let mut pending: Vec<usize> = (0..runs.len()).collect();
    while let Some(index) = pending.pop() {
        let run = runs[index];
        if is_helped_by(&run, true) {
            let before = run.before as usize;
            encode_end_c.borrow_mut()[before] = true;
            if is_encoded(before)
                && let Some(r) = run_before[before]
            {
                pending.push(r);
            }
        }
        if is_helped_by(&run, false) {
            encode_start_c.borrow_mut()[run.after] = true;
            // An opener that sees the same character, of a run that both
            // closes and opens, may now need its own reference.
            if run.opens && run.after_first.is_none() {
                pending.push(index);
            }
            if is_encoded(run.after)
                && let Some(r) = run_after[run.after]
            {
                pending.push(r);
            }
        }
    }
    (encode_start_c.into_inner(), encode_end_c.into_inner())
}

// --- Brackets -------------------------------------------------------------

/// `BARRIER`: exported text that cannot be part of a label (a link).
const BARRIER: u32 = 0x01;
/// `UNKNOWN`: exported text this pass cannot see.
const UNKNOWN: u32 = 0x02;
/// `MAX_LABEL_DISTANCE`.
const MAX_LABEL_DISTANCE: isize = 1000;

/// `asLiteral` of one character.
fn as_literal_char(c: u32) -> u32 {
    if c == BARRIER || c == UNKNOWN {
        0xFFFD
    } else {
        c
    }
}

/// `asLiteral(text)`.
fn as_literal(text: &str) -> String {
    text.replace(['\u{1}', '\u{2}'], "\u{FFFD}")
}

/// `LinePhase`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinePhase {
    Start,
    Bullet,
    Digits,
    Text,
}

/// `LineState`. A missing character (JS `''`) is `None`.
#[derive(Clone, Copy, Debug)]
struct LineState {
    phase: LinePhase,
    opener_distance: isize,
    opener_char: Option<u32>,
    opener_next: Option<u32>,
    opener_at_line_start: bool,
}

const BLOCK_START: LineState = LineState {
    opener_at_line_start: false,
    opener_char: None,
    opener_distance: -1,
    opener_next: None,
    phase: LinePhase::Start,
};

fn is_digit(c: u32) -> bool {
    (u32::from(b'0')..=u32::from(b'9')).contains(&c)
}

/// `nextLinePhase(phase, character)`.
fn next_line_phase(phase: LinePhase, character: u32) -> LinePhase {
    if character == UNKNOWN {
        return LinePhase::Start;
    }
    let space = is_space(character);
    match phase {
        LinePhase::Start => {
            if space || character == u32::from(b'>') {
                LinePhase::Start
            } else if matches!(character, 0x2D | 0x2A | 0x2B) {
                LinePhase::Bullet
            } else if is_digit(character) {
                LinePhase::Digits
            } else {
                LinePhase::Text
            }
        }
        LinePhase::Bullet => {
            if space {
                LinePhase::Start
            } else {
                LinePhase::Text
            }
        }
        LinePhase::Digits => {
            if is_digit(character) {
                LinePhase::Digits
            } else if matches!(character, 0x2E | 0x29) {
                LinePhase::Bullet
            } else {
                LinePhase::Text
            }
        }
        LinePhase::Text => LinePhase::Text,
    }
}

/// `advanceLineState(state, character)`.
fn advance_line_state(state: &mut LineState, character: u32) {
    if state.opener_distance != -1 {
        state.opener_distance += 1;
        if state.opener_distance == 1 {
            state.opener_next = Some(character);
        } else if state.opener_distance > MAX_LABEL_DISTANCE {
            state.opener_distance = -1;
        }
    }
    if character == u32::from(b'\n') {
        state.phase = LinePhase::Start;
        return;
    }
    if character == u32::from(b'[') || character == BARRIER || character == UNKNOWN {
        let keeps_line_start = character == UNKNOWN
            && state.opener_distance != -1
            && state.opener_char != Some(BARRIER)
            && state.opener_at_line_start;
        state.opener_distance = 0;
        state.opener_char = Some(character);
        state.opener_next = None;
        state.opener_at_line_start = state.phase == LinePhase::Start || keeps_line_start;
    }
    state.phase = next_line_phase(state.phase, character);
}

/// `linkTextBracketsToEscape(text, nextChar)`.
fn link_text_brackets_to_escape(text: &[u16], next_char: Option<u32>) -> HashSet<usize> {
    let mut escaped = HashSet::new();
    let mut openers: Vec<usize> = Vec::new();
    for (index, &c) in text.iter().enumerate() {
        if c == u16::from(b'[') {
            openers.push(index);
        } else if c == u16::from(b']') {
            let opener = openers.pop();
            let after = text.get(index + 1).map(|u| u32::from(*u)).or(next_char);
            match opener {
                None => {
                    escaped.insert(index);
                }
                Some(o) if after == Some(u32::from(b'(')) || after == Some(UNKNOWN) => {
                    escaped.insert(o);
                    escaped.insert(index);
                }
                Some(_) => {}
            }
        }
    }
    escaped.extend(openers);
    escaped
}

/// `$literalBracketsToEscape(before, text, nextChar, $labelClosesAfter)`.
fn literal_brackets_to_escape(
    before: LineState,
    text: &[u16],
    next_char: Option<u32>,
    label_closes_after: &dyn Fn(isize) -> Result<bool, LexicalError>,
) -> Result<HashSet<usize>, LexicalError> {
    let mut escaped = HashSet::new();
    let mut line_start_openers: Vec<usize> = Vec::new();
    let mut state = before;
    for index in 0..text.len() {
        let character = as_literal_char(u32::from(text[index]));
        if character == u32::from(b']') {
            let after = text.get(index + 1).map(|u| u32::from(*u)).or(next_char);
            let distance = if state.opener_distance == -1 {
                -1
            } else {
                state.opener_distance + 1
            };
            let opens_line =
                distance != -1 && state.opener_char != Some(BARRIER) && state.opener_at_line_start;
            let is_unknown = state.opener_char == Some(UNKNOWN);
            let after_is_space = after.is_some_and(is_space);
            if after == Some(u32::from(b'('))
                || after == Some(UNKNOWN)
                || (opens_line && after == Some(u32::from(b':')))
                || (opens_line
                    && after_is_space
                    && (is_unknown
                        || distance == 1
                        || (distance == 2
                            && state.opener_next.is_some_and(|n| {
                                is_space(n) || n == u32::from(b'x') || n == u32::from(b'X')
                            }))))
            {
                escaped.insert(index);
            }
        } else if character == u32::from(b'[') && state.phase == LinePhase::Start {
            line_start_openers.push(index);
        }
        advance_line_state(&mut state, character);
    }
    for index in line_start_openers {
        if label_closes_in_code(text, index, &escaped, label_closes_after)? {
            // The `]` that would close it is in inline code, where it can't
            // be escaped, so the `[` is.
            escaped.insert(index);
        }
    }
    Ok(escaped)
}

/// `$labelClosesInCode(text, index, escaped, $rest)`.
fn label_closes_in_code(
    text: &[u16],
    index: usize,
    escaped: &HashSet<usize>,
    rest: &dyn Fn(isize) -> Result<bool, LexicalError>,
) -> Result<bool, LexicalError> {
    for (next, &unit) in text.iter().enumerate().skip(index + 1) {
        if unit == u16::from(b'[') || (unit == u16::from(b']') && !escaped.contains(&next)) {
            return Ok(false);
        }
    }
    let budget = MAX_LABEL_DISTANCE - (text.len() - index) as isize;
    Ok(budget > 0 && rest(budget)?)
}

/// `$labelClosesInCodeAfter(node, selection, budget)`.
fn label_closes_in_code_after(
    node: &NodeRef,
    selection: Option<&SelectionRef>,
    budget: isize,
) -> Result<bool, LexicalError> {
    let mut remaining = budget;
    let mut leaf = next_leaf(node, selection)?;
    while let Some(l) = leaf {
        if remaining <= 0 {
            break;
        }
        match export_kind(&l) {
            ExportKind::Link => return Ok(false),
            ExportKind::Unknown => return Ok(true),
            _ => {}
        }
        if l.is_text() {
            let text = units(&as_literal(&exported_text(&l, selection)?));
            if l.has_format("code") {
                if let Some(bracket) = text
                    .iter()
                    .position(|c| *c == u16::from(b'[') || *c == u16::from(b']'))
                {
                    return Ok(text[bracket] == u16::from(b']')
                        && text.get(bracket + 1) == Some(&u16::from(b':')));
                }
            } else if text.contains(&u16::from(b'[')) {
                return Ok(false);
            }
            remaining -= text.len() as isize;
        } else if is_line_break_node(&l) {
            remaining -= 1;
        }
        leaf = next_leaf(&l, selection)?;
    }
    Ok(false)
}

/// `ExportKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExportKind {
    Link,
    Text,
    Transparent,
    Unknown,
}

/// `BracketContext`: the export in progress's text match transformers, the
/// export kind of each node asked about, and the line state after each
/// leaf read.
struct BracketContext {
    text_match_transformers: Vec<Rc<TextMatchTransformer>>,
    export_kinds: HashMap<NodeKey, ExportKind>,
    line_states: HashMap<NodeKey, LineState>,
}

thread_local! {
    static BRACKET_CONTEXT: RefCell<Option<BracketContext>> = const { RefCell::new(None) };
}

/// `withBracketContext(textMatchTransformers, callback)`.
fn with_bracket_context<T>(
    text_match_transformers: Vec<Rc<TextMatchTransformer>>,
    callback: impl FnOnce() -> T,
) -> T {
    let previous = BRACKET_CONTEXT.with(|c| {
        c.replace(Some(BracketContext {
            export_kinds: HashMap::new(),
            line_states: HashMap::new(),
            text_match_transformers,
        }))
    });
    struct Restore(Option<Option<BracketContext>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take().expect("a context to restore");
            BRACKET_CONTEXT.with(|c| *c.borrow_mut() = previous);
        }
    }
    let _restore = Restore(Some(previous));
    callback()
}

/// `$exportKind(node)`.
fn export_kind(node: &NodeRef) -> ExportKind {
    if is_line_break_node(node) || (node.is_element() && !node.is_inline()) {
        return ExportKind::Text;
    } else if !node.is_text() && !node.is_element() {
        return ExportKind::Unknown;
    }
    let key = node.get_key().to_owned();
    let (has_context, cached, transformers) = BRACKET_CONTEXT.with(|c| match &*c.borrow() {
        Some(context) => (
            true,
            context.export_kinds.get(&key).copied(),
            context.text_match_transformers.clone(),
        ),
        None => (false, None, Vec::new()),
    });
    if let Some(cached) = cached {
        return cached;
    }
    let unknown = char::from_u32(UNKNOWN).expect("a char").to_string();
    let mut written: Option<String> = None;
    if has_context {
        for transformer in &transformers {
            let Some(export) = &transformer.export else {
                continue;
            };
            let empty_children = |_: &NodeRef| Ok(String::new());
            let empty_format = |_: &NodeRef, _: &str| Ok(String::new());
            written = export(node, &empty_children, &empty_format).unwrap_or(Some(unknown.clone()));
            if written.is_some() {
                break;
            }
        }
    } else if is_link_node(node) && !is_auto_link_node(node) {
        written = Some("[]()".to_owned());
    }
    let kind = match written {
        None if node.is_text() => ExportKind::Text,
        None => ExportKind::Transparent,
        Some(w) if node.is_element() && w.starts_with('[') => ExportKind::Link,
        Some(_) => ExportKind::Unknown,
    };
    BRACKET_CONTEXT.with(|c| {
        if let Some(context) = c.borrow_mut().as_mut() {
            context.export_kinds.insert(key, kind);
        }
    });
    kind
}

/// `$isInsideLink(node)`.
fn is_inside_link(node: &NodeRef) -> bool {
    let mut parent = node.get_parent();
    while let Some(p) = parent {
        if export_kind(&p) == ExportKind::Link {
            return true;
        }
        if !p.is_inline() {
            return false;
        }
        parent = p.get_parent();
    }
    false
}

/// `$isInlineElement(node)`.
fn is_inline_element(node: Option<&NodeRef>) -> bool {
    node.is_some_and(|n| n.is_element() && n.is_inline())
}

/// `$isTransparentInline(node)`.
fn is_transparent_inline(node: Option<&NodeRef>) -> bool {
    node.is_some_and(|n| {
        n.is_element() && n.is_inline() && export_kind(n) == ExportKind::Transparent
    })
}

/// `$nextExportedChar(node, selection)`: the first character of the
/// exported text after `node` in its block (`None` for JS `''`).
fn next_exported_char(
    node: &NodeRef,
    selection: Option<&SelectionRef>,
) -> Result<Option<u32>, LexicalError> {
    let mut current = node.clone();
    loop {
        let mut next = current.get_next_sibling();
        while next.is_none() {
            let parent = current.get_parent();
            if let Some(p) = &parent
                && export_kind(p) == ExportKind::Link
            {
                return Ok(Some(u32::from(b']')));
            }
            if !is_transparent_inline(parent.as_ref()) {
                return Ok(is_inline_element(parent.as_ref()).then_some(UNKNOWN));
            }
            current = parent.expect("a parent");
            next = current.get_next_sibling();
        }
        let mut next = next.expect("a sibling");
        while is_transparent_inline(Some(&next))
            && let Some(first) = next.get_first_child()
        {
            next = first;
        }
        if let Some(selection) = selection
            && !is_selected(&next, Some(selection))?
        {
            return Ok(None);
        }
        let kind = export_kind(&next);
        if kind == ExportKind::Link {
            return Ok(Some(u32::from(b'[')));
        } else if kind == ExportKind::Unknown {
            return Ok(Some(UNKNOWN));
        } else if next.is_text() {
            let exported = units(&exported_text(&next, selection)?);
            if let Some(first) = exported.first() {
                return Ok(Some(if next.has_format("code") {
                    u32::from(b'`')
                } else {
                    as_literal_char(u32::from(*first))
                }));
            }
        } else if is_line_break_node(&next) {
            return Ok(Some(u32::from(b'\n')));
        }
        current = next;
    }
}

/// `$nextLeaf(node, selection)`.
fn next_leaf(
    node: &NodeRef,
    selection: Option<&SelectionRef>,
) -> Result<Option<NodeRef>, LexicalError> {
    let mut current = node.clone();
    let mut next = current.get_next_sibling();
    while next.is_none() {
        let parent = current.get_parent();
        if !is_transparent_inline(parent.as_ref()) {
            return Ok(parent.filter(|p| is_inline_element(Some(p))));
        }
        current = parent.expect("a parent");
        next = current.get_next_sibling();
    }
    let mut next = next.expect("a sibling");
    while is_transparent_inline(Some(&next))
        && let Some(first) = next.get_first_child()
    {
        next = first;
    }
    if let Some(selection) = selection
        && !is_selected(&next, Some(selection))?
    {
        return Ok(None);
    }
    Ok(Some(next))
}

/// `$previousLeaf(node, selection)`.
fn previous_leaf(
    node: &NodeRef,
    selection: Option<&SelectionRef>,
) -> Result<Option<NodeRef>, LexicalError> {
    let mut current = node.clone();
    let mut previous = current.get_previous_sibling();
    while previous.is_none() {
        let parent = current.get_parent();
        if !is_transparent_inline(parent.as_ref()) {
            return Ok(parent.filter(|p| is_inline_element(Some(p))));
        }
        current = parent.expect("a parent");
        previous = current.get_previous_sibling();
    }
    let mut previous = previous.expect("a sibling");
    while is_transparent_inline(Some(&previous))
        && let Some(last) = previous.get_last_child()
    {
        previous = last;
    }
    if let Some(selection) = selection
        && !is_selected(&previous, Some(selection))?
    {
        return Ok(None);
    }
    Ok(Some(previous))
}

/// `$leafText(leaf, selection)`, as the characters LineState reads (code
/// points, as JS's `for...of` iterates).
fn leaf_text(leaf: &NodeRef, selection: Option<&SelectionRef>) -> Result<Vec<u32>, LexicalError> {
    Ok(match export_kind(leaf) {
        ExportKind::Link => vec![BARRIER],
        ExportKind::Unknown => vec![UNKNOWN],
        _ if leaf.is_text() => {
            let text = as_literal(&exported_text(leaf, selection)?);
            let is_code = leaf.has_format("code");
            text.chars()
                .map(|c| {
                    if is_code && matches!(c, '[' | ']' | ' ' | '\t') {
                        u32::from(b'`')
                    } else {
                        c as u32
                    }
                })
                .collect()
        }
        _ if is_line_break_node(leaf) => vec![u32::from(b'\n')],
        _ => Vec::new(),
    })
}

/// `$lineStateBefore(node, selection)`.
fn line_state_before(
    node: &NodeRef,
    selection: Option<&SelectionRef>,
) -> Result<LineState, LexicalError> {
    let mut pending: Vec<NodeRef> = Vec::new();
    let mut state = BLOCK_START;
    let mut leaf = previous_leaf(node, selection)?;
    while let Some(l) = leaf {
        let cached = BRACKET_CONTEXT.with(|c| {
            c.borrow()
                .as_ref()
                .and_then(|context| context.line_states.get(l.get_key()).copied())
        });
        if let Some(cached) = cached {
            state = cached;
            break;
        }
        leaf = previous_leaf(&l, selection)?;
        pending.push(l);
    }
    for l in pending.iter().rev() {
        let mut next = state;
        for character in leaf_text(l, selection)? {
            advance_line_state(&mut next, character);
        }
        BRACKET_CONTEXT.with(|c| {
            if let Some(context) = c.borrow_mut().as_mut() {
                context.line_states.insert(l.get_key().to_owned(), next);
            }
        });
        state = next;
    }
    Ok(state)
}

/// `$escapeLiteralBrackets(node, text, selection)`: escapes only the
/// literal brackets that would otherwise turn into markdown syntax.
fn escape_literal_brackets(
    node: &NodeRef,
    text: &str,
    selection: Option<&SelectionRef>,
) -> Result<String, LexicalError> {
    if !text.contains(['[', ']']) {
        return Ok(text.to_owned());
    }
    let u = units(text);
    let next_char = next_exported_char(node, selection)?;
    let escaped = if is_inside_link(node) {
        link_text_brackets_to_escape(&u, next_char)
    } else {
        let before = line_state_before(node, selection)?;
        literal_brackets_to_escape(before, &u, next_char, &|budget| {
            label_closes_in_code_after(node, selection, budget)
        })?
    };
    if escaped.is_empty() {
        return Ok(text.to_owned());
    }
    let mut output: Vec<u16> = Vec::with_capacity(u.len());
    for (index, c) in u.iter().enumerate() {
        if escaped.contains(&index) {
            output.extend(format!("&#{c};").encode_utf16());
        } else {
            output.push(*c);
        }
    }
    Ok(string(&output))
}

/// `getTextSibling(node, backward)`.
fn get_text_sibling(node: &NodeRef, backward: bool) -> Option<NodeRef> {
    let sibling = if backward {
        node.get_previous_sibling()
    } else {
        node.get_next_sibling()
    };
    sibling.filter(NodeRef::is_text)
}

/// `$continuesTag(node, tag, selection)`: whether `tag` carries on past
/// `node` to exported text that can hold it.
fn continues_tag(
    node: &NodeRef,
    tag: &OpenTag,
    selection: Option<&SelectionRef>,
) -> Result<bool, LexicalError> {
    if let Some(known) = tag.answers.borrow().get(node.get_key()) {
        return Ok(*known);
    }
    let mut passed: Vec<NodeRef> = Vec::new();
    let mut next = get_text_sibling(node, false);
    let mut result = false;
    while let Some(n) = next {
        if !n.has_format(tag.format) {
            break;
        }
        if let Some(selection) = selection
            && !is_selected(&n, Some(selection))?
        {
            break;
        }
        let text = exported_text(&n, selection)?;
        if can_hold_tag(&text) {
            result = true;
            break;
        }
        // Markdown import matches a tag within one line.
        if text.contains('\n') {
            break;
        }
        if let Some(answer) = tag.answers.borrow().get(n.get_key()) {
            result = *answer;
            break;
        }
        next = get_text_sibling(&n, false);
        passed.push(n);
    }
    let mut answers = tag.answers.borrow_mut();
    answers.insert(node.get_key().to_owned(), result);
    for p in passed {
        answers.insert(p.get_key().to_owned(), result);
    }
    Ok(result)
}

/// `hasFormat(node, format)`.
fn has_format(node: Option<&NodeRef>, format: &str) -> bool {
    node.is_some_and(|n| n.is_text() && n.has_format(format))
}

/// `canHoldTag(text)`: CommonMark keeps tags off whitespace.
fn can_hold_tag(text: &str) -> bool {
    has_non_space(text)
}

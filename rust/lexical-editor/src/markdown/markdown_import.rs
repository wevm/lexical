//! `MarkdownImport.ts`: `$importMarkdownNodes`, which parses markdown line
//! by line into a container.

use super::import_text_transformers::import_text_transformers;
use super::markdown_transformers::{
    ElementTransformer, MultilineElementTransformer, MultilineImportArgs, MultilineImportResult,
    TextFormatTransformer, TextMatchTransformer, Transformer, create_markdown_line_break_node,
    with_list_indent_columns,
};
use super::reg_exp::RegExpMatch;
use super::utils::{
    index_of, is_empty_paragraph, is_paragraph_node, is_tab_node, string, transformers_by_type,
    units,
};
use crate::list::{is_list_item_node, is_list_node};
use crate::nodes::tab::create_tab_node;
use crate::rich_text::is_quote_node;
use crate::utils::find_matching_parent;
use crate::{LexicalError, NodeRef, create_paragraph_node, create_text_node, utf16};
use std::rc::Rc;

/// `TextFormatTransformersIndex`: the text format transformers by tag, in
/// the order their tags were first given (a later one with the same tag
/// replaces an earlier one, as JS's object does).
#[derive(Clone, Default)]
pub(crate) struct TextFormatTransformersIndex {
    pub transformers_by_tag: Vec<(String, Rc<TextFormatTransformer>)>,
}

impl TextFormatTransformersIndex {
    pub(crate) fn transformer(&self, tag: &str) -> Option<Rc<TextFormatTransformer>> {
        self.transformers_by_tag
            .iter()
            .find(|(t, _)| t == tag)
            .map(|(_, t)| t.clone())
    }
}

/// `$importMarkdownNodes(markdownString, container, transformers,
/// shouldPreserveNewLines)`: parses markdown and appends the nodes to
/// `container`, without clearing it or touching the selection.
pub fn import_markdown_nodes(
    markdown_string: &str,
    container: &NodeRef,
    transformers: &[Transformer],
    should_preserve_new_lines: bool,
) -> Result<(), LexicalError> {
    let by_type = transformers_by_type(transformers);
    let index = create_text_format_transformers_index(&by_type.text_format);
    let lines: Vec<String> = markdown_string.split('\n').map(str::to_owned).collect();
    // Blank lines between list lines make the list loose rather than
    // ending it, unless they are preserved.
    with_list_indent_columns(
        !should_preserve_new_lines,
        || -> Result<(), LexicalError> {
            let mut i = 0;
            while i < lines.len() {
                let (imported, shifted_index) =
                    import_multiline(&lines, i, &by_type.multiline_element, container)?;
                if imported {
                    i = shifted_index + 1;
                    continue;
                }
                import_blocks(
                    &lines[i],
                    container,
                    &by_type.element,
                    &index,
                    &by_type.text_match,
                    should_preserve_new_lines,
                )?;
                i += 1;
            }
            Ok(())
        },
    )?;
    for child in container.get_children() {
        if !should_preserve_new_lines
            && is_empty_paragraph(&child)
            && container.get_children_size() > 1
        {
            child.remove(false)?;
            continue;
        }
        if child.is_element() {
            for text_node in get_all_text_nodes(&child) {
                normalize_markdown_text_node(&text_node)?;
            }
        }
    }
    Ok(())
}

/// `ElementNode.getAllTextNodes()`.
fn get_all_text_nodes(node: &NodeRef) -> Vec<NodeRef> {
    let mut text_nodes = Vec::new();
    for child in node.get_children() {
        if child.is_text() {
            text_nodes.push(child);
        } else if child.is_element() {
            text_nodes.extend(get_all_text_nodes(&child));
        }
    }
    text_nodes
}

/// `$importMultiline(lines, startLineIndex, multilineElementTransformers,
/// rootNode)`: whether a multiline element was imported, and the index of
/// the last line processed.
fn import_multiline(
    lines: &[String],
    start_line_index: usize,
    transformers: &[Rc<MultilineElementTransformer>],
    root_node: &NodeRef,
) -> Result<(bool, usize), LexicalError> {
    'transformers: for transformer in transformers {
        let Some(start_match) = transformer
            .reg_exp_start
            .string_match(&lines[start_line_index])
        else {
            continue;
        };
        if let Some(handle) = &transformer.handle_import_after_start_match {
            match handle(&MultilineImportArgs {
                lines,
                root_node,
                start_line_index,
                start_match: &start_match,
                transformer,
            })? {
                MultilineImportResult::Skip => continue,
                MultilineImportResult::Handled(imported, last) => return Ok((imported, last)),
                MultilineImportResult::Default => {}
            }
        }
        let regexp_end = transformer.reg_exp_end.as_ref().map(|e| &e.reg_exp);
        let is_end_optional = match &transformer.reg_exp_end {
            Some(end) => end.optional,
            None => true,
        };
        let mut end_line_index = start_line_index;
        let lines_length = lines.len();
        while end_line_index < lines_length {
            let end_match: Option<RegExpMatch> =
                regexp_end.and_then(|r| r.string_match(&lines[end_line_index]));
            if end_match.is_none() && (!is_end_optional || end_line_index < lines_length - 1) {
                end_line_index += 1;
                continue;
            }
            // A closing match at the opening one's place doesn't count.
            if let Some(end) = &end_match
                && start_line_index == end_line_index
                && end.index == start_match.index
            {
                end_line_index += 1;
                continue;
            }
            let mut lines_in_between = Vec::new();
            let start_len = start_match.match_len();
            if let Some(end) = end_match
                .as_ref()
                .filter(|_| start_line_index == end_line_index)
            {
                let line = units(&lines[start_line_index]);
                lines_in_between.push(slice(&line, start_len, slice_end(&line, end)));
            } else {
                for (i, text) in lines
                    .iter()
                    .enumerate()
                    .take(end_line_index + 1)
                    .skip(start_line_index)
                {
                    let line = units(text);
                    if i == start_line_index {
                        lines_in_between.push(slice(&line, start_len, line.len()));
                    } else if let Some(end) = end_match.as_ref().filter(|_| i == end_line_index) {
                        lines_in_between.push(slice(&line, 0, slice_end(&line, end)));
                    } else {
                        lines_in_between.push(text.clone());
                    }
                }
            }
            if (transformer.replace)(
                root_node,
                None,
                &start_match,
                end_match.as_ref(),
                Some(lines_in_between),
                true,
            )? {
                return Ok((true, end_line_index));
            }
            // The transformer declined: let the rest try from the start.
            continue 'transformers;
        }
    }
    Ok((false, start_line_index))
}

/// `-endMatch[0].length` as `slice`'s end: `-0` is 0, not the end.
fn slice_end(line: &[u16], end: &RegExpMatch) -> usize {
    match end.match_len() {
        0 => 0,
        n => line.len().saturating_sub(n),
    }
}

/// `text.slice(start, end)` with JS's clamping (an end before the start is
/// empty).
fn slice(text: &[u16], start: usize, end: usize) -> String {
    let start = start.min(text.len());
    let end = end.min(text.len());
    if end <= start {
        String::new()
    } else {
        string(&text[start..end])
    }
}

/// `$importBlocks(lineText, rootNode, elementTransformers,
/// textFormatTransformersIndex, textMatchTransformers,
/// shouldPreserveNewLines)`.
fn import_blocks(
    line_text: &str,
    root_node: &NodeRef,
    element_transformers: &[Rc<ElementTransformer>],
    index: &TextFormatTransformersIndex,
    text_match_transformers: &[Rc<TextMatchTransformer>],
    should_preserve_new_lines: bool,
) -> Result<(), LexicalError> {
    let text_node = create_text_node(line_text);
    let element_node = create_paragraph_node();
    element_node.append(std::slice::from_ref(&text_node))?;
    root_node.append(std::slice::from_ref(&element_node))?;
    for transformer in element_transformers {
        if let Some(m) = transformer.reg_exp.string_match(line_text) {
            text_node.set_text_content(&utf16::slice_from(line_text, m.match_len()));
            if (transformer.replace)(&element_node, std::slice::from_ref(&text_node), &m, true)? {
                break;
            }
        }
    }
    import_text_transformers(&text_node, index, text_match_transformers, None, None)?;

    // A paragraph no transformer replaced joins the paragraph, quote or
    // list before it.
    if element_node.get_parent().is_some() && !line_text.is_empty() {
        let previous_node = element_node.get_previous_sibling();
        if !should_preserve_new_lines
            && let Some(previous) = previous_node
                .filter(|p| is_paragraph_node(p) || is_quote_node(p) || is_list_node(p))
        {
            let target_node = if is_list_node(&previous) {
                previous
                    .get_last_descendant()
                    .and_then(|d| find_matching_parent(&d, is_list_item_node))
            } else {
                Some(previous)
            };
            if let Some(target) = target_node.filter(|t| t.get_text_content_size() > 0) {
                let mut nodes = vec![create_markdown_line_break_node(&target)?];
                nodes.extend(element_node.get_children());
                target.splice(target.get_children_size(), 0, &nodes)?;
                element_node.remove(false)?;
            }
        }
    }
    Ok(())
}

/// `$normalizeMarkdownTextNode(textNode)`: a tab node for each `\t`.
fn normalize_markdown_text_node(text_node: &NodeRef) -> Result<(), LexicalError> {
    if is_tab_node(text_node) {
        return Ok(());
    }
    let text = text_node.get_text_content();
    if !text.contains('\t') {
        return Ok(());
    }
    let u = units(&text);
    let data = text_node.text_data();
    let format = data.get_format();
    let style = data.get_style().to_owned();
    let mut nodes = Vec::new();
    let text_part = |s: &[u16]| {
        let node = create_text_node(&string(s));
        node.set_format(format);
        node.set_style(&style);
        node
    };
    let tab = [u16::from(b'\t')];
    let mut start = 0;
    let mut found = index_of(&u, &tab, 0);
    while let Some(index) = found {
        if index > start {
            nodes.push(text_part(&u[start..index]));
        }
        nodes.push(create_tab_node());
        start = index + 1;
        found = index_of(&u, &tab, index + 1);
    }
    if start < u.len() {
        nodes.push(text_part(&u[start..]));
    }
    let index = text_node.get_index_within_parent().unwrap_or(0);
    text_node.get_parent_or_throw()?.splice(index, 1, &nodes)
}

/// `createTextFormatTransformersIndex(textTransformers)`.
fn create_text_format_transformers_index(
    text_transformers: &[Rc<TextFormatTransformer>],
) -> TextFormatTransformersIndex {
    let mut index = TextFormatTransformersIndex::default();
    for transformer in text_transformers {
        match index
            .transformers_by_tag
            .iter_mut()
            .find(|(t, _)| *t == transformer.tag)
        {
            Some((_, t)) => *t = transformer.clone(),
            None => index
                .transformers_by_tag
                .push((transformer.tag.clone(), transformer.clone())),
        }
    }
    index
}

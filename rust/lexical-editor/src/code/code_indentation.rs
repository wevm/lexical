//! `CodeIndentation.ts`: `registerCodeIndentation`'s handlers of the
//! commands a model dispatches (`INSERT_TAB_COMMAND`,
//! `INDENT_CONTENT_COMMAND`, `OUTDENT_CONTENT_COMMAND`).
//!
//! Not ported: the handlers of `KEY_TAB_COMMAND`, the arrow keys (with
//! `$handleShiftLines` and the `escapeWithArrows` option), `MOVE_TO_START`
//! and `MOVE_TO_END` (`$handleMoveTo`), which read a `KeyboardEvent` and
//! wait for an input layer; `CodeIndentExtension`, which needs the
//! extension API.

use super::code_highlight_node::is_code_highlight_node;
use super::code_node::is_code_node;
use super::flat_structure_utils::{get_first_code_node_of_line, outdent_leading_spaces};
use crate::caret::{CaretDirection, get_caret_range, get_sibling_caret, get_text_point_caret};
use crate::caret_utils::{
    get_caret_range_in_direction, normalize_caret, set_selection_from_caret_range,
};
use crate::commands::{INDENT_CONTENT_COMMAND, INSERT_TAB_COMMAND, OUTDENT_CONTENT_COMMAND};
use crate::editor::COMMAND_PRIORITY_LOW;
use crate::error::invariant;
use crate::lexical_selection::{Point, PointType, SelectionRef, Which, get_selection};
use crate::node::NodeRef;
use crate::nodes::line_break::{LineBreakNode, create_line_break_node};
use crate::nodes::tab::{TabNode, create_tab_node};
use crate::selection_insert_nodes::insert_nodes;
use crate::{Editor, LexicalError, Unregister};

fn is_tab_node(node: &NodeRef) -> bool {
    node.get_latest().is::<TabNode>()
}

fn is_line_break_node(node: &NodeRef) -> bool {
    node.get_latest().is::<LineBreakNode>()
}

/// `$isSelectionInCode(selection)`: a range selection whose points are in
/// (or on) the same code block.
fn is_selection_in_code(selection: Option<&SelectionRef>) -> Result<bool, LexicalError> {
    let Some(range) = selection.and_then(SelectionRef::range) else {
        return Ok(false);
    };
    let code_of = |point: &Point| -> Result<Option<NodeRef>, LexicalError> {
        let node = point.get_node()?;
        Ok(if is_code_node(&node) {
            Some(node)
        } else {
            node.get_parent()
        })
    };
    let maybe_anchor_code_node = code_of(&range.anchor)?;
    let maybe_focus_code_node = code_of(&range.focus)?;
    Ok(maybe_anchor_code_node
        .as_ref()
        .is_some_and(|a| is_code_node(a) && maybe_focus_code_node.as_ref() == Some(a)))
}

/// `$getCodeLines(selection)`: the selected code highlight and tab nodes,
/// split at line breaks, without empty lines, and without the last line
/// when the selection ends at its start.
fn get_code_lines(selection: &SelectionRef) -> Result<Vec<Vec<NodeRef>>, LexicalError> {
    let nodes = selection.get_nodes()?;
    let mut lines = Vec::new();
    if nodes.len() == 1 && is_code_node(&nodes[0]) {
        return Ok(lines);
    }
    let mut last_line = Vec::new();
    for node in nodes {
        invariant!(
            is_code_highlight_node(&node) || is_tab_node(&node) || is_line_break_node(&node),
            "Expected selection to be inside CodeBlock and consisting of CodeHighlightNode, TabNode and LineBreakNode"
        );
        if is_line_break_node(&node) {
            if !last_line.is_empty() {
                lines.push(std::mem::take(&mut last_line));
            }
        } else {
            last_line.push(node);
        }
    }
    if let Some(first) = last_line.first() {
        let selection_end = selection
            .point(if selection.is_backward()? {
                Which::Anchor
            } else {
                Which::Focus
            })
            .expect("a range selection");
        // Discard the last line if the selection ends exactly at the
        // start of the line (no real selection)
        let last_point = Point::new(first.get_key(), 0, PointType::Text);
        if selection_end != last_point {
            lines.push(last_line);
        }
    }
    Ok(lines)
}

/// `$outdentLineAtCaret(selection, tabSize)`: removes the leading tab of
/// the caret's line, or with `tab_size` up to that many leading spaces.
fn outdent_line_at_caret(
    selection: &SelectionRef,
    tab_size: Option<usize>,
) -> Result<(), LexicalError> {
    let anchor_node = selection
        .point(Which::Anchor)
        .expect("a range selection")
        .get_node()?;
    // An element point (e.g. the caret on a blank line) has no line to outdent.
    if !is_code_highlight_node(&anchor_node) && !is_tab_node(&anchor_node) {
        return Ok(());
    }
    let first_of_line = get_first_code_node_of_line(&anchor_node);
    if is_tab_node(&first_of_line) {
        first_of_line.remove(false)?;
    } else if let Some(tab_size) = tab_size
        && is_code_highlight_node(&first_of_line)
    {
        outdent_leading_spaces(&first_of_line, tab_size, selection)?;
    }
    Ok(())
}

/// `$handleMultilineIndent(type, tabSize)`: a tab before each selected
/// line (`indent`), or one fewer (a tab, or with `tab_size` leading
/// spaces) at the start of each.
fn handle_multiline_indent(indent: bool, tab_size: Option<usize>) -> Result<bool, LexicalError> {
    let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
        return Ok(false);
    };
    if !is_selection_in_code(Some(&selection))? {
        return Ok(false);
    }
    let code_lines = get_code_lines(&selection)?;
    let code_lines_length = code_lines.len();

    // Special Indent case
    // Selection is collapsed at the beginning of a line
    let is_collapsed = selection.range().expect("a range selection").is_collapsed();
    if code_lines_length == 0 && is_collapsed {
        if indent {
            selection.insert_nodes(vec![create_tab_node()])?;
        } else {
            outdent_line_at_caret(&selection, tab_size)?;
        }
        return Ok(true);
    }

    // Special Indent case
    // Selection is matching only one LineBreak
    if code_lines_length == 0 && indent && selection.get_text_content()? == "\n" {
        let tab_node = create_tab_node();
        let line_break_node = create_line_break_node();
        let direction = if selection.is_backward()? {
            CaretDirection::Previous
        } else {
            CaretDirection::Next
        };
        selection.insert_nodes(vec![tab_node.clone(), line_break_node.clone()])?;
        set_selection_from_caret_range(&get_caret_range_in_direction(
            &get_caret_range(
                get_text_point_caret(&tab_node, CaretDirection::Next, 0)?,
                normalize_caret(&get_sibling_caret(&line_break_node, CaretDirection::Next)),
            )?,
            direction,
        )?)?;
        return Ok(true);
    }

    // Indent Non Empty Lines
    for (i, line) in code_lines.iter().enumerate() {
        // a line here is never empty
        let Some(first) = line.first() else {
            continue;
        };
        // make sure to consider the first node on the first line
        // because the line might not be fully selected
        let first_of_line = if i == 0 {
            get_first_code_node_of_line(first)
        } else {
            first.clone()
        };
        if indent {
            let tab_node = create_tab_node();
            first_of_line.insert_before(&tab_node, true)?;
            // First real code line may need selection adjustment
            // when firstOfLine is at the selection boundary
            if i == 0 {
                let anchor_key = if selection.is_backward()? {
                    Which::Focus
                } else {
                    Which::Anchor
                };
                let anchor_line = Point::new(first_of_line.get_key(), 0, PointType::Text);
                if selection.point(anchor_key).as_ref() == Some(&anchor_line) {
                    selection.point_set(anchor_key, tab_node.get_key(), 0, PointType::Text)?;
                }
            }
        } else if is_tab_node(&first_of_line) {
            first_of_line.remove(false)?;
        } else if let Some(tab_size) = tab_size
            && is_code_highlight_node(&first_of_line)
        {
            // Outdent space-indented lines (e.g. code formatted with prettier).
            outdent_leading_spaces(&first_of_line, tab_size, &selection)?;
        }
    }
    Ok(true)
}

/// `registerCodeIndentation(editor, tabSize)`'s model handlers:
/// `INSERT_TAB_COMMAND` inserts a tab in a code block, and
/// `INDENT_CONTENT_COMMAND` and `OUTDENT_CONTENT_COMMAND` indent and
/// outdent its selected lines, all at `COMMAND_PRIORITY_LOW`, before rich
/// text's handlers. With `tab_size`, outdenting also removes up to that
/// many leading spaces from a line. Returns what unregisters them.
pub fn register_code_indentation(editor: &Editor, tab_size: Option<usize>) -> Unregister {
    super::merge(vec![
        editor.register_command(
            &INSERT_TAB_COMMAND,
            |_, _| {
                let selection = get_selection();
                if !is_selection_in_code(selection.as_ref())? {
                    return Ok(false);
                }
                insert_nodes(vec![create_tab_node()])?;
                Ok(true)
            },
            COMMAND_PRIORITY_LOW,
        ),
        editor.register_command(
            &INDENT_CONTENT_COMMAND,
            |_, _| handle_multiline_indent(true, None),
            COMMAND_PRIORITY_LOW,
        ),
        editor.register_command(
            &OUTDENT_CONTENT_COMMAND,
            move |_, _| handle_multiline_indent(false, tab_size),
            COMMAND_PRIORITY_LOW,
        ),
    ])
}

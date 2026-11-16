//! `CodeIndentation.ts`: `registerCodeIndentation`'s handlers: Tab and
//! Shift+Tab (`$handleTab`), `INSERT_TAB_COMMAND`, `INDENT_CONTENT_COMMAND`
//! and `OUTDENT_CONTENT_COMMAND`, the up and down arrows (`$handleShiftLines`:
//! Alt moves the selected lines, and a caret at the edge of a code block
//! with nothing around it stays or leaves), the arrows that leave a code
//! block with `escapeWithArrows` (`$onEscapeUp`, `$onEscapeDown`), and
//! `MOVE_TO_START` and `MOVE_TO_END` (`$handleMoveTo`: Home and End move to
//! the start of the line's code and the end of the line).

use super::code_highlight_node::is_code_highlight_node;
use super::code_node::is_code_node;
use super::flat_structure_utils::{
    get_code_line_direction, get_end_of_code_in_line, get_first_code_node_of_line,
    get_last_code_node_of_line, get_start_of_code_in_line, outdent_leading_spaces,
};
use crate::caret::{CaretDirection, get_caret_range, get_sibling_caret, get_text_point_caret};
use crate::caret_utils::{
    get_caret_range_in_direction, normalize_caret, set_selection_from_caret_range,
};
use crate::commands::{
    INDENT_CONTENT_COMMAND, INSERT_TAB_COMMAND, KEY_ARROW_DOWN_COMMAND, KEY_ARROW_LEFT_COMMAND,
    KEY_ARROW_RIGHT_COMMAND, KEY_ARROW_UP_COMMAND, KEY_TAB_COMMAND, MOVE_TO_END, MOVE_TO_START,
    OUTDENT_CONTENT_COMMAND,
};
use crate::editor::COMMAND_PRIORITY_LOW;
use crate::editor::LexicalCommand;
use crate::error::invariant;
use crate::events::KeyboardEvent;
use crate::lexical_selection::{Point, PointType, SelectionRef, Which, get_selection};
use crate::node::NodeRef;
use crate::nodes::element::Direction;
use crate::nodes::line_break::{LineBreakNode, create_line_break_node};
use crate::nodes::tab::{TabNode, create_tab_node};
use crate::selection_insert_nodes::insert_nodes;
use crate::utils::{on_escape_down, on_escape_up};
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

/// `$handleTab(shiftKey)`: the command Tab (or Shift+Tab) stands for in a
/// code block: a tab or an outdent at a caret or inside a line, an indent
/// or outdent of the lines a selection spans; `None` outside one.
fn handle_tab(shift_key: bool) -> Result<Option<LexicalCommand<()>>, LexicalError> {
    let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
        return Ok(None);
    };
    if !is_selection_in_code(Some(&selection))? {
        return Ok(None);
    }
    let indent_or_outdent = if shift_key {
        OUTDENT_CONTENT_COMMAND
    } else {
        INDENT_CONTENT_COMMAND
    };
    let tab_or_outdent = if shift_key {
        OUTDENT_CONTENT_COMMAND
    } else {
        INSERT_TAB_COMMAND
    };
    let range = selection.range().expect("a range selection");
    let (anchor, focus) = (&range.anchor, &range.focus);

    // 1. early decision when there is no real selection
    if anchor == focus {
        return Ok(Some(tab_or_outdent));
    }

    // 2. If only empty lines or multiple non-empty lines are selected: indent/outdent
    let code_lines = get_code_lines(&selection)?;
    if code_lines.len() != 1 {
        return Ok(Some(indent_or_outdent));
    }
    let code_line = &code_lines[0];
    invariant!(
        !code_line.is_empty(),
        "$getCodeLines only extracts non-empty lines"
    );

    // Take into account the direction of the selection
    let (selection_first, selection_last) = if selection.is_backward()? {
        (focus, anchor)
    } else {
        (anchor, focus)
    };

    // find boundary elements of the line
    let first_of_line = get_first_code_node_of_line(&code_line[0]);
    let last_of_line = get_last_code_node_of_line(&code_line[0]);
    let anchor_of_line = Point::new(first_of_line.get_key(), 0, PointType::Text);
    let focus_of_line = Point::new(
        last_of_line.get_key(),
        last_of_line.get_text_content_size(),
        PointType::Text,
    );

    // 3. multiline because selection started strictly before the line
    if selection_first.is_before(&anchor_of_line)? {
        return Ok(Some(indent_or_outdent));
    }
    // 4. multiline because the selection stops strictly after the line
    if focus_of_line.is_before(selection_last)? {
        return Ok(Some(indent_or_outdent));
    }
    // The selection if within the line.
    // 4. If it does not touch both borders, it needs a tab
    if anchor_of_line.is_before(selection_first)? || selection_last.is_before(&focus_of_line)? {
        return Ok(Some(tab_or_outdent));
    }
    // 5. Selection is matching a full line on non-empty code
    Ok(Some(indent_or_outdent))
}

fn is_code_line_node(node: &NodeRef) -> bool {
    is_code_highlight_node(node) || is_tab_node(node)
}

/// `$handleShiftLines(type, event)`: Alt+Up and Alt+Down (`arrow_is_up`)
/// move the selected lines of a code block past the line above or below;
/// without Alt, a caret at the start (end) of a code block that is its
/// parent's first (last) child leaves it, the key going on.
fn handle_shift_lines(arrow_is_up: bool, event: &KeyboardEvent) -> Result<bool, LexicalError> {
    // We only care about the alt+arrow keys
    let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
        return Ok(false);
    };
    let range = selection.range().expect("a range selection");
    let (anchor_offset, focus_offset) = (range.anchor.offset, range.focus.offset);
    let anchor_node = range.anchor.get_node()?;
    let focus_node = range.focus.get_node()?;

    // Ensure the selection is within the codeblock
    if !is_selection_in_code(Some(&selection))?
        || !is_code_line_node(&anchor_node)
        || !is_code_line_node(&focus_node)
    {
        return Ok(false);
    }
    if !event.alt_key {
        // Handle moving selection out of the code block, given there are
        // no siblings that can natively take the selection.
        if range.is_collapsed() {
            let code_node = anchor_node.get_parent_or_throw()?;
            if arrow_is_up && anchor_offset == 0 && anchor_node.get_previous_sibling().is_none() {
                if code_node.get_previous_sibling().is_none() {
                    code_node.select_previous(None, None)?;
                    event.prevent_default();
                    return Ok(false);
                }
            } else if !arrow_is_up
                && anchor_offset == anchor_node.get_text_content_size()
                && anchor_node.get_next_sibling().is_none()
                && code_node.get_next_sibling().is_none()
            {
                code_node.select_next(None, None)?;
                event.prevent_default();
                return Ok(false);
            }
        }
        return Ok(false);
    }

    let (start, end) = if anchor_node.is_before(&focus_node) {
        (
            get_first_code_node_of_line(&anchor_node),
            get_last_code_node_of_line(&focus_node),
        )
    } else {
        (
            get_first_code_node_of_line(&focus_node),
            get_last_code_node_of_line(&anchor_node),
        )
    };
    let range_nodes = start.get_nodes_between(&end);
    if range_nodes
        .iter()
        .any(|node| !is_code_line_node(node) && !is_line_break_node(node))
    {
        return Ok(false);
    }

    // After this point, we know the selection is within the codeblock. We
    // may not be able to actually move the lines around, but we want to
    // return true either way to prevent the event's default behavior
    event.prevent_default();
    event.stop_propagation();

    let linebreak = if arrow_is_up {
        start.get_previous_sibling()
    } else {
        end.get_next_sibling()
    };
    let Some(linebreak) = linebreak.filter(is_line_break_node) else {
        return Ok(true);
    };
    let sibling = if arrow_is_up {
        linebreak.get_previous_sibling()
    } else {
        linebreak.get_next_sibling()
    };
    let Some(sibling) = sibling else {
        return Ok(true);
    };

    // A LineBreakNode sibling means the adjacent line is blank, so it has
    // no node of its own to anchor the move to: anchoring on it would
    // splice the moving line into the line past the blank one.
    let adjacent_line_is_blank = is_line_break_node(&sibling);
    let mut insertion_point = if !adjacent_line_is_blank && is_code_line_node(&sibling) {
        if arrow_is_up {
            get_first_code_node_of_line(&sibling)
        } else {
            get_last_code_node_of_line(&sibling)
        }
    } else {
        sibling
    };
    linebreak.remove(false)?;
    for node in &range_nodes {
        node.remove(false)?;
    }
    if adjacent_line_is_blank {
        // The blank line's position is immediately after the sibling
        // linebreak, in both directions.
        for node in &range_nodes {
            insertion_point.insert_after(node, true)?;
            insertion_point = node.clone();
        }
        insertion_point.insert_after(&linebreak, true)?;
    } else if arrow_is_up {
        for node in &range_nodes {
            insertion_point.insert_before(node, true)?;
        }
        insertion_point.insert_before(&linebreak, true)?;
    } else {
        insertion_point.insert_after(&linebreak, true)?;
        insertion_point = linebreak;
        for node in &range_nodes {
            insertion_point.insert_after(node, true)?;
            insertion_point = node.clone();
        }
    }
    selection.set_text_node_range(&anchor_node, anchor_offset, &focus_node, focus_offset)?;
    Ok(true)
}

/// `$handleMoveTo(type, event)`: `MOVE_TO_START` (`is_move_to_start`) and
/// `MOVE_TO_END` in a code block move the focus to the start of the line's
/// code (or of the line, or the block) and to the end of the line, the
/// other way in right-to-left code; with Shift the anchor stays.
fn handle_move_to(is_move_to_start: bool, event: &KeyboardEvent) -> Result<bool, LexicalError> {
    let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
        return Ok(false);
    };
    let range = selection.range().expect("a range selection");
    let (anchor, focus) = (&range.anchor, &range.focus);
    let anchor_node = anchor.get_node()?;
    let focus_node = focus.get_node()?;

    // Ensure the selection is within the codeblock
    if !is_selection_in_code(Some(&selection))?
        || !is_code_line_node(&anchor_node)
        || !is_code_line_node(&focus_node)
    {
        return Ok(false);
    }
    let focus_line_node = focus_node;
    let direction = get_code_line_direction(&focus_line_node);
    let move_to_start = if direction == Some(Direction::Rtl) {
        !is_move_to_start
    } else {
        is_move_to_start
    };

    // Shift variant: let the non-shift branches resolve the target, then
    // restore the original anchor so we end up with an extended selection.
    let original_anchor = anchor.clone();
    if move_to_start {
        match get_start_of_code_in_line(&focus_line_node, focus.offset)? {
            Some(start) => {
                if is_line_break_node(&start.node) {
                    start.node.select_next(Some(0), Some(0))?;
                } else {
                    selection.set_text_node_range(
                        &start.node,
                        start.offset,
                        &start.node,
                        start.offset,
                    )?;
                }
            }
            None => {
                focus_line_node.get_parent_or_throw()?.select_start()?;
            }
        }
    } else {
        get_end_of_code_in_line(&focus_line_node)?.select(None, None)?;
    }
    if event.shift_key {
        selection.point_set(
            Which::Anchor,
            &original_anchor.key,
            original_anchor.offset,
            original_anchor.point_type,
        )?;
    }
    event.prevent_default();
    event.stop_propagation();
    Ok(true)
}

/// `registerCodeIndentation(editor, tabSize, escapeWithArrows)`: the key
/// and command handlers of code-block indentation, all at
/// `COMMAND_PRIORITY_LOW`, before rich text's: Tab and Shift+Tab dispatch
/// the command `$handleTab` picks; `INSERT_TAB_COMMAND` inserts a tab in a
/// code block, and `INDENT_CONTENT_COMMAND` and `OUTDENT_CONTENT_COMMAND`
/// indent and outdent its selected lines (with `tab_size`, outdenting also
/// removes up to that many leading spaces); the up and down arrows keep a
/// caret at the very start or end of the document in the block, and
/// otherwise go to `$handleShiftLines`; `MOVE_TO_START` and `MOVE_TO_END`
/// go to `$handleMoveTo`. With `escape_with_arrows`, the arrows first leave
/// a code block at its edge by inserting a paragraph beside it. Returns
/// what unregisters them.
pub fn register_code_indentation(
    editor: &Editor,
    tab_size: Option<usize>,
    escape_with_arrows: bool,
) -> Unregister {
    let mut unregisters = Vec::new();
    if escape_with_arrows {
        // When node is the last child pressing down/right or up/left arrow
        // will insert paragraph below it to allow adding more content.
        // These handlers must be executed before $handleShiftLines
        unregisters.extend([
            editor.register_command(
                &KEY_ARROW_DOWN_COMMAND,
                |event, _| {
                    if event.alt_key {
                        return Ok(false);
                    }
                    on_escape_down(is_code_node, Some(event))
                },
                COMMAND_PRIORITY_LOW,
            ),
            editor.register_command(
                &KEY_ARROW_RIGHT_COMMAND,
                |event, _| on_escape_down(is_code_node, Some(event)),
                COMMAND_PRIORITY_LOW,
            ),
            editor.register_command(
                &KEY_ARROW_UP_COMMAND,
                |event, _| {
                    if event.alt_key {
                        return Ok(false);
                    }
                    on_escape_up(is_code_node, Some(event))
                },
                COMMAND_PRIORITY_LOW,
            ),
            editor.register_command(
                &KEY_ARROW_LEFT_COMMAND,
                |event, _| on_escape_up(is_code_node, Some(event)),
                COMMAND_PRIORITY_LOW,
            ),
        ]);
    }
    unregisters.extend([
        editor.register_command(
            &KEY_TAB_COMMAND,
            |event, editor| {
                let Some(command) = handle_tab(event.shift_key)? else {
                    return Ok(false);
                };
                event.prevent_default();
                editor.dispatch_command(&command, ())?;
                Ok(true)
            },
            COMMAND_PRIORITY_LOW,
        ),
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
        editor.register_command(
            &KEY_ARROW_UP_COMMAND,
            |event, _| {
                let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
                    return Ok(false);
                };
                let range = selection.range().expect("a range selection");
                let anchor_node = range.anchor.get_node()?;
                if !is_selection_in_code(Some(&selection))? {
                    return Ok(false);
                }
                // If at the start of a code block, prevent selection from
                // moving out
                let parent = anchor_node.get_parent();
                if range.is_collapsed()
                    && range.anchor.offset == 0
                    && anchor_node.get_previous_sibling().is_none()
                    && parent
                        .as_ref()
                        .is_some_and(|p| is_code_node(p) && p.get_previous_sibling().is_none())
                {
                    event.prevent_default();
                    return Ok(true);
                }
                handle_shift_lines(true, event)
            },
            COMMAND_PRIORITY_LOW,
        ),
        editor.register_command(
            &KEY_ARROW_DOWN_COMMAND,
            |event, _| {
                let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
                    return Ok(false);
                };
                let range = selection.range().expect("a range selection");
                let anchor_node = range.anchor.get_node()?;
                if !is_selection_in_code(Some(&selection))? {
                    return Ok(false);
                }
                // If at the end of a code block, prevent selection from
                // moving out
                if range.is_collapsed()
                    && range.anchor.offset == anchor_node.get_text_content_size()
                    && anchor_node.get_next_sibling().is_none()
                {
                    let parent = anchor_node.get_parent_or_throw()?;
                    if is_code_node(&parent) && parent.get_next_sibling().is_none() {
                        event.prevent_default();
                        return Ok(true);
                    }
                }
                handle_shift_lines(false, event)
            },
            COMMAND_PRIORITY_LOW,
        ),
        editor.register_command(
            &MOVE_TO_START,
            |event, _| handle_move_to(true, event),
            COMMAND_PRIORITY_LOW,
        ),
        editor.register_command(
            &MOVE_TO_END,
            |event, _| handle_move_to(false, event),
            COMMAND_PRIORITY_LOW,
        ),
    ]);
    super::merge(unregisters)
}

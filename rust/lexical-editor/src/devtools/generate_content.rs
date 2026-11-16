//! `packages/lexical-devtools-core/src/generateContent.ts`.
//!
//! Not ported: the `exportDOM` form, which needs `@lexical/html`; mark and
//! table nodes and table selections (`@lexical/mark`, `@lexical/table`); and
//! slots (`$getSlotNames`, `$resolveSlotContext`, `$slotValueMetaSuffix`),
//! which the core doesn't have, so a tree prints no slot entries. A headless
//! editor never composes, so no `compositionKey` line is printed.

use crate::constants::{IS_DIRECTIONLESS, TEXT_TYPE_TO_FORMAT, lookup};
use crate::node_state::{is_default, unparse};
use crate::nodes::paragraph::ParagraphNode;
use crate::nodes::text::to_int32;
use crate::selection_deletion::is_js_whitespace;
use crate::updates::read_editor_state;
use crate::{
    Editor, LexicalError, NodeRef, PointType, SelectionRef, get_root, get_selection, utf16,
};

/// `CustomPrintNodeFn`: what to print for a node (given whether to
/// obfuscate its text) instead of the default; `None` or an empty string
/// for the default.
pub type CustomPrintNodeFn<'a> = &'a dyn Fn(&NodeRef, bool) -> Option<String>;

/// An entry of `LexicalCommandLog` (`useLexicalCommandsLog.ts`): a
/// dispatched command's index in the log, its `type` and its payload as
/// the tree view prints it (JS prints `String(payload)`, or an event's
/// class name).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexicalCommandLogEntry {
    pub index: usize,
    pub command_type: String,
    pub payload: String,
}

/// `NON_SINGLE_WIDTH_CHARS_REPLACEMENT`.
const NON_SINGLE_WIDTH_CHARS_REPLACEMENT: [(char, &str); 2] = [('\t', "\\t"), ('\n', "\\n")];

/// `SYMBOLS`.
const ANCESTOR_HAS_NEXT_SIBLING: &str = "|";
const ANCESTOR_IS_LAST_CHILD: &str = " ";
const HAS_NEXT_SIBLING: &str = "├";
const IS_LAST_CHILD: &str = "└";
const SELECTED_CHAR: &str = "^";
const SELECTED_LINE: &str = ">";

/// The formats `FORMAT_PREDICATES` and `FORMAT_PREDICATES_PARAGRAPH` ask
/// about, in their order, with the names they print.
const FORMATS: [(&str, &str); 8] = [
    ("bold", "Bold"),
    ("code", "Code"),
    ("italic", "Italic"),
    ("strikethrough", "Strikethrough"),
    ("subscript", "Subscript"),
    ("superscript", "Superscript"),
    ("underline", "Underline"),
    ("highlight", "Highlight"),
];

/// `generateContent(editor, commandsLog, exportDOM, customPrintNode,
/// obfuscateText)`: the tree view's text: each node of the editor's state
/// on a line (`>` when selected, with a line of `^` under a text node's
/// selected characters), the selection, the commands logged and the
/// editor's version, namespace and editable flag. With `export_dom`, JS
/// prints the state's HTML instead, which needs `@lexical/html`, not ported:
/// that fails.
pub fn generate_content(
    editor: &Editor,
    commands_log: &[LexicalCommandLogEntry],
    export_dom: bool,
    custom_print_node: Option<CustomPrintNodeFn<'_>>,
    obfuscate_text: bool,
) -> Result<String, LexicalError> {
    let editor_state = editor.get_editor_state();

    if export_dom {
        return Err(LexicalError::new(
            "generateContent: exportDOM needs @lexical/html, which isn't ported",
        ));
    }

    let mut res = String::from(" root\n");

    let selection_string = read_editor_state(Some(editor), &editor_state, || {
        let selection = get_selection();
        visit_tree(&get_root(), &[], &mut |node, indent| {
            let node_key_display = format!("({})", node.get_key());
            let type_display = node.get_type();
            let is_selected = node.is_selected(None)?;
            res.push_str(&format!(
                "{} {} {node_key_display} {type_display} {}\n",
                if is_selected { SELECTED_LINE } else { " " },
                indent.join(" "),
                print_node(node, custom_print_node, obfuscate_text),
            ));
            res.push_str(&print_selected_chars_line(
                indent,
                is_selected,
                node,
                &node_key_display,
                selection.as_ref(),
                type_display,
            )?);
            Ok(())
        })?;
        Ok::<_, LexicalError>(match &selection {
            None => ": null".to_owned(),
            Some(selection) if selection.is_range() => print_range_selection(selection),
            Some(selection) => print_node_selection(selection),
        })
    })?;

    res.push_str("\n selection");
    res.push_str(&selection_string);

    res.push_str("\n\n commands:");

    if commands_log.is_empty() {
        res.push_str("\n  └ None dispatched.");
    } else {
        for entry in commands_log {
            res.push_str(&format!(
                "\n  └ {}. {{ type: {}, payload: {} }}",
                entry.index, entry.command_type, entry.payload
            ));
        }
    }
    res.push_str(&format!("\n\n editor (v{}):", Editor::VERSION));
    res.push_str(&format!("\n  └ namespace {}", editor.get_namespace()));
    res.push_str(&format!("\n  └ editable {}", editor.is_editable()));

    Ok(res)
}

/// `$printRangeSelection(selection)`.
fn print_range_selection(selection: &SelectionRef) -> String {
    let range = selection.range().expect("a range selection");
    let format_text = print_format_properties(|format| selection.has_format(format));
    let mut res = format!(
        ": range {} {}",
        if format_text.is_empty() {
            String::new()
        } else {
            format!("{{ {format_text} }}")
        },
        if range.style.is_empty() {
            String::new()
        } else {
            format!("{{ style: {} }} ", range.style)
        },
    );
    for (line, point) in [("├ anchor", &range.anchor), ("└ focus", &range.focus)] {
        res.push_str(&format!(
            "\n  {line} {{ key: {}, offset: {}, type: {} }}",
            point.key,
            point.offset,
            point.point_type.as_str()
        ));
    }
    res
}

/// `printNodeSelection(selection)`.
fn print_node_selection(selection: &SelectionRef) -> String {
    format!(": node\n  └ [{}]", selection.node_keys().join(", "))
}

/// What `$visitTree` calls for each node, with its indent.
type Visitor<'a> = dyn FnMut(&NodeRef, &[&'static str]) -> Result<(), LexicalError> + 'a;

/// `$visitTree(currentNode, visitor, indent)`: each child of an element,
/// with its indent, then its own children.
fn visit_tree(
    current_node: &NodeRef,
    indent: &[&'static str],
    visitor: &mut Visitor<'_>,
) -> Result<(), LexicalError> {
    let child_nodes = if current_node.is_element() {
        current_node.get_children()
    } else {
        Vec::new()
    };
    let total_count = child_nodes.len();
    for (i, child_node) in child_nodes.iter().enumerate() {
        let is_last = i + 1 == total_count;
        let mut child_indent = indent.to_vec();
        child_indent.push(if is_last {
            IS_LAST_CHILD
        } else {
            HAS_NEXT_SIBLING
        });
        visitor(child_node, &child_indent)?;
        if child_node.is_element() || child_node.is_decorator() {
            let mut descendant_indent = indent.to_vec();
            descendant_indent.push(if is_last {
                ANCESTOR_IS_LAST_CHILD
            } else {
                ANCESTOR_HAS_NEXT_SIBLING
            });
            visit_tree(child_node, &descendant_indent, visitor)?;
        }
    }
    Ok(())
}

/// `normalize(text, obfuscateText)`: tabs and newlines as escapes, and with
/// `obfuscate_text` every other code unit that isn't whitespace as `*`.
fn normalize(text: &str, obfuscate_text: bool) -> String {
    let mut text_to_print = text.to_owned();
    for (key, value) in NON_SINGLE_WIDTH_CHARS_REPLACEMENT {
        text_to_print = text_to_print.replace(key, value);
    }
    if obfuscate_text {
        // `/[^\s]/g` without `u` replaces each UTF-16 code unit.
        return text_to_print
            .chars()
            .flat_map(|c| {
                let n = if is_js_whitespace(c) {
                    0
                } else {
                    c.len_utf16()
                };
                std::iter::repeat_n('*', n).chain((n == 0).then_some(c))
            })
            .collect();
    }
    text_to_print
}

/// `[title, properties].filter(Boolean).join(' ').trim()` with the
/// properties in braces.
fn title_and_properties(title: String, properties: &str) -> String {
    let joined = if properties.is_empty() {
        title
    } else {
        format!("{title} {{ {properties} }}")
    };
    joined.trim_matches(is_js_whitespace).to_owned()
}

/// `printNode(node, customPrintNode, obfuscateText)`.
fn print_node(
    node: &NodeRef,
    custom_print_node: Option<CustomPrintNodeFn<'_>>,
    obfuscate_text: bool,
) -> String {
    if let Some(custom_print) = custom_print_node
        .and_then(|custom| custom(node, obfuscate_text))
        .filter(|custom| !custom.is_empty())
    {
        return custom_print;
    }

    if node.is_text() {
        let text = node.get_text_content();
        let title = if text.is_empty() {
            "(empty)".to_owned()
        } else {
            format!("\"{}\"", normalize(&text, obfuscate_text))
        };
        return title_and_properties(title, &print_all_text_node_properties(node));
    }
    #[cfg(feature = "link")]
    if crate::link::is_link_node(node) {
        let link = node.get_url().unwrap_or_default();
        let title = if link.is_empty() {
            "(empty)".to_owned()
        } else {
            format!("\"{}\"", normalize(&link, obfuscate_text))
        };
        return title_and_properties(title, &print_all_link_node_properties(node));
    }
    let latest = node.get_latest();
    if latest.is::<ParagraphNode>() {
        let element = latest.element().expect("a paragraph is an element");
        let format_text = print_format_properties(|format| {
            let flag = lookup(TEXT_TYPE_TO_FORMAT, format).unwrap_or(0);
            to_int32(element.get_text_format()) & flag != 0
        });
        let mut paragraph_data = if format_text.is_empty() {
            String::new()
        } else {
            format!("{{ {format_text} }}")
        };
        if !element.get_style().is_empty() {
            paragraph_data.push_str(&format!("({})", element.get_style()));
        }
        return paragraph_data;
    }
    String::new()
}

/// `printAllTextNodeProperties(node)`.
fn print_all_text_node_properties(node: &NodeRef) -> String {
    [
        print_format_properties(|format| node.has_format(format)),
        print_detail_properties(node),
        print_mode_properties(node),
        print_state_properties(node),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(", ")
}

/// `printAllLinkNodeProperties(node)`, with `printTargetProperties`,
/// `printRelProperties` and `printTitleProperties` (a `null` property is
/// left out; an empty one is printed).
#[cfg(feature = "link")]
fn print_all_link_node_properties(node: &NodeRef) -> String {
    [
        node.get_target().map(|s| format!("target: {s}")),
        node.get_rel().map(|s| format!("rel: {s}")),
        node.get_title().map(|s| format!("title: {s}")),
        Some(print_state_properties(node)),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(", ")
}

/// `predicates.map(...).filter(Boolean).join(', ').toLocaleLowerCase()`
/// with `label: ` before it when not empty.
fn print_properties(label: &str, names: &[&str]) -> String {
    if names.is_empty() {
        return String::new();
    }
    format!("{label}: {}", names.join(", ").to_lowercase())
}

/// `printDetailProperties(node)`.
fn print_detail_properties(node: &NodeRef) -> String {
    let detail = to_int32(node.text_data().get_detail());
    let mut names = Vec::new();
    if detail & IS_DIRECTIONLESS != 0 {
        names.push("Directionless");
    }
    if node.is_unmergeable() {
        names.push("Unmergeable");
    }
    print_properties("detail", &names)
}

/// `printModeProperties(node)`.
fn print_mode_properties(node: &NodeRef) -> String {
    let mut names = Vec::new();
    if node.is_token() {
        names.push("Token");
    }
    if node.is_segmented() {
        names.push("Segmented");
    }
    print_properties("mode", &names)
}

/// `printFormatProperties(nodeOrSelection)` and
/// `printTextFormatProperties(paragraph)`, by what `hasFormat` or
/// `hasTextFormat` answers.
fn print_format_properties(has_format: impl Fn(&str) -> bool) -> String {
    let names: Vec<&str> = FORMATS
        .iter()
        .filter(|(format, _)| has_format(format))
        .map(|(_, name)| *name)
        .collect();
    print_properties("format", &names)
}

/// `printStateProperties(node)`: the node's known NodeState values that
/// aren't their default, as `JSON.stringify` writes their `unparse`.
fn print_state_properties(node: &NodeRef) -> String {
    let Some(state) = node.get_latest().base().state.clone() else {
        return String::new();
    };
    let states: Vec<String> = state
        .borrow()
        .known_entries()
        .iter()
        .filter(|(config, value)| !is_default(config, value))
        .map(|(config, value)| {
            let unparsed = unparse(config, value);
            let text_value = if unparsed.is_undefined() {
                "undefined".to_owned()
            } else {
                unparsed.to_string()
            };
            format!("[{}: {text_value}]", config.key())
        })
        .collect();
    let joined = states.join(",");
    if joined.is_empty() {
        return joined;
    }
    format!("state: {joined}")
}

/// `$printSelectedCharsLine(...)`: under a selected text node, a line with
/// `^` under its selected characters, or nothing.
fn print_selected_chars_line(
    indent: &[&str],
    is_selected: bool,
    node: &NodeRef,
    node_key_display: &str,
    selection: Option<&SelectionRef>,
    type_display: &str,
) -> Result<String, LexicalError> {
    // No selection or node is not selected.
    let Some(range) = selection.and_then(SelectionRef::range) else {
        return Ok(String::new());
    };
    if !node.is_text() || !is_selected || node.is_element() {
        return Ok(String::new());
    }

    // No selected characters.
    let anchor = &range.anchor;
    let focus = &range.focus;

    if node.get_text_content().is_empty()
        || (anchor.key == focus.key && anchor.offset == focus.offset)
    {
        return Ok(String::new());
    }

    let (start, end) = get_selection_start_end(node, anchor, focus)?;

    if start == end {
        return Ok(String::new());
    }

    let selection_last_indent = if indent.last() == Some(&HAS_NEXT_SIBLING) {
        ANCESTOR_HAS_NEXT_SIBLING
    } else {
        ANCESTOR_IS_LAST_CHILD
    };

    let mut indention_chars = indent[..indent.len().saturating_sub(1)].to_vec();
    indention_chars.push(selection_last_indent);
    // `Array(n)` throws for a negative length.
    let (Ok(unselected), Ok(selected)) = (usize::try_from(start + 1), usize::try_from(end - start))
    else {
        return Err(LexicalError::new("Invalid array length"));
    };
    // 1 for the space after + 1 for the double quote.
    let padding_length = utf16::len(type_display) + 2;
    let node_print_spaces = utf16::len(node_key_display) + padding_length;

    Ok(format!(
        "{SELECTED_LINE} {} {}{}\n",
        indention_chars.join(" "),
        " ".repeat(node_print_spaces + unselected),
        SELECTED_CHAR.repeat(selected),
    ))
}

/// `String#slice(start, end)` on UTF-16 code units: clamped, and a negative
/// index counts from the end.
fn js_slice(units: &[u16], start: i64, end: i64) -> &[u16] {
    let n = units.len() as i64;
    let clamp = |i: i64| (if i < 0 { (n + i).max(0) } else { i.min(n) }) as usize;
    let (a, b) = (clamp(start), clamp(end));
    if a < b { &units[a..b] } else { &[] }
}

/// `$getSelectionStartEnd(node, selection)`: the selected UTF-16 range of
/// `node`'s text, in printed characters (a tab or newline prints as two).
fn get_selection_start_end(
    node: &NodeRef,
    anchor: &crate::Point,
    focus: &crate::Point,
) -> Result<(i64, i64), LexicalError> {
    let text_content = node.get_text_content();
    let units: Vec<u16> = text_content.encode_utf16().collect();
    let text_length = units.len() as i64;

    let mut start = -1;
    let mut end = -1;

    // Only one node is being selected.
    if anchor.point_type == PointType::Text && focus.point_type == PointType::Text {
        let anchor_node = anchor.get_node()?;
        let focus_node = focus.get_node()?;
        let (anchor_offset, focus_offset) = (anchor.offset as i64, focus.offset as i64);

        if anchor_node == focus_node && node == &anchor_node && anchor_offset != focus_offset {
            (start, end) = if anchor_offset < focus_offset {
                (anchor_offset, focus_offset)
            } else {
                (focus_offset, anchor_offset)
            };
        } else if node == &anchor_node {
            (start, end) = if anchor_node.is_before(&focus_node) {
                (anchor_offset, text_length)
            } else {
                (0, anchor_offset)
            };
        } else if node == &focus_node {
            (start, end) = if focus_node.is_before(&anchor_node) {
                (focus_offset, text_length)
            } else {
                (0, focus_offset)
            };
        } else {
            // Node is within selection but not the anchor nor focus.
            (start, end) = (0, text_length);
        }
    }

    // Account for non-single width characters.
    let is_wide = |unit: &&u16| {
        NON_SINGLE_WIDTH_CHARS_REPLACEMENT
            .iter()
            .any(|(c, _)| u32::from(**unit) == u32::from(*c))
    };
    let before = js_slice(&units, 0, start).iter().filter(is_wide).count() as i64;
    let inside = js_slice(&units, start, end).iter().filter(is_wide).count() as i64;

    Ok((start + before, end + before + inside))
}

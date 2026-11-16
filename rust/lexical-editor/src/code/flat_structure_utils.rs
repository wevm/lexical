//! `FlatStructureUtils.ts`: the lines of a code block, which are the runs
//! of [`CodeHighlightNode`](super::CodeHighlightNode)s and tabs between its
//! line breaks.

use super::code_highlight_node::{create_code_highlight_node, is_code_highlight_node};
use crate::LexicalError;
use crate::caret::{CaretDirection, get_sibling_caret};
use crate::error::invariant;
use crate::lexical_selection::{PointType, SelectionRef, Which};
use crate::lexical_utils::get_text_direction;
use crate::node::NodeRef;
use crate::nodes::element::Direction;
use crate::nodes::line_break::{LineBreakNode, create_line_break_node};
use crate::nodes::tab::{TabNode, create_tab_node};
use crate::selection_insert_nodes::{RawTextVisitor, tokenize_raw_text};

const SPACE: u16 = b' ' as u16;

fn is_tab_node(node: &NodeRef) -> bool {
    node.get_latest().is::<TabNode>()
}

fn is_line_break_node(node: &NodeRef) -> bool {
    node.get_latest().is::<LineBreakNode>()
}

fn units(node: &NodeRef) -> Vec<u16> {
    node.get_text_content().encode_utf16().collect()
}

/// `$getLastMatchingCodeNode(anchor, direction)`: the farthest node from
/// `anchor` in `direction` across a run of code highlight and tab nodes,
/// starting from `anchor` itself; `anchor` when it isn't one.
fn get_last_matching_code_node(anchor: &NodeRef, direction: CaretDirection) -> NodeRef {
    let mut matching_node = anchor.clone();
    let mut caret = Some(get_sibling_caret(anchor, direction));
    while let Some(c) =
        caret.filter(|c| is_code_highlight_node(c.origin()) || is_tab_node(c.origin()))
    {
        matching_node = c.origin().clone();
        caret = c.get_adjacent_caret();
    }
    matching_node
}

/// `$getFirstCodeNodeOfLine(anchor)`.
pub fn get_first_code_node_of_line(anchor: &NodeRef) -> NodeRef {
    get_last_matching_code_node(anchor, CaretDirection::Previous)
}

/// `$getLastCodeNodeOfLine(anchor)`.
pub fn get_last_code_node_of_line(anchor: &NodeRef) -> NodeRef {
    get_last_matching_code_node(anchor, CaretDirection::Next)
}

/// `$getCodeLineDirection(anchor)`: the direction of the first strongly
/// directional character of the line's code highlight nodes, else the code
/// block's own direction, else `None`.
pub fn get_code_line_direction(anchor: &NodeRef) -> Option<Direction> {
    let start = get_first_code_node_of_line(anchor);
    let end = get_last_code_node_of_line(anchor);
    let mut node = Some(start.clone());
    while let Some(n) = node {
        if is_code_highlight_node(&n)
            && let Some(direction) = get_text_direction(&n.get_text_content())
        {
            return Some(direction);
        }
        if n == end {
            break;
        }
        node = n.get_next_sibling();
    }
    start
        .get_parent()
        .filter(NodeRef::is_element)
        .and_then(|parent| parent.element_data().get_direction())
}

/// A point `$getStartOfCodeInLine` returns: `{node, offset}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeLinePoint {
    pub node: NodeRef,
    pub offset: usize,
}

/// `$getStartOfCodeInLine(anchor, offset)`: where the code of the line
/// starts, past its leading spaces and tabs (the first non-blank character
/// before the anchor when there is one), the line break before the line
/// (offset 1), or `None` at the start of the block. Offsets are UTF-16 code
/// units.
pub fn get_start_of_code_in_line(
    anchor: &NodeRef,
    offset: usize,
) -> Result<Option<CodeLinePoint>, LexicalError> {
    let mut last = None;
    let mut last_non_blank = None;
    let mut node = anchor.clone();
    let mut node_offset = offset;
    let mut node_text_content = units(anchor);
    loop {
        if node_offset == 0 {
            let Some(prev_sibling) = node.get_previous_sibling() else {
                break;
            };
            invariant!(
                is_code_highlight_node(&prev_sibling)
                    || is_tab_node(&prev_sibling)
                    || is_line_break_node(&prev_sibling),
                "Expected a valid Code Node: CodeHighlightNode, TabNode, LineBreakNode"
            );
            node = prev_sibling;
            if is_line_break_node(&node) {
                last = Some(CodeLinePoint { node, offset: 1 });
                break;
            }
            node_offset = node.get_text_content_size().saturating_sub(1);
            node_text_content = units(&node);
        } else {
            node_offset -= 1;
        }
        let character = node_text_content.get(node_offset);
        if is_code_highlight_node(&node) && character != Some(&SPACE) {
            last_non_blank = Some(CodeLinePoint {
                node: node.clone(),
                offset: node_offset,
            });
        }
    }
    // lastNonBlank !== null: anchor in the middle of code; move to line beginning
    if last_non_blank.is_some() {
        return Ok(last_non_blank);
    }
    // Spaces, tabs or nothing ahead of anchor. `None` is JS's `null`, and
    // `Some(None)` the `undefined` of reading past a text's end.
    let mut code_character_at_anchor_offset: Option<Option<u16>> = None;
    if offset < anchor.get_text_content_size() {
        if is_code_highlight_node(anchor) {
            code_character_at_anchor_offset = Some(units(anchor).get(offset).copied());
        }
    } else if let Some(next_sibling) = anchor.get_next_sibling().filter(is_code_highlight_node) {
        code_character_at_anchor_offset = Some(units(&next_sibling).first().copied());
    }
    if code_character_at_anchor_offset.is_some()
        && code_character_at_anchor_offset != Some(Some(SPACE))
    {
        // Borderline whitespace and code, move to line beginning
        Ok(last)
    } else {
        Ok(find_next_non_blank_in_line(anchor, offset).or(last))
    }
}

/// `findNextNonBlankInLine(anchor, offset)`.
fn find_next_non_blank_in_line(anchor: &NodeRef, offset: usize) -> Option<CodeLinePoint> {
    let mut node = anchor.clone();
    let mut node_offset = offset;
    let mut node_text_content = units(anchor);
    let mut node_text_content_size = anchor.get_text_content_size();
    loop {
        if !is_code_highlight_node(&node) || node_offset == node_text_content_size {
            node = node.get_next_sibling()?;
            if is_line_break_node(&node) {
                return None;
            }
            if is_code_highlight_node(&node) {
                node_offset = 0;
                node_text_content = units(&node);
                node_text_content_size = node.get_text_content_size();
            }
        }
        if is_code_highlight_node(&node) {
            if node_text_content.get(node_offset) != Some(&SPACE) {
                return Some(CodeLinePoint {
                    node,
                    offset: node_offset,
                });
            }
            node_offset += 1;
        }
    }
}

/// `$getEndOfCodeInLine(anchor)`: the last node of the line.
pub fn get_end_of_code_in_line(anchor: &NodeRef) -> Result<NodeRef, LexicalError> {
    let last_node = get_last_code_node_of_line(anchor);
    invariant!(
        !is_line_break_node(&last_node),
        "Unexpected lineBreakNode in getEndOfCodeInLine"
    );
    Ok(last_node)
}

struct Plainify(Vec<NodeRef>);

impl RawTextVisitor for Plainify {
    fn linebreak(&mut self) {
        self.0.push(create_line_break_node());
    }
    fn tab(&mut self) {
        self.0.push(create_tab_node());
    }
    fn text(&mut self, text: &str) {
        self.0.push(create_code_highlight_node(text, None));
    }
}

/// `$plainifyCodeContent(text)`: `text` as code highlight nodes with no
/// highlight type, line breaks and tabs (`tokenizeRawText`).
pub fn plainify_code_content(text: &str) -> Vec<NodeRef> {
    let mut out = Plainify(Vec::new());
    tokenize_raw_text(text, &mut out);
    out.0
}

/// `$outdentLeadingSpaces(node, tabSize, selection)`: removes up to
/// `tab_size` leading spaces from the code highlight node that starts a
/// line, and moves the range selection's points in it back by as many
/// (not before 0). Returns whether it removed any. JS takes any number and
/// does nothing unless it is a positive integer; here it is a `usize`, and
/// 0 does nothing.
pub fn outdent_leading_spaces(
    node: &NodeRef,
    tab_size: usize,
    selection: &SelectionRef,
) -> Result<bool, LexicalError> {
    if tab_size == 0 {
        return Ok(false);
    }
    let text = node.get_text_content();
    let leading = text.chars().take_while(|&c| c == ' ').count();
    if leading == 0 {
        return Ok(false);
    }
    let strip_count = tab_size.min(leading);
    let line_key = node.get_key().to_owned();
    let old_offset = |which| {
        selection
            .point(which)
            .filter(|p| p.key == line_key && p.point_type == PointType::Text)
            .map(|p| p.offset)
    };
    let old_anchor_offset = old_offset(Which::Anchor);
    let old_focus_offset = old_offset(Which::Focus);
    node.splice_text(0, strip_count as i64, "", false)?;
    for (which, old) in [
        (Which::Anchor, old_anchor_offset),
        (Which::Focus, old_focus_offset),
    ] {
        if let Some(old) = old {
            selection.point_set(
                which,
                &line_key,
                old.saturating_sub(strip_count),
                PointType::Text,
            )?;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::{code_nodes, create_code_node};
    use crate::lexical_selection::{create_range_selection, set_selection};
    use crate::{CreateEditorArgs, UpdateOptions, create_editor, get_root, reset_random_key};

    /// What JS's `getTextDirection` returns, recorded from Node 24.
    #[test]
    fn get_text_direction_matches_js() {
        let cases = [
            ("", None),
            ("123 ", None),
            ("abc", Some(Direction::Ltr)),
            (" שלום a", Some(Direction::Rtl)),
            ("1 a שלום", Some(Direction::Ltr)),
            ("😀", Some(Direction::Ltr)),
            ("\u{FEFC}", Some(Direction::Rtl)),
            ("\u{FEFD}", Some(Direction::Ltr)),
            ("— ٣", Some(Direction::Rtl)),
        ];
        for (text, want) in cases {
            assert_eq!(get_text_direction(text), want, "{text:?}");
        }
    }

    /// A node's key, `$getStartOfCodeInLine` at each of its offsets,
    /// `$getEndOfCodeInLine` and `$getCodeLineDirection`.
    type LineCase = (
        &'static str,
        Vec<Option<(String, usize)>>,
        &'static str,
        Option<Direction>,
    );

    /// The line functions on one code block, against what JS's return
    /// (recorded from Node 24 with the same keys): a tab, `"  ab"`, `" "`,
    /// `"cd"`, a line break, `"   "`, a line break, `"x  y"` and `""`.
    #[test]
    fn line_functions_match_js() {
        reset_random_key();
        let editor = create_editor(CreateEditorArgs {
            nodes: code_nodes(),
        });
        editor
            .update(
                || {
                    let code = create_code_node(Some("js"), None);
                    let h = |text| create_code_highlight_node(text, None);
                    code.append(&[
                        create_tab_node(),
                        h("  ab"),
                        h(" "),
                        h("cd"),
                        create_line_break_node(),
                        h("   "),
                        create_line_break_node(),
                        h("x  y"),
                        h(""),
                    ])?;
                    get_root().append(std::slice::from_ref(&code))?;
                    let all = code.get_children();
                    let keys: Vec<_> = all.iter().map(|n| n.get_key().to_owned()).collect();
                    assert_eq!(keys, ["2", "3", "4", "5", "6", "7", "8", "9", "10"]);

                    let point = |p: Option<CodeLinePoint>| {
                        p.map(|p| (p.node.get_key().to_owned(), p.offset))
                    };
                    let some = |k: &str, o| Some((k.to_owned(), o));
                    // Key, then `$getStartOfCodeInLine` at each offset, `$getEndOfCodeInLine`
                    // and `$getCodeLineDirection`.
                    let want: [LineCase; 7] = [
                        (
                            "2",
                            vec![some("3", 2), some("3", 2)],
                            "5",
                            Some(Direction::Ltr),
                        ),
                        (
                            "3",
                            vec![some("3", 2), some("3", 2), None, some("3", 2), some("3", 2)],
                            "5",
                            Some(Direction::Ltr),
                        ),
                        (
                            "4",
                            vec![some("3", 2), some("3", 2)],
                            "5",
                            Some(Direction::Ltr),
                        ),
                        (
                            "5",
                            vec![some("3", 2), some("3", 2), some("3", 2)],
                            "5",
                            Some(Direction::Ltr),
                        ),
                        ("7", vec![some("6", 1); 4], "7", None),
                        (
                            "9",
                            vec![
                                some("8", 1),
                                some("9", 0),
                                some("9", 0),
                                some("9", 0),
                                some("9", 0),
                            ],
                            "10",
                            Some(Direction::Ltr),
                        ),
                        ("10", vec![some("9", 0)], "10", Some(Direction::Ltr)),
                    ];
                    for (key, starts, end, direction) in want {
                        let node = all.iter().find(|n| n.get_key() == key).expect("a key");
                        let got: Vec<_> = (0..starts.len())
                            .map(|o| get_start_of_code_in_line(node, o).map(point))
                            .collect::<Result<_, _>>()?;
                        assert_eq!(got, starts, "start of {key}");
                        assert_eq!(
                            get_end_of_code_in_line(node)?.get_key(),
                            end,
                            "end of {key}"
                        );
                        assert_eq!(
                            get_code_line_direction(node),
                            direction,
                            "direction of {key}"
                        );
                    }

                    let plain: Vec<_> = plainify_code_content("a\tb\r\nc\n\n")
                        .iter()
                        .map(|n| (n.get_type(), n.get_text_content()))
                        .collect();
                    let want: Vec<_> = [
                        ("code-highlight", "a"),
                        ("tab", "\t"),
                        ("code-highlight", "b"),
                        ("linebreak", "\n"),
                        ("code-highlight", "c"),
                        ("linebreak", "\n"),
                        ("linebreak", "\n"),
                    ]
                    .into_iter()
                    .map(|(t, s)| (t, s.to_owned()))
                    .collect();
                    assert_eq!(plain, want);

                    let spaces = &all[5];
                    let selection = create_range_selection();
                    selection.point_set(Which::Anchor, spaces.get_key(), 3, PointType::Text)?;
                    selection.point_set(Which::Focus, spaces.get_key(), 1, PointType::Text)?;
                    set_selection(Some(selection.clone()));
                    let offsets = || {
                        (
                            selection.point(Which::Anchor).unwrap().offset,
                            selection.point(Which::Focus).unwrap().offset,
                        )
                    };
                    assert!(outdent_leading_spaces(spaces, 2, &selection)?);
                    assert_eq!(
                        (spaces.get_text_content(), offsets()),
                        (" ".to_owned(), (1, 0))
                    );
                    assert!(!outdent_leading_spaces(spaces, 0, &selection)?);
                    assert!(!outdent_leading_spaces(&all[7], 4, &selection)?);
                    assert!(outdent_leading_spaces(spaces, 4, &selection)?);
                    assert_eq!(
                        (spaces.get_text_content(), offsets()),
                        (String::new(), (0, 0))
                    );
                    Ok(())
                },
                UpdateOptions {
                    discrete: true,
                    ..UpdateOptions::default()
                },
            )
            .unwrap();
    }
}

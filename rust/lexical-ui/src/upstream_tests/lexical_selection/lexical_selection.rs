//! Ports of
//! `packages/lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx`.
//!
//! Upstream's first `describe` renders a React `RichTextPlugin` editor
//! (with `HistoryPlugin`, `ListPlugin` and `MarkdownShortcutPlugin`) into
//! jsdom; [`Fixture`] builds the same editor with the `register*`
//! functions those plugins call, mounted, with the selection the initial
//! native selection (`[0, 0]`, offset 0: the empty paragraph's managed
//! `<br>`) resolves to, the paragraph's start. Where upstream reads the
//! container's `innerHTML`, these render the committed document as the
//! reconciler does ([`rendered_html`]). The table-driven `suite` (#1 to
//! #138), which drives and asserts the DOM selection, is skipped.

#![cfg(all(
    feature = "extension",
    feature = "html",
    feature = "link",
    feature = "list",
    feature = "markdown",
    feature = "rich-text"
))]

use super::support::{rendered_html, set_anchor_point, set_focus_point};
use crate::extension::build_editor_from_extensions;
use crate::link::{LinkAttributes, create_link_node, link_extension};
use crate::list::{ListType, create_list_item_node, create_list_node};
use crate::nodes::element::ElementData;
use crate::nodes::line_break::create_line_break_node;
use crate::rich_text::{HeadingTagType, create_heading_node, rich_text_extension};
use crate::selection::{
    StylePatchTarget, StylePatchValue, get_selection_style_value_for_property, patch_style_text,
    set_blocks_type,
};
use crate::upstream_tests::support::{
    create_test_decorator_node, create_test_editor, create_test_element_node, panic_message,
    set_test_root_element, update, update_with,
};
use crate::{
    AnyLexicalExtensionArgument, CreateEditorArgs, Editor, InitialEditorStateType, LexicalError,
    LexicalExtension, LexicalNode, NodeBase, NodeClass, NodeRef, PointType, SelectionRef, Which,
    create_paragraph_node, create_range_selection, create_text_node, define_extension, get_root,
    get_selection, get_slot, set_selection, set_slot,
};
use std::cell::RefCell;
use std::rc::Rc;

use PointType::{Element as E, Text as T};

const FILE: &str = "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx";

const ROOT_OPEN: &str = "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\">";

const THEME: &str = r#"{"code":"editor-code","heading":{"h1":"editor-heading-h1","h2":"editor-heading-h2","h3":"editor-heading-h3","h4":"editor-heading-h4","h5":"editor-heading-h5","h6":"editor-heading-h6"},"image":"editor-image","list":{"ol":"editor-list-ol","ul":"editor-list-ul"},"listitem":"editor-listitem","paragraph":"editor-paragraph","quote":"editor-quote","text":{"bold":"editor-text-bold","code":"editor-text-code","hashtag":"editor-text-hashtag","italic":"editor-text-italic","link":"editor-text-link","strikethrough":"editor-text-strikethrough","underline":"editor-text-underline","underlineStrikethrough":"editor-text-underlineStrikethrough"}}"#;

/// `Selection.modify` as the upstream selection test utils' polyfill does
/// it for the points these tests delete from: an element point (a caret in
/// an empty block) doesn't move.
struct PolyfillMovement;

impl crate::SelectionMovement for PolyfillMovement {
    fn landed_point(
        &self,
        _from: &crate::Point,
        _is_backward: bool,
        _granularity: crate::SelectionGranularity,
    ) -> Result<Option<crate::Point>, LexicalError> {
        Ok(None)
    }
}

/// The editor upstream's `TestBase` renders.
struct Fixture {
    editor: Editor,
    _unregister: Vec<crate::Unregister>,
}

impl Fixture {
    fn new() -> Self {
        let editor = create_test_editor(CreateEditorArgs {
            theme: crate::json::parse(THEME)
                .unwrap()
                .as_object()
                .unwrap()
                .clone(),
            ..CreateEditorArgs::default()
        });
        let unregister = vec![
            // `RichTextPlugin`.
            crate::rich_text::register_rich_text(&editor),
            // `HistoryPlugin`, whose default delay is 1000 ms.
            crate::history::register_history(
                &editor,
                &crate::history::create_empty_history_state(),
                1000.0,
                None,
                None,
                None,
            ),
            // `ListPlugin`.
            crate::list::register_list(&editor, crate::list::RegisterListOptions::default()),
            // `MarkdownShortcutPlugin transformers={[UNORDERED_LIST]}`.
            crate::markdown::register_markdown_shortcuts(
                &editor,
                &[crate::markdown::Transformer::Element(
                    crate::markdown::unordered_list(),
                )],
            )
            .unwrap(),
        ];
        editor.set_selection_movement(Some(Rc::new(PolyfillMovement)));
        set_test_root_element(&editor);
        // `LexicalComposer`'s initial paragraph, then the native selection
        // at its start.
        update(&editor, || {
            let paragraph = create_paragraph_node();
            get_root().append(std::slice::from_ref(&paragraph))?;
            paragraph.select(Some(0), Some(0))?;
            Ok(())
        });
        Self {
            editor,
            _unregister: unregister,
        }
    }

    /// `container.innerHTML`, once `act` has flushed the updates the
    /// listeners queued.
    fn html(&self) -> String {
        self.editor.read(|| ()).unwrap();
        format!("{ROOT_OPEN}{}</div>", rendered_html(&self.editor))
    }

    /// One input of `applySelectionInputs`, in its own update.
    fn apply(&self, f: impl FnOnce(&SelectionRef) -> Result<(), LexicalError> + 'static) {
        update(&self.editor, move || f(&get_selection().unwrap()));
    }
}

fn current() -> SelectionRef {
    get_selection()
        .filter(SelectionRef::is_range)
        .expect("a range selection")
}

fn first_paragraph() -> NodeRef {
    let paragraph = get_root().get_first_child().unwrap();
    assert_eq!(paragraph.get_type(), "paragraph");
    paragraph
}

fn paragraph_of(children: &[NodeRef]) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(children).unwrap();
    paragraph
}

fn element_of(element: NodeRef, children: &[NodeRef]) -> NodeRef {
    element.append(children).unwrap();
    element
}

fn bold(text: &str) -> NodeRef {
    let node = create_text_node(text);
    node.toggle_format("bold");
    node
}

fn h1() -> NodeRef {
    create_heading_node(HeadingTagType::H1)
}

fn root_types() -> Vec<&'static str> {
    get_root()
        .get_children()
        .iter()
        .map(NodeRef::get_type)
        .collect()
}

/// The ids marked above the test named `test_fn` in this file.
fn markers_of(test_fn: &str) -> Vec<String> {
    let source = include_str!("lexical_selection.rs");
    let lines: Vec<&str> = source.lines().collect();
    let at = lines
        .iter()
        .position(|l| *l == format!("fn {test_fn}() {{"))
        .expect("the test function");
    let mut start = at;
    while start > 0 && !lines[start - 1].is_empty() {
        start -= 1;
    }
    lines[start..at]
        .iter()
        .filter_map(|l| l.strip_prefix("// upstream: "))
        .map(str::to_owned)
        .collect()
}

/// Runs each named case, fails listing the cases that failed, and checks
/// the ids the cases stand for are exactly the markers of `test_fn`.
fn run_cases(test_fn: &str, describe: &str, cases: Vec<(String, Box<dyn FnOnce()>)>) {
    let mut ids = Vec::new();
    let mut failed = Vec::new();
    for (name, case) in cases {
        if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(case)) {
            failed.push(format!("{name}: {}", panic_message(&*payload)));
        }
        ids.push(format!("{FILE}::{describe} > {name}"));
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
    assert_eq!(markers_of(test_fn), ids);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Expect initial output to be a block with no text.
#[test]
fn expect_initial_output_to_be_a_block_with_no_text() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.html(),
        format!(
            "{ROOT_OPEN}<p class=\"editor-paragraph\" dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p></div>"
        )
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Bold format preserved when typing between consecutive line breaks
#[test]
fn bold_format_preserved_when_typing_between_consecutive_line_breaks() {
    let fixture = Fixture::new();
    fixture.apply(|s| s.format_text("bold", None));
    fixture.apply(|s| s.insert_text("hello"));
    // Move cursor between "he" and "llo"
    update(&fixture.editor, || {
        let paragraph = get_root().get_first_child_or_throw()?;
        assert!(paragraph.is_element());
        let text_node = paragraph.get_first_child_or_throw()?;
        assert!(text_node.is_text());
        text_node.select(Some(2), Some(2))?;
        Ok(())
    });
    // Shift+Enter twice (logical line breaks)
    update(&fixture.editor, || {
        if let Some(selection) = get_selection().filter(SelectionRef::is_range) {
            selection.insert_line_break(false)?;
            selection.insert_line_break(false)?;
        }
        Ok(())
    });
    // Move cursor between the two line breaks in the paragraph
    update(&fixture.editor, || {
        let paragraph = get_root().get_first_child_or_throw()?;
        let children = paragraph.get_children();
        let between = (0..children.len().saturating_sub(1))
            .find(|&i| {
                children[i].get_type() == "linebreak" && children[i + 1].get_type() == "linebreak"
            })
            .map(|i| i + 1)
            .expect("Expected to find consecutive line breaks");
        paragraph.select(Some(between), Some(between))?;
        Ok(())
    });
    fixture.apply(|s| s.insert_text("x"));
    fixture
        .editor
        .read_with_mode(crate::EditorReadMode::Latest, || {
            let x_node = get_root()
                .get_all_text_nodes()
                .into_iter()
                .find(|node| node.get_text_content() == "x")
                .expect("an x node");
            assert!(x_node.has_format("bold"));
        })
        .unwrap();
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Ctrl+Backspace deletes list created by typing "- "
#[test]
fn ctrl_backspace_deletes_list_created_by_typing_dash_space() {
    let fixture = Fixture::new();
    let delete_word_backward =
        || fixture.apply(|s| s.range().map_or(Ok(()), |_| s.delete_word(true)));
    fixture.apply(|s| s.insert_text("-"));
    fixture.apply(|s| s.insert_text(" "));
    let empty_li = |value: u32| {
        format!("<li value=\"{value}\"><br data-lexical-managed-linebreak=\"true\"></li>")
    };
    assert_eq!(
        fixture.html(),
        format!(
            "{ROOT_OPEN}<ul class=\"editor-list-ul\" dir=\"auto\">{}</ul></div>",
            empty_li(1)
        )
    );
    delete_word_backward();
    assert_eq!(
        fixture.html(),
        format!(
            "{ROOT_OPEN}<p class=\"editor-paragraph\" dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p></div>"
        )
    );
    fixture.apply(|s| s.insert_text("preceding paragraph"));
    fixture.apply(|s| s.insert_paragraph().map(|_| ()));
    fixture.apply(|s| s.insert_text("-"));
    fixture.apply(|s| s.insert_text(" "));
    fixture.apply(|s| s.insert_paragraph().map(|_| ()));
    let preceding = "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">preceding paragraph</span></p>";
    assert_eq!(
        fixture.html(),
        format!(
            "{ROOT_OPEN}{preceding}<ul class=\"editor-list-ul\" dir=\"auto\">{}{}</ul></div>",
            empty_li(1),
            empty_li(2)
        )
    );
    delete_word_backward();
    assert_eq!(
        fixture.html(),
        format!(
            "{ROOT_OPEN}{preceding}<ul class=\"editor-list-ul\" dir=\"auto\">{}</ul></div>",
            empty_li(1)
        )
    );
    delete_word_backward();
    assert_eq!(fixture.html(), format!("{ROOT_OPEN}{preceding}</div>"));
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > insert text one selected node element selection
#[test]
fn insert_text_one_selected_node_element_selection() {
    let fixture = Fixture::new();
    update(&fixture.editor, || {
        let paragraph = first_paragraph();
        let element_node = create_test_element_node();
        let text = create_text_node("foo");
        paragraph.append(std::slice::from_ref(&element_node))?;
        element_node.append(std::slice::from_ref(&text))?;
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, text.get_key(), 0, T)?;
        selection.point_set(Which::Focus, paragraph.get_key(), 1, E)?;
        selection.insert_text("")?;
        assert_eq!(get_root().get_text_content(), "");
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > getNodes resolves nested block nodes
#[test]
fn get_nodes_resolves_nested_block_nodes() {
    let fixture = Fixture::new();
    update(&fixture.editor, || {
        let paragraph = first_paragraph();
        let element_node = create_test_element_node();
        let text = create_text_node("");
        paragraph.append(std::slice::from_ref(&element_node))?;
        element_node.append(std::slice::from_ref(&text))?;
        let selected_nodes = get_selection().unwrap().get_nodes()?;
        assert_eq!(selected_nodes.len(), 1);
        assert_eq!(selected_nodes[0].get_key(), text.get_key());
        Ok(())
    });
}

// LexicalSelection tests > Block selection moves when new nodes are inserted

/// What a case expects: anchor node and offset, focus node and offset.
type Expected = (NodeRef, usize, NodeRef, usize);

/// A `baseCases` entry's `fnBefore`.
#[derive(Clone, Copy)]
enum Setup {
    None,
    /// `originalText1.insertBefore($createTextNode('bar'))`.
    BarBefore,
    /// `paragraph.append($createTextNode('bar'))`.
    BarAppended,
}

struct BlockCase {
    name: &'static str,
    anchor_offset: usize,
    focus_offset: usize,
    before: Setup,
    f: fn(&NodeRef, &NodeRef) -> Expected,
}

fn text(t: &str) -> NodeRef {
    create_text_node(t)
}

fn block_cases() -> Vec<BlockCase> {
    use Setup::{BarAppended, BarBefore, None as No};
    vec![
        // Collapsed selection on end; add/remove/replace beginning
        BlockCase {
            name: "insertBefore - Collapsed selection on end; add beginning",
            anchor_offset: 2,
            focus_offset: 2,
            before: No,
            f: |p, t| {
                t.insert_before(&text("2"), true).unwrap();
                (p.clone(), 3, p.clone(), 3)
            },
        },
        BlockCase {
            name: "insertAfter - Collapsed selection on end; add beginning",
            anchor_offset: 2,
            focus_offset: 2,
            before: No,
            f: |p, t| {
                t.insert_after(&text("2"), true).unwrap();
                (p.clone(), 3, p.clone(), 3)
            },
        },
        BlockCase {
            name: "splitText - Collapsed selection on end; add beginning",
            anchor_offset: 2,
            focus_offset: 2,
            before: No,
            f: |p, t| {
                t.split_text(&[1]).unwrap();
                (p.clone(), 3, p.clone(), 3)
            },
        },
        BlockCase {
            name: "remove - Collapsed selection on end; add beginning",
            anchor_offset: 1,
            focus_offset: 1,
            before: No,
            f: |p, t| {
                t.remove(false).unwrap();
                (p.clone(), 0, p.clone(), 0)
            },
        },
        BlockCase {
            name: "replace - Collapsed selection on end; replace beginning",
            anchor_offset: 1,
            focus_offset: 1,
            before: No,
            f: |p, t| {
                t.replace(&text("replacement"), false).unwrap();
                (p.clone(), 1, p.clone(), 1)
            },
        },
        // All selected; add/remove/replace on beginning
        BlockCase {
            name: "insertBefore - All selected; add on beginning",
            anchor_offset: 0,
            focus_offset: 2,
            before: No,
            f: |p, t| {
                t.insert_before(&text("2"), true).unwrap();
                (t.clone(), 0, p.clone(), 3)
            },
        },
        BlockCase {
            name: "splitNodes - All selected; add on beginning",
            anchor_offset: 0,
            focus_offset: 2,
            before: No,
            f: |p, original| {
                let t = original.split_text(&[1]).unwrap()[1].clone();
                (t, 0, p.clone(), 3)
            },
        },
        BlockCase {
            name: "remove - All selected; remove on beginning",
            anchor_offset: 0,
            focus_offset: 1,
            before: No,
            f: |p, t| {
                t.remove(false).unwrap();
                (p.clone(), 0, p.clone(), 0)
            },
        },
        BlockCase {
            name: "replace - All selected; replace on beginning",
            anchor_offset: 0,
            focus_offset: 1,
            before: No,
            f: |p, t| {
                t.replace(&text("replacement"), false).unwrap();
                (p.clone(), 0, p.clone(), 1)
            },
        },
        // Selection beginning; add/remove/replace on end
        BlockCase {
            name: "insertBefore - Selection beginning; add on end",
            anchor_offset: 0,
            focus_offset: 1,
            before: BarBefore,
            f: |p, t1| {
                let t2 = t1.get_previous_sibling().unwrap();
                p.get_last_child()
                    .unwrap()
                    .insert_before(&text("2"), true)
                    .unwrap();
                (t2, 0, t1.clone(), 0)
            },
        },
        BlockCase {
            name: "insertAfter - Selection beginning; add on end",
            anchor_offset: 0,
            focus_offset: 1,
            before: No,
            f: |p, t| {
                p.get_last_child()
                    .unwrap()
                    .insert_after(&text("2"), true)
                    .unwrap();
                (t.clone(), 0, p.clone(), 1)
            },
        },
        BlockCase {
            name: "splitText - Selection beginning; add on end",
            anchor_offset: 0,
            focus_offset: 1,
            before: BarBefore,
            f: |_, t1| {
                let t2 = t1.get_previous_sibling().unwrap();
                let t = t1.split_text(&[1]).unwrap()[1].clone();
                (t2, 0, t, 0)
            },
        },
        BlockCase {
            name: "remove - Selection beginning; remove on end",
            anchor_offset: 0,
            focus_offset: 1,
            before: No,
            f: |p, t| {
                p.get_last_child().unwrap().remove(false).unwrap();
                (t.clone(), 0, t.clone(), 3)
            },
        },
        BlockCase {
            name: "replace - Selection beginning; replace on end",
            anchor_offset: 0,
            focus_offset: 1,
            before: No,
            f: |p, _| {
                p.get_last_child()
                    .unwrap()
                    .replace(&text("replacement"), false)
                    .unwrap();
                (p.clone(), 0, p.clone(), 1)
            },
        },
        // All selected; add/remove/replace in end offset [1, 2] -> [1, N, 2]
        BlockCase {
            name: "insertBefore - All selected; add in end offset",
            anchor_offset: 0,
            focus_offset: 1,
            before: No,
            f: |p, t| {
                p.get_last_child()
                    .unwrap()
                    .insert_before(&text("2"), true)
                    .unwrap();
                (t.clone(), 0, p.clone(), 2)
            },
        },
        BlockCase {
            name: "insertAfter - All selected; add in end offset",
            anchor_offset: 0,
            focus_offset: 1,
            before: No,
            f: |p, t| {
                t.insert_after(&text("2"), true).unwrap();
                (t.clone(), 0, p.clone(), 2)
            },
        },
        BlockCase {
            name: "splitText - All selected; add in end offset",
            anchor_offset: 0,
            focus_offset: 1,
            before: BarBefore,
            f: |_, t1| {
                let t2 = t1.get_previous_sibling().unwrap();
                let t = t1.split_text(&[1]).unwrap()[1].clone();
                (t2, 0, t, 0)
            },
        },
        BlockCase {
            name: "remove - All selected; remove in end offset",
            anchor_offset: 1,
            focus_offset: 2,
            before: BarBefore,
            f: |p, t1| {
                p.get_last_child().unwrap().remove(false).unwrap();
                (t1.clone(), 0, t1.clone(), 3)
            },
        },
        BlockCase {
            name: "replace - All selected; replace in end offset",
            anchor_offset: 1,
            focus_offset: 2,
            before: BarBefore,
            f: |p, _| {
                p.get_last_child()
                    .unwrap()
                    .replace(&text("replacement"), false)
                    .unwrap();
                (p.clone(), 1, p.clone(), 2)
            },
        },
        // All selected; add/remove/replace in middle [1, 2, 3] -> [1, 2, N, 3]
        BlockCase {
            name: "insertBefore - All selected; add in middle",
            anchor_offset: 0,
            focus_offset: 2,
            before: BarBefore,
            f: |p, t1| {
                let t2 = t1.get_previous_sibling().unwrap();
                p.get_last_child()
                    .unwrap()
                    .insert_before(&text("2"), true)
                    .unwrap();
                (t2, 0, p.clone(), 3)
            },
        },
        BlockCase {
            name: "insertAfter - All selected; add in middle",
            anchor_offset: 0,
            focus_offset: 2,
            before: BarBefore,
            f: |p, t1| {
                let t2 = t1.get_previous_sibling().unwrap();
                t1.insert_after(&text("2"), true).unwrap();
                (t2, 0, p.clone(), 3)
            },
        },
        BlockCase {
            name: "splitText - All selected; add in middle",
            anchor_offset: 0,
            focus_offset: 2,
            before: BarBefore,
            f: |p, t1| {
                let t2 = t1.get_previous_sibling().unwrap();
                t1.split_text(&[1]).unwrap();
                (t2, 0, p.clone(), 3)
            },
        },
        BlockCase {
            name: "remove - All selected; remove in middle",
            anchor_offset: 0,
            focus_offset: 2,
            before: BarBefore,
            f: |p, t1| {
                let t2 = t1.get_previous_sibling().unwrap();
                t1.remove(false).unwrap();
                (t2, 0, p.clone(), 1)
            },
        },
        BlockCase {
            name: "replace - All selected; replace in middle",
            anchor_offset: 0,
            focus_offset: 2,
            before: BarBefore,
            f: |p, t1| {
                t1.replace(&text("replacement"), false).unwrap();
                (p.clone(), 0, p.clone(), 2)
            },
        },
        // Edge cases
        BlockCase {
            name: "Selection resolves to the end of text node when it's at the end (1)",
            anchor_offset: 3,
            focus_offset: 3,
            before: BarAppended,
            f: |p, t1| {
                let t2 = p.get_last_child().unwrap();
                t1.insert_before(&text("new"), true).unwrap();
                (t2.clone(), 3, t2, 3)
            },
        },
        BlockCase {
            name: "Selection resolves to the end of text node when it's at the end (2)",
            anchor_offset: 0,
            focus_offset: 3,
            before: BarAppended,
            f: |p, t1| {
                let t2 = p.get_last_child().unwrap();
                t1.insert_before(&text("new"), true).unwrap();
                (t1.clone(), 0, t2, 3)
            },
        },
        BlockCase {
            name: "remove - Remove with collapsed selection at offset #4221",
            anchor_offset: 1,
            focus_offset: 1,
            before: No,
            f: |_, t1| {
                t1.get_next_sibling().unwrap().remove(false).unwrap();
                (t1.clone(), 3, t1.clone(), 3)
            },
        },
        BlockCase {
            name: "remove - Remove with non-collapsed selection at offset",
            anchor_offset: 0,
            focus_offset: 1,
            before: No,
            f: |_, t1| {
                t1.get_next_sibling().unwrap().remove(false).unwrap();
                (t1.clone(), 0, t1.clone(), 3)
            },
        },
    ]
}

fn block_selection_case(case: &BlockCase, invert: bool) {
    let fixture = Fixture::new();
    let (anchor_offset, focus_offset) = if invert {
        (case.focus_offset, case.anchor_offset)
    } else {
        (case.anchor_offset, case.focus_offset)
    };
    let (before, f) = (case.before, case.f);
    update(&fixture.editor, move || {
        let paragraph = first_paragraph();
        let text_node = create_text_node("foo");
        // Note: line break can't be selected by the DOM
        let linebreak = create_line_break_node();
        let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
            return Ok(());
        };
        paragraph.append(&[text_node.clone(), linebreak])?;
        match before {
            Setup::None => {}
            Setup::BarBefore => {
                text_node.insert_before(&create_text_node("bar"), true)?;
            }
            Setup::BarAppended => paragraph.append(&[create_text_node("bar")])?,
        }
        selection.point_set(Which::Anchor, paragraph.get_key(), anchor_offset, E)?;
        selection.point_set(Which::Focus, paragraph.get_key(), focus_offset, E)?;
        let (expected_anchor, expected_anchor_offset, expected_focus, expected_focus_offset) =
            f(&paragraph, &text_node);
        let range = selection.range().unwrap();
        let (anchor, focus) = if invert {
            (range.focus, range.anchor)
        } else {
            (range.anchor, range.focus)
        };
        assert_eq!(anchor.key, expected_anchor.get_key());
        assert_eq!(anchor.offset, expected_anchor_offset);
        assert_eq!(focus.key, expected_focus.get_key());
        assert_eq!(focus.offset, expected_focus_offset);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - Collapsed selection on end; add beginning
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - Collapsed selection on end; add beginning (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - Collapsed selection on end; add beginning
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - Collapsed selection on end; add beginning (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - Collapsed selection on end; add beginning
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - Collapsed selection on end; add beginning (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Collapsed selection on end; add beginning
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Collapsed selection on end; add beginning (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - Collapsed selection on end; replace beginning
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - Collapsed selection on end; replace beginning (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add on beginning
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add on beginning (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitNodes - All selected; add on beginning
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitNodes - All selected; add on beginning (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove on beginning
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove on beginning (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace on beginning
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace on beginning (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - Selection beginning; add on end
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - Selection beginning; add on end (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - Selection beginning; add on end
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - Selection beginning; add on end (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - Selection beginning; add on end
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - Selection beginning; add on end (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Selection beginning; remove on end
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Selection beginning; remove on end (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - Selection beginning; replace on end
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - Selection beginning; replace on end (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add in end offset
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add in end offset (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - All selected; add in end offset
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - All selected; add in end offset (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - All selected; add in end offset
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - All selected; add in end offset (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove in end offset
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove in end offset (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace in end offset
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace in end offset (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add in middle
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertBefore - All selected; add in middle (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - All selected; add in middle
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > insertAfter - All selected; add in middle (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - All selected; add in middle
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > splitText - All selected; add in middle (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove in middle
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - All selected; remove in middle (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace in middle
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > replace - All selected; replace in middle (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > Selection resolves to the end of text node when it's at the end (1)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > Selection resolves to the end of text node when it's at the end (1) (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > Selection resolves to the end of text node when it's at the end (2)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > Selection resolves to the end of text node when it's at the end (2) (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Remove with collapsed selection at offset #4221
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Remove with collapsed selection at offset #4221 (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Remove with non-collapsed selection at offset
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Block selection moves when new nodes are inserted > remove - Remove with non-collapsed selection at offset (inverse selection)
#[test]
fn block_selection_moves_when_new_nodes_are_inserted() {
    let mut cases: Vec<(String, Box<dyn FnOnce()>)> = Vec::new();
    for case in block_cases() {
        let case = Rc::new(case);
        for invert in [false, true] {
            let name = if invert {
                format!("{} (inverse selection)", case.name)
            } else {
                case.name.to_owned()
            };
            let case = case.clone();
            cases.push((name, Box::new(move || block_selection_case(&case, invert))));
        }
    }
    run_cases(
        "block_selection_moves_when_new_nodes_are_inserted",
        "LexicalSelection tests > Block selection moves when new nodes are inserted",
        cases,
    );
}

// upstream: "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Selection correctly resolves to a sibling ElementNode when a node is removed > "
#[test]
fn selection_correctly_resolves_to_a_sibling_element_node_when_a_node_is_removed() {
    let fixture = Fixture::new();
    update(&fixture.editor, || {
        let list_node = create_list_node(ListType::Bullet, 1.0);
        let list_item_node = create_list_item_node(None);
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&list_node))?;
        list_node.append(std::slice::from_ref(&list_item_node))?;
        list_item_node.select(None, None)?;
        list_node.insert_after(&paragraph, true)?;
        list_item_node.remove(false)?;
        let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
            return Ok(());
        };
        assert_eq!(
            selection
                .point(Which::Anchor)
                .unwrap()
                .get_node()?
                .get_type(),
            "paragraph"
        );
        assert_eq!(
            selection
                .point(Which::Focus)
                .unwrap()
                .get_node()?
                .get_type(),
            "paragraph"
        );
        Ok(())
    });
}

// upstream: "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Selection correctly resolves to a sibling ElementNode when a selected node child is removed > "
#[test]
fn selection_correctly_resolves_to_a_sibling_element_node_when_a_selected_node_child_is_removed() {
    let fixture = Fixture::new();
    let paragraph_node_key = update_with(&fixture.editor, || {
        let paragraph_node = create_paragraph_node();
        let list_node = create_list_node(ListType::Number, 1.0);
        let list_item_node1 = create_list_item_node(None);
        let text_node1 = create_text_node("foo");
        let list_item_node2 = create_list_item_node(None);
        let list_node2 = create_list_node(ListType::Number, 1.0);
        let list_item_node2x1 = create_list_item_node(None);
        list_node.append(&[list_item_node1.clone(), list_item_node2.clone()])?;
        list_item_node1.append(&[text_node1])?;
        list_item_node2.append(std::slice::from_ref(&list_node2))?;
        list_node2.append(&[list_item_node2x1])?;
        get_root().append(&[paragraph_node.clone(), list_node.clone()])?;
        list_item_node2.select(None, None)?;
        list_node.remove(false)?;
        Ok(paragraph_node.get_key().to_owned())
    });
    fixture
        .editor
        .read_with_mode(crate::EditorReadMode::Latest, || {
            let range = current().range().unwrap();
            assert_eq!(range.anchor.key, paragraph_node_key);
            assert_eq!(range.focus.key, paragraph_node_key);
        })
        .unwrap();
}

// upstream: "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Selection correctly resolves to a sibling ElementNode that has multiple children with the correct offset when a node is removed > "
#[test]
fn selection_correctly_resolves_to_a_sibling_element_node_that_has_multiple_children_with_the_correct_offset()
 {
    let fixture = Fixture::new();
    update(&fixture.editor, || {
        // Root
        //  |- Paragraph
        //    |- Link
        //      |- Text
        //      |- LineBreak
        //      |- Text
        //    |- Text
        let paragraph = create_paragraph_node();
        let link = create_link_node("bullet", &LinkAttributes::default());
        let text_three = create_text_node(" ");
        get_root().append(std::slice::from_ref(&paragraph))?;
        link.append(&[create_text_node("Hello")])?;
        link.append(&[create_line_break_node()])?;
        link.append(&[create_text_node("world")])?;
        paragraph.append(std::slice::from_ref(&link))?;
        paragraph.append(std::slice::from_ref(&text_three))?;
        text_three.select(None, None)?;
        // Act
        text_three.remove(false)?;
        // Assert
        let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
            return Ok(());
        };
        let range = selection.range().unwrap();
        assert_eq!(range.anchor.get_node()?.get_key(), link.get_key());
        assert_eq!(range.focus.get_node()?.get_key(), link.get_key());
        assert_eq!(range.anchor.offset, 3);
        assert_eq!(range.focus.offset, 3);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > isBackward
#[test]
fn is_backward() {
    let fixture = Fixture::new();
    update(&fixture.editor, || {
        let paragraph = first_paragraph();
        let paragraph_key = paragraph.get_key().to_owned();
        let text_node = create_text_node("foo");
        let text_node_key = text_node.get_key().to_owned();
        // Note: line break can't be selected by the DOM
        let linebreak = create_line_break_node();
        let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
            return Ok(());
        };
        paragraph.append(&[text_node, linebreak])?;
        let set = |a: (&str, usize, PointType), f: (&str, usize, PointType)| {
            selection.point_set(Which::Anchor, a.0, a.1, a.2).unwrap();
            selection.point_set(Which::Focus, f.0, f.1, f.2).unwrap();
            selection.is_backward().unwrap()
        };
        assert!(!set((&text_node_key, 0, T), (&text_node_key, 0, T)));
        assert!(!set((&paragraph_key, 1, E), (&paragraph_key, 1, E)));
        assert!(!set((&paragraph_key, 0, E), (&paragraph_key, 1, E)));
        assert!(set((&paragraph_key, 1, E), (&paragraph_key, 0, E)));
        Ok(())
    });
}

// LexicalSelection tests > Decorator text content for selection

/// The nodes a `Decorator text content` case sets its points on.
struct DecoratorCase {
    paragraph: NodeRef,
    text_node1: NodeRef,
    text_node2: NodeRef,
    decorator: NodeRef,
}

type DecoratorFn = fn(&DecoratorCase, &dyn Fn(Which, &NodeRef, usize, PointType)) -> String;

fn decorator_cases() -> Vec<(&'static str, DecoratorFn)> {
    vec![
        ("Not included if cursor right before it", |c, set| {
            set(Which::Anchor, &c.text_node1, 1, T);
            set(Which::Focus, &c.text_node1, 1, T);
            String::new()
        }),
        ("Not included if cursor right after it", |c, set| {
            set(Which::Anchor, &c.text_node2, 0, T);
            set(Which::Focus, &c.text_node2, 0, T);
            String::new()
        }),
        ("Included if decorator is selected within text", |c, set| {
            set(Which::Anchor, &c.text_node1, 1, T);
            set(Which::Focus, &c.text_node2, 0, T);
            c.decorator.get_text_content()
        }),
        (
            "Included if decorator is selected with another node before it",
            |c, set| {
                set(Which::Anchor, &c.text_node1, 0, T);
                set(Which::Focus, &c.text_node2, 0, T);
                c.text_node1.get_text_content() + &c.decorator.get_text_content()
            },
        ),
        (
            "Included if decorator is selected with another node after it",
            |c, set| {
                set(Which::Anchor, &c.text_node1, 1, T);
                set(Which::Focus, &c.text_node2, 1, T);
                c.decorator.get_text_content() + &c.text_node2.get_text_content()
            },
        ),
        (
            "Included if decorator is selected as the only node",
            |c, set| {
                c.text_node1.remove(false).unwrap();
                c.text_node2.remove(false).unwrap();
                set(Which::Anchor, &c.paragraph, 0, E);
                set(Which::Focus, &c.paragraph, 1, E);
                c.decorator.get_text_content()
            },
        ),
    ]
}

fn decorator_text_content_case(f: DecoratorFn, invert: bool) {
    let fixture = Fixture::new();
    update(&fixture.editor, move || {
        let paragraph = first_paragraph();
        let case = DecoratorCase {
            paragraph: paragraph.clone(),
            text_node1: create_text_node("1"),
            text_node2: create_text_node("2"),
            decorator: create_test_decorator_node(),
        };
        paragraph.append(&[
            case.text_node1.clone(),
            case.decorator.clone(),
            case.text_node2.clone(),
        ])?;
        let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
            return Ok(());
        };
        let set = |which: Which, node: &NodeRef, offset: usize, point_type: PointType| {
            let which = match (which, invert) {
                (Which::Anchor, true) => Which::Focus,
                (Which::Focus, true) => Which::Anchor,
                (which, false) => which,
            };
            selection
                .point_set(which, node.get_key(), offset, point_type)
                .unwrap();
        };
        let expected_text_content = f(&case, &set);
        assert_eq!(selection.get_text_content()?, expected_text_content);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Not included if cursor right before it
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Not included if cursor right before it (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Not included if cursor right after it
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Not included if cursor right after it (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected within text
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected within text (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected with another node before it
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected with another node before it (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected with another node after it
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected with another node after it (inverse selection)
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected as the only node
// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Decorator text content for selection > Included if decorator is selected as the only node (inverse selection)
#[test]
fn decorator_text_content_for_selection() {
    let mut cases: Vec<(String, Box<dyn FnOnce()>)> = Vec::new();
    for (name, f) in decorator_cases() {
        for invert in [false, true] {
            let name = if invert {
                format!("{name} (inverse selection)")
            } else {
                name.to_owned()
            };
            cases.push((
                name,
                Box::new(move || decorator_text_content_case(f, invert)),
            ));
        }
    }
    run_cases(
        "decorator_text_content_for_selection",
        "LexicalSelection tests > Decorator text content for selection",
        cases,
    );
}

// LexicalSelection tests > insertParagraph

/// `createTestEditor()` with `testEditor.setRootElement(element)`.
fn mounted_test_editor() -> Editor {
    let editor = create_test_editor(CreateEditorArgs::default());
    set_test_root_element(&editor);
    editor
}

fn span(text: &str) -> String {
    format!("<span data-lexical-text=\"true\">{text}</span>")
}

fn strong(text: &str) -> String {
    format!("<strong data-lexical-text=\"true\">{text}</strong>")
}

/// A paragraph of `texts` at the root, the caret at the start of the
/// third, `insertParagraph`; the rendered HTML.
fn insert_paragraph_at_third(specs: &'static [(&'static str, bool)]) -> String {
    let editor = mounted_test_editor();
    update(&editor, move || {
        let texts: Vec<NodeRef> = specs
            .iter()
            .map(|&(t, is_bold)| if is_bold { bold(t) } else { text(t) })
            .collect();
        let third = texts[2].clone();
        get_root().append(&[paragraph_of(&texts)])?;
        set_anchor_point(third.get_key(), 0, T)?;
        set_focus_point(third.get_key(), 0, T)?;
        let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
            return Ok(());
        };
        selection.insert_paragraph()?;
        Ok(())
    });
    rendered_html(&editor)
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > insertParagraph > three text nodes at offset 0 on third node
#[test]
fn insert_paragraph_three_text_nodes_at_offset_0_on_third_node() {
    let html =
        insert_paragraph_at_third(&[("Hello ", false), ("awesome", true), (" world", false)]);
    assert_eq!(
        html,
        format!(
            "<p dir=\"auto\">{}{}</p><p dir=\"auto\">{}</p>",
            span("Hello "),
            strong("awesome"),
            span(" world")
        )
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > insertParagraph > four text nodes at offset 0 on third node
#[test]
fn insert_paragraph_four_text_nodes_at_offset_0_on_third_node() {
    let html = insert_paragraph_at_third(&[
        ("Hello ", false),
        ("awesome ", true),
        ("beautiful", false),
        (" world", true),
    ]);
    assert_eq!(
        html,
        format!(
            "<p dir=\"auto\">{}{}</p><p dir=\"auto\">{}{}</p>",
            span("Hello "),
            strong("awesome "),
            span("beautiful"),
            strong(" world")
        )
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > insertParagraph > adjust offset for inline elements text formatting
#[test]
fn insert_paragraph_adjust_offset_for_inline_elements_text_formatting() {
    // `await init()`: the fixture is rendered a second time.
    let _first = Fixture::new();
    let fixture = Fixture::new();
    update(&fixture.editor, || {
        let text1 = create_text_node("--");
        let text2 = create_text_node("abc");
        let text3 = create_text_node("--");
        let link = create_link_node("https://lexical.dev", &LinkAttributes::default());
        link.append(std::slice::from_ref(&text2))?;
        get_root().append(&[paragraph_of(&[text1.clone(), link, text3.clone()])])?;
        set_anchor_point(text1.get_key(), 2, T)?;
        set_focus_point(text3.get_key(), 0, T)?;
        let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
            return Ok(());
        };
        selection.format_text("bold", None)?;
        assert!(text2.has_format("bold"));
        Ok(())
    });
}

// LexicalSelection tests > $patchStyle

/// A paragraph holding a text node with `style` at the root; a new range
/// selection, set and `insertParagraph`ed, then over the text's first ten
/// units. Returns the selection.
fn styled_text_selection(style: &str) -> Result<SelectionRef, LexicalError> {
    let text_node = create_text_node("Hello, World!");
    text_node.set_style(style);
    get_root().append(&[paragraph_of(std::slice::from_ref(&text_node))])?;
    let selection = create_range_selection();
    set_selection(Some(selection.clone()));
    selection.insert_paragraph()?;
    set_anchor_point(text_node.get_key(), 0, T)?;
    set_focus_point(text_node.get_key(), 10, T)?;
    Ok(selection)
}

fn style_value(selection: &SelectionRef, property: &str) -> String {
    get_selection_style_value_for_property(selection, property, "").unwrap()
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $patchStyle > should patch the style with the new style object
#[test]
fn patch_style_should_patch_the_style_with_the_new_style_object() {
    let fixture = Fixture::new();
    update(&fixture.editor, || {
        let selection = styled_text_selection("font-family: serif; color: red;")?;
        patch_style_text(
            &selection,
            &[("color", "blue".into()), ("font-family", "Arial".into())],
        )?;
        assert_eq!(style_value(&selection, "font-family"), "Arial");
        assert_eq!(style_value(&selection, "color"), "blue");
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $patchStyle > should patch the style with property function
#[test]
fn patch_style_should_patch_the_style_with_property_function() {
    let fixture = Fixture::new();
    update(&fixture.editor, || {
        let current_color = "red";
        let next_color = "blue";
        let selection = styled_text_selection(&format!("color: {current_color};"))?;
        // `vi.fn(...)`: the calls, as (current value, whether the target is
        // a text node).
        let calls: Rc<RefCell<Vec<Call>>> = Rc::default();
        let record = calls.clone();
        let color = StylePatchValue::Compute(Rc::new(move |current, target| {
            record.borrow_mut().push((
                current.map(str::to_owned),
                matches!(target, StylePatchTarget::Text(node) if node.is_text()),
            ));
            next_color.to_owned()
        }));
        patch_style_text(&selection, &[("color", color)])?;
        assert_eq!(style_value(&selection, "color"), next_color);
        assert_eq!(calls.borrow().len(), 1);
        let last_call = calls.borrow().last().cloned().unwrap();
        assert_eq!(last_call.0.as_deref(), Some(current_color));
        assert!(last_call.1);
        Ok(())
    });
}

/// A call of the patch function: the current value, and whether the
/// target was a text node.
type Call = (Option<String>, bool);

// LexicalSelection tests > Node.replace

/// The `beforeEach`: "Hello ", bold "awesome" and " world" in a paragraph.
fn node_replace_editor() -> (Editor, [NodeRef; 3], NodeRef) {
    let editor = mounted_test_editor();
    let nodes = update_with(&editor, || {
        let text1 = create_text_node("Hello ");
        let text2 = bold("awesome");
        let text3 = create_text_node(" world");
        let paragraph = paragraph_of(&[text1.clone(), text2.clone(), text3.clone()]);
        get_root().append(std::slice::from_ref(&paragraph))?;
        Ok(([text1, text2, text3], paragraph))
    });
    (editor, nodes.0, nodes.1)
}

fn node_replace_case(f: fn(&[NodeRef; 3], &NodeRef) -> (String, usize)) {
    let (editor, texts, paragraph) = node_replace_editor();
    update(&editor, move || {
        let (key, offset) = f(&texts, &paragraph);
        let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
            return Ok(());
        };
        let range = selection.range().unwrap();
        assert_eq!(
            (range.anchor.key.as_str(), range.anchor.offset),
            (key.as_str(), offset)
        );
        assert_eq!(
            (range.focus.key.as_str(), range.focus.offset),
            (key.as_str(), offset)
        );
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Node.replace > moves selection to next text node if replacing with decorator
#[test]
fn node_replace_moves_selection_to_next_text_node_if_replacing_with_decorator() {
    node_replace_case(|[_, text2, text3], _| {
        text2.select(Some(1), Some(1)).unwrap();
        text2.replace(&create_test_decorator_node(), false).unwrap();
        (text3.get_key().to_owned(), 0)
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Node.replace > moves selection to parent if next sibling is not a text node
#[test]
fn node_replace_moves_selection_to_parent_if_next_sibling_is_not_a_text_node() {
    node_replace_case(|[_, text2, text3], paragraph| {
        text3.replace(&create_test_decorator_node(), false).unwrap();
        text2.select(Some(1), Some(1)).unwrap();
        text2.replace(&create_test_decorator_node(), false).unwrap();
        (paragraph.get_key().to_owned(), 2)
    });
}

/// A mounted `createTestEditor()` and a text styled `style`, selected as
/// [`styled_text_selection`] does; `check` reads it.
fn raw_css_case(style: &'static str, check: fn(&SelectionRef)) {
    let editor = mounted_test_editor();
    update(&editor, move || {
        check(&styled_text_selection(style)?);
        Ok(())
    });
}

// upstream: "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Testing that $getStyleObjectFromRawCSS handles unformatted css text  > "
#[test]
fn get_style_object_from_raw_css_handles_unformatted_css_text() {
    raw_css_case(
        "   font-family  : Arial  ;  color    :   red   ;top     : 50px",
        |selection| {
            assert_eq!(style_value(selection, "font-family"), "Arial");
            assert_eq!(style_value(selection, "color"), "red");
            assert_eq!(style_value(selection, "top"), "50px");
        },
    );
}

// upstream: "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Testing that getStyleObjectFromRawCSS handles values with colons > "
#[test]
fn get_style_object_from_raw_css_handles_values_with_colons() {
    raw_css_case(
        "font-family: double:prefix:Arial; color: color:white; font-size: 30px",
        |selection| {
            assert_eq!(style_value(selection, "font-family"), "double:prefix:Arial");
            assert_eq!(style_value(selection, "color"), "color:white");
            assert_eq!(style_value(selection, "font-size"), "30px");
        },
    );
}

// upstream: "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > Testing that getStyleObjectFromRawCSS handles comments and semicolons inside values > "
#[test]
fn get_style_object_from_raw_css_handles_comments_and_semicolons_inside_values() {
    raw_css_case(
        "background-image: url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'></svg>\"); /* ignored */ content: \"semi;colon:value\"; color: red;",
        |selection| {
            assert_eq!(
                style_value(selection, "background-image"),
                "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'></svg>\")"
            );
            assert_eq!(style_value(selection, "content"), "\"semi;colon:value\"");
            assert_eq!(style_value(selection, "color"), "red");
        },
    );
}

// LexicalSelection tests > $setBlocksType

/// A mounted `createTestEditor()`; `f` in an update.
fn in_test_editor(f: impl FnOnce() -> Result<(), LexicalError> + 'static) -> Editor {
    let editor = mounted_test_editor();
    update(&editor, f);
    editor
}

/// `$createRangeSelection()`, `$setSelection`, then the two points.
fn select_points(
    anchor: (&NodeRef, usize, PointType),
    focus: (&NodeRef, usize, PointType),
) -> Result<SelectionRef, LexicalError> {
    let selection = create_range_selection();
    set_selection(Some(selection.clone()));
    set_anchor_point(anchor.0.get_key(), anchor.1, anchor.2)?;
    set_focus_point(focus.0.get_key(), focus.1, focus.2)?;
    Ok(selection)
}

fn to_h1(selection: &SelectionRef) -> Result<(), LexicalError> {
    set_blocks_type(Some(selection), h1, None)
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Collapsed selection in text
#[test]
fn set_blocks_type_collapsed_selection_in_text() {
    in_test_editor(|| {
        let text1 = create_text_node("text 1");
        get_root().append(&[
            paragraph_of(std::slice::from_ref(&text1)),
            paragraph_of(&[create_text_node("text 2")]),
        ])?;
        let selection = select_points((&text1, 6, T), (&text1, 6, T))?;
        to_h1(&selection)?;
        assert_eq!(root_types(), ["heading", "paragraph"]);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Collapsed selection in element
#[test]
fn set_blocks_type_collapsed_selection_in_element() {
    in_test_editor(|| {
        get_root().append(&[create_paragraph_node(), create_paragraph_node()])?;
        let root = get_root();
        let selection = select_points((&root, 0, E), (&root, 0, E))?;
        to_h1(&selection)?;
        assert_eq!(root_types(), ["heading", "paragraph"]);
        Ok(())
    });
}

/// `Two elements, same top-element` (upstream has it twice, the same).
fn two_elements_same_top_element() {
    in_test_editor(|| {
        let text1 = create_text_node("text 1");
        let text2 = create_text_node("text 2");
        get_root().append(&[
            paragraph_of(std::slice::from_ref(&text1)),
            paragraph_of(std::slice::from_ref(&text2)),
        ])?;
        let selection = select_points((&text1, 0, T), (&text2, 6, T))?;
        to_h1(&selection)?;
        assert_eq!(root_types(), ["heading", "heading"]);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Two elements, same top-element
#[test]
fn set_blocks_type_two_elements_same_top_element() {
    two_elements_same_top_element();
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Two elements, same top-element #2
#[test]
fn set_blocks_type_two_elements_same_top_element_2() {
    two_elements_same_top_element();
}

/// [`in_test_editor`] on an editor that also has the table nodes, with the
/// editor passed to `f`.
#[cfg(feature = "table")]
fn in_table_test_editor(f: impl FnOnce(&Editor) -> Result<(), LexicalError> + 'static) {
    let editor = create_test_editor(CreateEditorArgs {
        nodes: crate::table::table_nodes(),
        ..CreateEditorArgs::default()
    });
    set_test_root_element(&editor);
    let e = editor.clone();
    update(&editor, move || f(&e));
}

/// `$createTableNodeWithDimensions(rows, columns)`, with its default
/// `includeHeaders = true`.
#[cfg(feature = "table")]
fn table_with_dimensions(rows: usize, columns: usize) -> Result<NodeRef, LexicalError> {
    crate::table::create_table_node_with_dimensions(
        rows,
        columns,
        crate::table::InsertTableCommandPayloadHeaders::Both(true),
    )
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Collapsed in element inside top-element
#[cfg(feature = "table")]
#[test]
fn set_blocks_type_collapsed_in_element_inside_top_element() {
    in_table_test_editor(|_| {
        let table = table_with_dimensions(1, 1)?;
        let row = table.get_first_child().filter(NodeRef::is_element).unwrap();
        let column = row.get_first_child().filter(NodeRef::is_element).unwrap();
        let paragraph = column
            .get_first_child()
            .filter(NodeRef::is_element)
            .unwrap();
        if let Some(child) = paragraph.get_first_child() {
            child.remove(false)?;
        }
        get_root().append(&[table])?;
        let selection = select_points((&paragraph, 0, E), (&paragraph, 0, E))?;
        assert_eq!(column.get_children()[0].get_type(), "paragraph");
        to_h1(&selection)?;
        let after = column.get_children();
        assert_eq!(after[0].get_type(), "heading");
        assert_eq!(after.len(), 1);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Collapsed in text inside top-element
#[cfg(feature = "table")]
#[test]
fn set_blocks_type_collapsed_in_text_inside_top_element() {
    in_table_test_editor(|_| {
        let table = table_with_dimensions(1, 1)?;
        let row = table.get_first_child().filter(NodeRef::is_element).unwrap();
        let column = row.get_first_child().filter(NodeRef::is_element).unwrap();
        let paragraph = column
            .get_first_child()
            .filter(NodeRef::is_element)
            .unwrap();
        let text = create_text_node("foo");
        get_root().append(&[table])?;
        paragraph.append(std::slice::from_ref(&text))?;
        select_points((&text, 3, T), (&text, 3, T))?;
        let selection = get_selection().filter(SelectionRef::is_range).unwrap();
        assert_eq!(column.get_children()[0].get_type(), "paragraph");
        to_h1(&selection)?;
        let after = column.get_children();
        assert_eq!(after[0].get_type(), "heading");
        assert_eq!(after.len(), 1);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Full editor selection with a mix of top-elements
#[cfg(feature = "table")]
#[test]
fn set_blocks_type_full_editor_selection_with_a_mix_of_top_elements() {
    in_table_test_editor(|editor| {
        let root = get_root();
        let paragraph1 = create_paragraph_node();
        let paragraph2 = create_paragraph_node();
        paragraph1.append(&[create_text_node("")])?;
        paragraph2.append(&[create_text_node("")])?;
        root.append(&[paragraph1.clone(), paragraph2.clone()])?;
        let table = table_with_dimensions(1, 2)?;
        let row = table.get_first_child().filter(NodeRef::is_element).unwrap();
        let columns = row.get_children();
        root.append(&[table])?;
        // Upstream appends text3 and text4 to paragraph1 and paragraph2.
        paragraph1.append(&[create_text_node("")])?;
        paragraph2.append(&[create_text_node("")])?;
        columns[0].append(&[create_paragraph_node(), create_paragraph_node()])?;
        columns[1].append(&[create_paragraph_node(), create_paragraph_node()])?;
        let paragraph7 = create_paragraph_node();
        root.append(std::slice::from_ref(&paragraph7))?;
        select_points((&paragraph1, 0, E), (&paragraph7, 0, E))?;
        let selection = get_selection().filter(SelectionRef::is_range).unwrap();
        to_h1(&selection)?;
        let pending = editor.pending().expect("a pending editor state");
        assert_eq!(
            pending.to_json(false).to_string(),
            r#"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1}],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1}],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[{"children":[{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1}],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1}],"backgroundColor":null,"colSpan":1,"headerState":3,"rowSpan":1,"direction":null,"format":"","indent":0,"type":"tablecell","version":1},{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1}],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1}],"backgroundColor":null,"colSpan":1,"headerState":1,"rowSpan":1,"direction":null,"format":"","indent":0,"type":"tablecell","version":1}],"direction":null,"format":"","indent":0,"type":"tablerow","version":1}],"direction":null,"format":"","indent":0,"type":"table","version":1},{"children":[],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#
        );
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Two empty elements, same top-element
#[test]
fn set_blocks_type_two_empty_elements_same_top_element() {
    in_test_editor(|| {
        let paragraph1 = create_paragraph_node();
        let paragraph2 = create_paragraph_node();
        get_root().append(&[paragraph1.clone(), paragraph2.clone()])?;
        let selection = select_points((&paragraph1, 0, E), (&paragraph2, 0, E))?;
        to_h1(&selection)?;
        let root_children = get_root().get_children();
        assert_eq!(root_types(), ["heading", "heading"]);
        let sel = get_selection().unwrap();
        let range = sel.range().unwrap();
        assert_eq!(
            (
                range.anchor.key.as_str(),
                range.anchor.offset,
                range.anchor.point_type
            ),
            (root_children[0].get_key(), 0, E)
        );
        assert_eq!(
            (
                range.focus.key.as_str(),
                range.focus.offset,
                range.focus.point_type
            ),
            (root_children[1].get_key(), 0, E)
        );
        assert_eq!(sel.get_nodes()?, root_children);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Paragraph with links to heading with links
#[test]
fn set_blocks_type_paragraph_with_links_to_heading_with_links() {
    in_test_editor(|| {
        let text1 = create_text_node("Links: ");
        let text2 = create_text_node("link1");
        let text3 = create_text_node("link2");
        let link1 = create_link_node("https://lexical.dev", &LinkAttributes::default());
        link1.append(&[text2])?;
        let link2 = create_link_node("https://playground.lexical.dev", &LinkAttributes::default());
        link2.append(std::slice::from_ref(&text3))?;
        let paragraph = paragraph_of(&[text1.clone(), link1, create_text_node(" "), link2]);
        get_root().append(std::slice::from_ref(&paragraph))?;
        let paragraph_children_keys = paragraph.get_children_keys();
        let selection = select_points((&text1, 1, T), (&text3, 1, T))?;
        to_h1(&selection)?;
        let root_children = get_root().get_children();
        assert_eq!(root_children.len(), 1);
        assert!(root_children[0].is_element());
        assert_eq!(root_children[0].get_type(), "heading");
        assert_eq!(
            root_children[0].get_children_keys(),
            paragraph_children_keys
        );
        Ok(())
    });
}

fn list_item_of(children: &[NodeRef]) -> NodeRef {
    element_of(create_list_item_node(None), children)
}

fn bullet_list_of(children: &[NodeRef]) -> NodeRef {
    element_of(create_list_node(ListType::Bullet, 1.0), children)
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Nested list
#[test]
fn set_blocks_type_nested_list() {
    let editor = in_test_editor(|| {
        let text1 = create_text_node("1");
        let text1_1 = create_text_node("1.1");
        get_root().append(&[bullet_list_of(&[
            list_item_of(std::slice::from_ref(&text1)),
            list_item_of(&[bullet_list_of(&[list_item_of(std::slice::from_ref(
                &text1_1,
            ))])]),
        ])])?;
        let selection = select_points((&text1, 1, T), (&text1_1, 1, T))?;
        to_h1(&selection)
    });
    assert_eq!(
        rendered_html(&editor),
        format!(
            "<h1 dir=\"auto\">{}</h1><h1 dir=\"auto\" style=\"padding-inline-start: calc(1 * var(--lexical-indent-base-value, 40px));\">{}</h1>",
            span("1"),
            span("1.1")
        )
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Nested list with listItem twice indented from its parent
#[test]
fn set_blocks_type_nested_list_with_list_item_twice_indented_from_its_parent() {
    let editor = in_test_editor(|| {
        let text1_1 = create_text_node("1.1");
        get_root().append(&[bullet_list_of(&[list_item_of(&[bullet_list_of(&[
            list_item_of(std::slice::from_ref(&text1_1)),
        ])])])])?;
        let selection = select_points((&text1_1, 1, T), (&text1_1, 1, T))?;
        to_h1(&selection)
    });
    assert_eq!(
        rendered_html(&editor),
        format!(
            "<h1 dir=\"auto\" style=\"padding-inline-start: calc(1 * var(--lexical-indent-base-value, 40px));\">{}</h1>",
            span("1.1")
        )
    );
}

/// `buildEditorFromExtensions({$initialEditorState, dependencies, name:
/// '@test'})`, then the root's children's types.
fn with_initial_state(
    dependencies: Vec<AnyLexicalExtensionArgument>,
    initial: impl Fn() -> Result<(), LexicalError> + 'static,
) -> Vec<&'static str> {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
        ..LexicalExtension::<(), (), ()>::new("@test", ())
    })
    .into()])
    .unwrap();
    let types = editor.read(root_types).unwrap();
    editor.dispose();
    types
}

fn rich_text() -> Vec<AnyLexicalExtensionArgument> {
    vec![rich_text_extension().into()]
}

/// Paragraphs at the root holding `texts`; the text nodes.
fn paragraphs_of(texts: &[&str]) -> Vec<NodeRef> {
    let nodes: Vec<NodeRef> = texts.iter().map(|t| create_text_node(t)).collect();
    for node in &nodes {
        get_root()
            .append(&[paragraph_of(std::slice::from_ref(node))])
            .unwrap();
    }
    nodes
}

/// `text1.select().setTextNodeRange(anchor, anchorOffset, focus,
/// focusOffset)`, then `$setBlocksType` to h1.
fn text_range_to_h1(
    text1: &NodeRef,
    anchor: (&NodeRef, usize),
    focus: (&NodeRef, usize),
) -> Result<(), LexicalError> {
    let selection = text1.select(None, None)?;
    selection.set_text_node_range(anchor.0, anchor.1, focus.0, focus.1)?;
    to_h1(&selection)
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Triple-click overselection: focus at element offset 0 of non-empty next block is skipped
#[test]
fn triple_click_overselection_focus_at_element_offset_0_of_non_empty_next_block_is_skipped() {
    let types = with_initial_state(rich_text(), || {
        let text1 = create_text_node("text 1");
        let paragraph2 = create_paragraph_node();
        paragraph2.append(&[create_line_break_node()])?;
        get_root().append(&[
            paragraph_of(std::slice::from_ref(&text1)),
            paragraph2.clone(),
        ])?;
        // Browser triple-click: focus lands on the text node of the
        // following block at offset 0, even though visually only
        // paragraph1's content is selected.
        let selection = text1.select_start()?;
        selection.point_set(Which::Focus, paragraph2.get_key(), 0, E)?;
        to_h1(&selection)
    });
    assert_eq!(types, ["heading", "paragraph"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Triple-click overselection: focus at element offset 0 of empty next block is converted
#[test]
fn triple_click_overselection_focus_at_element_offset_0_of_empty_next_block_is_converted() {
    let types = with_initial_state(rich_text(), || {
        let text1 = create_text_node("text 1");
        let paragraph2 = create_paragraph_node();
        get_root().append(&[
            paragraph_of(std::slice::from_ref(&text1)),
            paragraph2.clone(),
        ])?;
        let selection = text1.select_start()?;
        selection.point_set(Which::Focus, paragraph2.get_key(), 0, E)?;
        to_h1(&selection)
    });
    assert_eq!(types, ["heading", "heading"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Triple-click overselection: focus at offset 0 of next block is skipped
#[test]
fn triple_click_overselection_focus_at_offset_0_of_next_block_is_skipped() {
    let types = with_initial_state(rich_text(), || {
        let t = paragraphs_of(&["text 1", "text 2"]);
        text_range_to_h1(&t[0], (&t[0], 0), (&t[1], 0))
    });
    assert_eq!(types, ["heading", "paragraph"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Triple-click overselection: focus inside nested inline at offset 0 is skipped
#[test]
fn triple_click_overselection_focus_inside_nested_inline_at_offset_0_is_skipped() {
    let types = with_initial_state(
        vec![rich_text_extension().into(), link_extension().into()],
        || {
            let text1 = create_text_node("text 1");
            let text2 = create_text_node("text 2");
            let link = create_link_node("https://lexical.dev", &LinkAttributes::default());
            link.append(std::slice::from_ref(&text2))?;
            get_root().append(&[
                paragraph_of(std::slice::from_ref(&text1)),
                paragraph_of(&[link]),
            ])?;
            text_range_to_h1(&text1, (&text1, 0), (&text2, 0))
        },
    );
    assert_eq!(types, ["heading", "paragraph"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Non-zero focus offset in next block still converts both blocks
#[test]
fn non_zero_focus_offset_in_next_block_still_converts_both_blocks() {
    let types = with_initial_state(rich_text(), || {
        let t = paragraphs_of(&["text 1", "text 2"]);
        text_range_to_h1(&t[0], (&t[0], 0), (&t[1], 1))
    });
    assert_eq!(types, ["heading", "heading"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Triple-click overselection spanning multiple blocks skips only the focus block
#[test]
fn triple_click_overselection_spanning_multiple_blocks_skips_only_the_focus_block() {
    let types = with_initial_state(rich_text(), || {
        let t = paragraphs_of(&["text 1", "text 2", "text 3"]);
        text_range_to_h1(&t[0], (&t[0], 0), (&t[2], 0))
    });
    assert_eq!(types, ["heading", "heading", "paragraph"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Focus at offset 0 in next block whose first descendant has a prior sibling still converts focus block
#[test]
fn focus_at_offset_0_in_next_block_whose_first_descendant_has_a_prior_sibling_still_converts_focus_block()
 {
    let types = with_initial_state(rich_text(), || {
        let text1 = create_text_node("text 1");
        let text2a = create_text_node("foo");
        let text2b = bold("bar");
        get_root().append(&[
            paragraph_of(std::slice::from_ref(&text1)),
            // text2b is the second child; offset 0 of text2b is NOT at the
            // start of paragraph2 (text2a precedes it).
            paragraph_of(&[text2a, text2b.clone()]),
        ])?;
        text_range_to_h1(&text1, (&text1, 0), (&text2b, 0))
    });
    assert_eq!(types, ["heading", "heading"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Backward selection: focus at block end is skipped
#[test]
fn backward_selection_focus_at_block_end_is_skipped() {
    let types = with_initial_state(rich_text(), || {
        let t = paragraphs_of(&["text 1", "text 2"]);
        text_range_to_h1(&t[0], (&t[1], 0), (&t[0], 6))
    });
    assert_eq!(types, ["paragraph", "heading"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Backward selection: non-zero focus offset converts both blocks
#[test]
fn backward_selection_non_zero_focus_offset_converts_both_blocks() {
    let types = with_initial_state(rich_text(), || {
        let t = paragraphs_of(&["text 1", "text 2"]);
        text_range_to_h1(&t[0], (&t[1], 1), (&t[0], 5))
    });
    assert_eq!(types, ["heading", "heading"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Backward selection spanning multiple blocks skips only focus block
#[test]
fn backward_selection_spanning_multiple_blocks_skips_only_focus_block() {
    let types = with_initial_state(rich_text(), || {
        let t = paragraphs_of(&["text 1", "text 2", "text 3"]);
        text_range_to_h1(&t[0], (&t[2], 6), (&t[0], 6))
    });
    assert_eq!(types, ["paragraph", "heading", "heading"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Backward selection: focus at block end with element point is skipped
#[test]
fn backward_selection_focus_at_block_end_with_element_point_is_skipped() {
    let types = with_initial_state(rich_text(), || {
        let text1 = create_text_node("text 1");
        let text2 = create_text_node("text 2");
        let paragraph2 = paragraph_of(std::slice::from_ref(&text2));
        get_root().append(&[paragraph_of(&[text1]), paragraph2.clone()])?;
        let selection = select_points((&text2, 0, T), (&paragraph2, 0, E))?;
        to_h1(&selection)
    });
    assert_eq!(types, ["paragraph", "heading"]);
}

/// `TestSlotHost` (`test-slot-host`): an element that declares a `content`
/// slot.
#[derive(Clone, Debug, Default)]
struct TestSlotHost {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for TestSlotHost {
    fn get_type(&self) -> &'static str {
        "test-slot-host"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
    fn declared_slots(&self) -> &'static [&'static str] {
        &["content"]
    }
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("div"))
    }
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Bare slot value is skipped (#8894)
#[test]
fn bare_slot_value_is_skipped_8894() {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            rich_text_extension().into(),
            define_extension(LexicalExtension {
                nodes: vec![NodeClass::of::<TestSlotHost>()],
                ..LexicalExtension::<(), (), ()>::new("@test/slot-host", ())
            })
            .into(),
        ],
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let host = crate::create::<TestSlotHost>();
            let text = create_text_node("slot text");
            let paragraph = paragraph_of(std::slice::from_ref(&text));
            set_slot(&host, "content", &paragraph)?;
            get_root().append(&[host])?;
            let selection = text.select(Some(0), None)?;
            to_h1(&selection)
        }))),
        ..LexicalExtension::<(), (), ()>::new("@test", ())
    })
    .into()])
    .unwrap();
    editor
        .read(|| {
            let host = get_root().get_first_child().unwrap();
            let paragraph = get_slot(&host, "content").unwrap();
            assert_eq!(paragraph.get_type(), "paragraph");
            assert_eq!(paragraph.get_text_content(), "slot text");
        })
        .unwrap();
    editor.dispose();
}

//! Ports of
//! `packages/lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts`.
//!
//! Where upstream reads the mounted root's `innerHTML`, these render the
//! committed document as the reconciler would ([`rendered_html`]).

#![cfg(all(feature = "html", feature = "link", feature = "rich-text"))]

use super::support::{
    BlockTestDecoratorNode, create_paragraph_with_nodes, points_of, rendered_html,
    set_anchor_point, set_focus_point,
};
use crate::link::{LinkAttributes, create_link_node};
use crate::nodes::line_break::create_line_break_node;
use crate::rich_text::{HeadingTagType, create_heading_node, is_heading_node};
use crate::selection::{StylePatchValue, get_selection_style_value_for_property, patch_style_text};
use crate::upstream_tests::support::{
    TestElementNode, TestInlineElementNode, TestSegmentedNode, TestShadowRootNode,
    create_test_editor, create_test_element_node, create_test_shadow_root_node,
    set_test_root_element, update,
};
use crate::{
    CreateEditorArgs, Editor, LexicalError, NodeClass, NodeRef, PointType, SelectionRef,
    create_paragraph_node, create_range_selection, create_text_node, get_node_by_key, get_root,
    get_selection, insert_nodes, set_selection,
};
use std::rc::Rc;

use PointType::{Element as E, Text as T};

/// A text node of `$createParagraphWithNodes`: key, text, mergeable.
type Spec = (&'static str, &'static str, bool);

const ABC: [Spec; 3] = [("a", "a", false), ("b", "b", false), ("c", "c", false)];
const MERGEABLE_ABC: [Spec; 3] = [("a", "a", true), ("b", "b", true), ("c", "c", true)];

/// A point to set: the key (`None` for the paragraph), offset and type.
type At = (Option<&'static str>, usize, PointType);

fn keys(nodes: &[NodeRef]) -> Vec<String> {
    nodes.iter().map(|n| n.get_key().to_owned()).collect()
}

fn current() -> SelectionRef {
    get_selection()
        .filter(SelectionRef::is_range)
        .expect("a range selection")
}

/// `createTestEditor()` with `editor.setRootElement(element)`.
fn mounted_test_editor() -> Editor {
    let editor = create_test_editor(CreateEditorArgs::default());
    set_test_root_element(&editor);
    editor
}

/// One `setupTestCase(cb)`: a fresh editor with a paragraph of `nodes`,
/// the selection set from `anchor` to `focus`, then `body`.
fn setup_test_case(
    nodes: &'static [Spec],
    anchor: At,
    focus: At,
    body: impl FnOnce(&SelectionRef, &NodeRef) -> Result<(), LexicalError> + 'static,
) {
    let editor = create_test_editor(CreateEditorArgs::default());
    update(&editor, move || {
        let element = create_paragraph_with_nodes(nodes);
        get_root().append(std::slice::from_ref(&element))?;
        set_anchor_point(anchor.0.unwrap_or(element.get_key()), anchor.1, anchor.2)?;
        set_focus_point(focus.0.unwrap_or(element.get_key()), focus.1, focus.2)?;
        body(&current(), &element)
    });
}

/// Asserts both points of `selection` are `expected`.
fn assert_collapsed_at(selection: &SelectionRef, expected: String) {
    assert_eq!(points_of(selection), (expected.clone(), expected));
}

/// A mounted editor with a paragraph of `nodes` and the selection set, then
/// the committed selection's points and the paragraph's key.
fn committed_points(nodes: &'static [Spec], anchor: At, focus: At) -> (String, String, String) {
    let editor = mounted_test_editor();
    let element_key = crate::upstream_tests::support::update_with(&editor, move || {
        let element = create_paragraph_with_nodes(nodes);
        get_root().append(std::slice::from_ref(&element))?;
        set_anchor_point(anchor.0.unwrap_or(element.get_key()), anchor.1, anchor.2)?;
        set_focus_point(focus.0.unwrap_or(element.get_key()), focus.1, focus.2)?;
        Ok(element.get_key().to_owned())
    });
    let (anchor, focus) = editor
        .read_with_mode(crate::EditorReadMode::Latest, || points_of(&current()))
        .unwrap();
    (anchor, focus, element_key)
}

/// The collapsed-selection checks shared by the `Collapsed` element point
/// tests and `Simple range`: `insertText`, `insertParagraph`,
/// `insertLineBreak` and `formatText` then `insertText`.
fn insert_text_at_first(nodes: &'static [Spec], anchor: At, focus: At, last: bool) {
    setup_test_case(nodes, anchor, focus, move |selection, element| {
        selection.insert_text("Test")?;
        let child = if last {
            element.get_last_child().unwrap()
        } else {
            element.get_first_child().unwrap()
        };
        assert_eq!(child.get_text_content(), "Test");
        assert_collapsed_at(selection, format!("{}:4:text", child.get_key()));
        Ok(())
    });
}

fn format_bold_then_insert_text(nodes: &'static [Spec], anchor: At, focus: At, last: bool) {
    setup_test_case(nodes, anchor, focus, move |selection, element| {
        selection.format_text("bold", None)?;
        selection.insert_text("Test")?;
        let child = if last {
            element.get_last_child().unwrap()
        } else {
            element.get_first_child().unwrap()
        };
        assert_eq!(child.get_text_content(), "Test");
        assert_collapsed_at(selection, format!("{}:4:text", child.get_key()));
        Ok(())
    });
}

fn insert_line_break_at(
    nodes: &'static [Spec],
    anchor: At,
    focus: At,
    select_start: bool,
    offset: usize,
) {
    setup_test_case(nodes, anchor, focus, move |selection, element| {
        selection.insert_line_break(select_start)?;
        assert_collapsed_at(selection, format!("{}:{offset}:element", element.get_key()));
        Ok(())
    });
}

/// `insertParagraph`, then the selection at `expected` (a key, or the new
/// paragraph after the element when `None`).
fn insert_paragraph_at(
    nodes: &'static [Spec],
    anchor: At,
    focus: At,
    expected: Option<&'static str>,
) {
    setup_test_case(nodes, anchor, focus, move |selection, element| {
        selection.insert_paragraph()?;
        let expected = match expected {
            Some(key) => format!("{key}:0:text"),
            None => format!(
                "{}:0:element",
                element.get_next_sibling().unwrap().get_key()
            ),
        };
        assert_collapsed_at(selection, expected);
        Ok(())
    });
}

fn get_nodes_is(nodes: &'static [Spec], anchor: At, focus: At, expected: &'static [&'static str]) {
    setup_test_case(nodes, anchor, focus, move |selection, _| {
        assert_eq!(keys(&selection.get_nodes()?), expected);
        Ok(())
    });
}

fn text_content_is(nodes: &'static [Spec], anchor: At, focus: At, expected: &'static str) {
    setup_test_case(nodes, anchor, focus, move |selection, _| {
        assert_eq!(selection.get_text_content()?, expected);
        Ok(())
    });
}

fn extract_is(nodes: &'static [Spec], anchor: At, focus: At, expected: &'static [&'static str]) {
    setup_test_case(nodes, anchor, focus, move |selection, _| {
        assert_eq!(keys(&selection.extract()?), expected);
        Ok(())
    });
}

fn insert_foo_at_first(nodes: &'static [Spec], anchor: At, focus: At) {
    setup_test_case(nodes, anchor, focus, |selection, element| {
        selection.insert_nodes(vec![create_text_node("foo")])?;
        let first = element.get_first_child().unwrap();
        assert_collapsed_at(selection, format!("{}:3:text", first.get_key()));
        Ok(())
    });
}

// LexicalSelectionHelpers tests > Collapsed

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Can handle a text point
#[test]
fn collapsed_can_handle_a_text_point() {
    let a: At = (Some("a"), 0, T);
    // getNodes
    get_nodes_is(&ABC, a, a, &["a"]);
    // getTextContent
    text_content_is(&ABC, a, a, "");
    // insertText
    setup_test_case(&ABC, a, a, |selection, _| {
        selection.insert_text("Test")?;
        assert_eq!(get_node_by_key("a").unwrap().get_text_content(), "Testa");
        assert_collapsed_at(selection, "a:4:text".to_owned());
        Ok(())
    });
    // insertNodes
    insert_foo_at_first(&ABC, a, a);
    // insertParagraph
    insert_paragraph_at(&ABC, a, a, Some("a"));
    // insertLineBreak
    insert_line_break_at(&ABC, a, a, true, 0);
    // Format text
    setup_test_case(&ABC, a, a, |selection, element| {
        selection.format_text("bold", None)?;
        selection.insert_text("Test")?;
        let first = element.get_first_child().unwrap();
        assert_eq!(first.get_text_content(), "Test");
        assert_collapsed_at(selection, format!("{}:4:text", first.get_key()));
        assert_eq!(first.get_next_sibling().unwrap().get_text_content(), "a");
        Ok(())
    });
    // Extract selection
    extract_is(&ABC, a, a, &["a"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct text point after removal after merge
#[test]
fn has_correct_text_point_after_removal_after_merge() {
    const NODES: [Spec; 5] = [
        ("a", "a", true),
        ("bb", "bb", true),
        ("empty", "", true),
        ("cc", "cc", true),
        ("d", "d", true),
    ];
    let (anchor, focus, _) = committed_points(&NODES, (Some("bb"), 1, T), (Some("cc"), 1, T));
    assert_eq!(anchor, "a:2:text");
    assert_eq!(focus, "a:4:text");
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct text point after removal after merge (2)
#[test]
fn has_correct_text_point_after_removal_after_merge_2() {
    const NODES: [Spec; 4] = [
        ("a", "a", true),
        ("empty", "", true),
        ("b", "b", true),
        ("c", "c", true),
    ];
    let (anchor, focus, _) = committed_points(&NODES, (Some("a"), 0, T), (Some("c"), 1, T));
    assert_eq!(anchor, "a:0:text");
    assert_eq!(focus, "a:3:text");
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct text point adjust to element point after removal of a single empty text node
#[test]
fn has_correct_text_point_adjust_to_element_point_after_removal_of_a_single_empty_text_node() {
    const NODES: [Spec; 1] = [("a", "", true)];
    let (anchor, focus, element) = committed_points(&NODES, (Some("a"), 0, T), (Some("a"), 0, T));
    assert_eq!(anchor, format!("{element}:0:element"));
    assert_eq!(focus, format!("{element}:0:element"));
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct element point after removal of an empty text node in a group #1
#[test]
fn has_correct_element_point_after_removal_of_an_empty_text_node_in_a_group_1() {
    const NODES: [Spec; 2] = [("a", "", true), ("b", "b", false)];
    let (anchor, focus, _) = committed_points(&NODES, (None, 2, E), (None, 2, E));
    assert_eq!(anchor, "b:1:text");
    assert_eq!(focus, "b:1:text");
}

const GROUP: [Spec; 4] = [
    ("a", "", true),
    ("b", "b", false),
    ("c", "c", true),
    ("d", "d", true),
];

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct element point after removal of an empty text node in a group #2
#[test]
fn has_correct_element_point_after_removal_of_an_empty_text_node_in_a_group_2() {
    let (anchor, focus, _) = committed_points(&GROUP, (None, 4, E), (None, 4, E));
    assert_eq!(anchor, "c:2:text");
    assert_eq!(focus, "c:2:text");
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct text point after removal of an empty text node in a group #3
#[test]
fn has_correct_text_point_after_removal_of_an_empty_text_node_in_a_group_3() {
    let (anchor, focus, _) = committed_points(&GROUP, (Some("d"), 1, T), (Some("d"), 1, T));
    assert_eq!(anchor, "c:2:text");
    assert_eq!(focus, "c:2:text");
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Can handle an element point on empty element
#[test]
fn can_handle_an_element_point_on_empty_element() {
    const NONE: [Spec; 0] = [];
    let point: At = (None, 0, E);
    // getNodes
    setup_test_case(&NONE, point, point, |selection, element| {
        assert_eq!(keys(&selection.get_nodes()?), [element.get_key()]);
        Ok(())
    });
    // getTextContent
    text_content_is(&NONE, point, point, "");
    // insertText
    insert_text_at_first(&NONE, point, point, false);
    // insertParagraph
    insert_paragraph_at(&NONE, point, point, None);
    // insertLineBreak
    insert_line_break_at(&NONE, point, point, true, 0);
    // Format text
    format_bold_then_insert_text(&NONE, point, point, false);
    // Extract selection
    setup_test_case(&NONE, point, point, |selection, element| {
        assert_eq!(keys(&selection.extract()?), [element.get_key()]);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Can handle a start element point
#[test]
fn can_handle_a_start_element_point() {
    let point: At = (None, 0, E);
    get_nodes_is(&ABC, point, point, &["a"]);
    text_content_is(&ABC, point, point, "");
    insert_text_at_first(&ABC, point, point, false);
    insert_paragraph_at(&ABC, point, point, Some("a"));
    insert_line_break_at(&ABC, point, point, true, 0);
    format_bold_then_insert_text(&ABC, point, point, false);
    extract_is(&ABC, point, point, &["a"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Can handle an end element point
#[test]
fn can_handle_an_end_element_point() {
    let point: At = (None, 3, E);
    get_nodes_is(&ABC, point, point, &["c"]);
    text_content_is(&ABC, point, point, "");
    insert_text_at_first(&ABC, point, point, true);
    insert_paragraph_at(&ABC, point, point, None);
    insert_line_break_at(&ABC, point, point, false, 4);
    format_bold_then_insert_text(&ABC, point, point, true);
    extract_is(&ABC, point, point, &["c"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct element point after merge from middle
#[test]
fn has_correct_element_point_after_merge_from_middle() {
    let (anchor, focus, _) = committed_points(&MERGEABLE_ABC, (None, 2, E), (None, 2, E));
    assert_eq!(anchor, "a:2:text");
    assert_eq!(focus, "a:2:text");
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Collapsed > Has correct element point after merge from end
#[test]
fn has_correct_element_point_after_merge_from_end() {
    let (anchor, focus, _) = committed_points(&MERGEABLE_ABC, (None, 3, E), (None, 3, E));
    assert_eq!(anchor, "a:3:text");
    assert_eq!(focus, "a:3:text");
}

// LexicalSelectionHelpers tests > Simple range

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Simple range > Can handle multiple text points
#[test]
fn can_handle_multiple_text_points() {
    let (anchor, focus): (At, At) = ((Some("a"), 0, T), (Some("b"), 0, T));
    get_nodes_is(&ABC, anchor, focus, &["a", "b"]);
    text_content_is(&ABC, anchor, focus, "a");
    insert_text_at_first(&ABC, anchor, focus, false);
    insert_foo_at_first(&ABC, anchor, focus);
    insert_paragraph_at(&ABC, anchor, focus, Some("b"));
    insert_line_break_at(&ABC, anchor, focus, true, 0);
    format_bold_then_insert_text(&ABC, anchor, focus, false);
    extract_is(&ABC, anchor, focus, &["a"]);
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Simple range > Can handle multiple element points
#[test]
fn can_handle_multiple_element_points() {
    let (anchor, focus): (At, At) = ((None, 0, E), (None, 1, E));
    get_nodes_is(&ABC, anchor, focus, &["a"]);
    text_content_is(&ABC, anchor, focus, "a");
    insert_text_at_first(&ABC, anchor, focus, false);
    insert_paragraph_at(&ABC, anchor, focus, Some("b"));
    insert_line_break_at(&ABC, anchor, focus, true, 0);
    format_bold_then_insert_text(&ABC, anchor, focus, false);
    setup_test_case(&ABC, anchor, focus, |selection, element| {
        assert_eq!(
            keys(&selection.extract()?),
            [element.get_first_child().unwrap().get_key()]
        );
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > Simple range > Can handle a mix of text and element points
#[test]
fn can_handle_a_mix_of_text_and_element_points() {
    let (anchor, focus): (At, At) = ((None, 0, E), (Some("c"), 1, T));
    // isBefore
    setup_test_case(&ABC, anchor, focus, |selection, _| {
        let range = selection.range().unwrap();
        assert!(range.anchor.is_before(&range.focus)?);
        Ok(())
    });
    get_nodes_is(&ABC, anchor, focus, &["a", "b", "c"]);
    text_content_is(&ABC, anchor, focus, "abc");
    insert_text_at_first(&ABC, anchor, focus, false);
    insert_paragraph_at(&ABC, anchor, focus, None);
    insert_line_break_at(&ABC, anchor, focus, true, 0);
    format_bold_then_insert_text(&ABC, anchor, focus, false);
    extract_is(&ABC, anchor, focus, &["a", "b", "c"]);
}

// can insert ... correctly

/// A mounted `createTestEditor()`, `f` in an update, then the rendered
/// HTML.
fn render_after_update(f: impl FnOnce() -> Result<(), LexicalError> + 'static) -> String {
    let editor = mounted_test_editor();
    update(&editor, f);
    rendered_html(&editor)
}

fn span(text: &str) -> String {
    format!("<span data-lexical-text=\"true\">{text}</span>")
}

fn p(inner: &str) -> String {
    format!("<p dir=\"auto\">{inner}</p>")
}

fn link_html(href: &str, text: &str) -> String {
    format!("<a href=\"{href}\">{}</a>", span(text))
}

fn link_of(url: &str, text: &str) -> NodeRef {
    let link = create_link_node(url, &LinkAttributes::default());
    link.append(&[create_text_node(text)]).unwrap();
    link
}

fn paragraph_of(children: &[NodeRef]) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(children).unwrap();
    paragraph
}

/// The paragraph-with-selection setup of `with an empty paragraph node
/// selected`.
fn select_empty_paragraph() -> Result<SelectionRef, LexicalError> {
    let paragraph = create_paragraph_node();
    get_root().append(std::slice::from_ref(&paragraph))?;
    set_anchor_point(paragraph.get_key(), 0, E)?;
    set_focus_point(paragraph.get_key(), 0, E)?;
    Ok(current())
}

fn select_text(text: &NodeRef, anchor: usize, focus: usize) -> Result<SelectionRef, LexicalError> {
    set_anchor_point(text.get_key(), anchor, T)?;
    set_focus_point(text.get_key(), focus, T)?;
    Ok(current())
}

/// `with a paragraph node selected on some existing text`.
fn existing_text() -> Result<SelectionRef, LexicalError> {
    let text = create_text_node("Existing text...");
    get_root().append(&[paragraph_of(std::slice::from_ref(&text))])?;
    select_text(&text, 16, 16)
}

/// `with a fully-selected text node`.
fn fully_selected_text() -> Result<SelectionRef, LexicalError> {
    let paragraph = create_paragraph_node();
    get_root().append(std::slice::from_ref(&paragraph))?;
    let text = create_text_node("Existing text...");
    paragraph.append(std::slice::from_ref(&text))?;
    select_text(&text, 0, 16)
}

/// The fully selected "Existing text..." with a link holding "link" after
/// it, or before it when `link_first`.
fn fully_selected_text_next_to_link(link_first: bool) -> Result<SelectionRef, LexicalError> {
    let paragraph = create_paragraph_node();
    get_root().append(std::slice::from_ref(&paragraph))?;
    let text = create_text_node("Existing text...");
    let link = link_of("https://", "link");
    if link_first {
        paragraph.append(&[link, text.clone()])?;
    } else {
        paragraph.append(&[text.clone(), link])?;
    }
    select_text(&text, 0, 16)
}

fn heading_of(text: &str) -> NodeRef {
    let heading = create_heading_node(HeadingTagType::H1);
    heading.append(&[create_text_node(text)]).unwrap();
    heading
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with an empty paragraph node selected > a single text node
#[test]
fn empty_paragraph_a_single_text_node() {
    let html = render_after_update(|| {
        select_empty_paragraph()?.insert_nodes(vec![create_text_node("foo")])
    });
    assert_eq!(html, p(&span("foo")));
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with an empty paragraph node selected > two text nodes
#[test]
fn empty_paragraph_two_text_nodes() {
    let html = render_after_update(|| {
        select_empty_paragraph()?
            .insert_nodes(vec![create_text_node("foo"), create_text_node("bar")])
    });
    assert_eq!(html, p(&span("foobar")));
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with an empty paragraph node selected > link insertion without parent element
#[test]
fn empty_paragraph_link_insertion_without_parent_element() {
    let html = render_after_update(|| {
        let selection = select_empty_paragraph()?;
        selection.insert_nodes(vec![
            create_text_node("h"),
            link_of("https://", "ello worl"),
            create_text_node("d"),
        ])
    });
    assert_eq!(
        html,
        p(&format!(
            "{}{}{}",
            span("h"),
            link_html("https://", "ello worl"),
            span("d")
        ))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with an empty paragraph node selected > a single heading node with a child text node
#[test]
fn empty_paragraph_a_single_heading_node_with_a_child_text_node() {
    let html =
        render_after_update(|| select_empty_paragraph()?.insert_nodes(vec![heading_of("foo")]));
    assert_eq!(html, format!("<h1 dir=\"auto\">{}</h1>", span("foo")));
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a paragraph node selected on some existing text > a single text node
#[test]
fn existing_text_a_single_text_node() {
    let html = render_after_update(|| existing_text()?.insert_nodes(vec![create_text_node("foo")]));
    assert_eq!(html, p(&span("Existing text...foo")));
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a paragraph node selected on some existing text > two text nodes
#[test]
fn existing_text_two_text_nodes() {
    let html = render_after_update(|| {
        existing_text()?.insert_nodes(vec![create_text_node("foo"), create_text_node("bar")])
    });
    assert_eq!(html, p(&span("Existing text...foobar")));
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a paragraph node selected on some existing text > a single heading node with a child text node
#[test]
fn existing_text_a_single_heading_node_with_a_child_text_node() {
    let html = render_after_update(|| existing_text()?.insert_nodes(vec![heading_of("foo")]));
    assert_eq!(html, p(&span("Existing text...foo")));
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a paragraph node selected on some existing text > a paragraph with a child text and a child italic text and a child text
#[test]
fn existing_text_a_paragraph_with_a_child_text_and_a_child_italic_text_and_a_child_text() {
    let html = render_after_update(|| {
        let text = create_text_node("AE");
        let paragraph = paragraph_of(std::slice::from_ref(&text));
        get_root().append(std::slice::from_ref(&paragraph))?;
        let selection = select_text(&text, 1, 1)?;
        let inserted_text_c = create_text_node("C");
        inserted_text_c.toggle_format("italic");
        let inserted_paragraph = paragraph_of(&[
            create_text_node("B"),
            inserted_text_c,
            create_text_node("D"),
        ]);
        selection.insert_nodes(vec![inserted_paragraph])?;
        let expected = paragraph
            .get_child_at_index(paragraph.get_children_size() - 2)
            .unwrap();
        assert_collapsed_at(&selection, format!("{}:1:text", expected.get_key()));
        Ok(())
    });
    assert_eq!(
        html,
        p(&format!(
            "{}<em data-lexical-text=\"true\">C</em>{}",
            span("AB"),
            span("DE")
        ))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a fully-selected text node > a single text node
#[test]
fn fully_selected_a_single_text_node() {
    let html =
        render_after_update(|| fully_selected_text()?.insert_nodes(vec![create_text_node("foo")]));
    assert_eq!(html, p(&span("foo")));
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a fully-selected text node followed by an inline element > a single text node
#[test]
fn fully_selected_followed_by_an_inline_element_a_single_text_node() {
    let html = render_after_update(|| {
        fully_selected_text_next_to_link(false)?.insert_nodes(vec![create_text_node("foo")])
    });
    assert_eq!(
        html,
        p(&format!("{}{}", span("foo"), link_html("https://", "link")))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > with a fully-selected text node preceded by an inline element > a single text node
#[test]
fn fully_selected_preceded_by_an_inline_element_a_single_text_node() {
    let html = render_after_update(|| {
        fully_selected_text_next_to_link(true)?.insert_nodes(vec![create_text_node("foo")])
    });
    assert_eq!(
        html,
        p(&format!("{}{}", link_html("https://", "link"), span("foo")))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert non-element nodes correctly > can insert a linebreak node before an inline element node
#[test]
fn can_insert_a_linebreak_node_before_an_inline_element_node() {
    let html = render_after_update(|| {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        let link = create_link_node("https://lexical.dev/", &LinkAttributes::default());
        paragraph.append(std::slice::from_ref(&link))?;
        let text = create_text_node("Lexical");
        link.append(std::slice::from_ref(&text))?;
        text.select(Some(0), Some(0))?;
        insert_nodes(vec![create_line_break_node()])
    });
    // TODO #5109 ElementNode should have a way to control when other nodes
    // can be inserted inside
    assert_eq!(
        html,
        p(&format!(
            "<br>{}",
            link_html("https://lexical.dev/", "Lexical")
        ))
    );
}

fn paragraph_foo() -> NodeRef {
    paragraph_of(&[create_text_node("foo")])
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > with a fully-selected text node > a paragraph node
#[test]
fn block_fully_selected_a_paragraph_node() {
    let html = render_after_update(|| fully_selected_text()?.insert_nodes(vec![paragraph_foo()]));
    assert_eq!(html, p(&span("foo")));
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > with a fully-selected text node followed by an inline element > a paragraph node
#[test]
fn block_fully_selected_followed_by_an_inline_element_a_paragraph_node() {
    let html = render_after_update(|| {
        fully_selected_text_next_to_link(false)?.insert_nodes(vec![paragraph_foo()])
    });
    assert_eq!(
        html,
        p(&format!("{}{}", span("foo"), link_html("https://", "link")))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > with a fully-selected text node preceded by an inline element > a paragraph node
#[test]
fn block_fully_selected_preceded_by_an_inline_element_a_paragraph_node() {
    let html = render_after_update(|| {
        fully_selected_text_next_to_link(true)?.insert_nodes(vec![paragraph_foo()])
    });
    assert_eq!(
        html,
        p(&format!("{}{}", link_html("https://", "link"), span("foo")))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > Can insert link into empty paragraph
#[test]
fn can_insert_link_into_empty_paragraph() {
    let html = render_after_update(|| {
        get_root().append(&[create_paragraph_node()])?;
        insert_nodes(vec![link_of("https://lexical.dev", "Lexical")])
    });
    assert_eq!(html, p(&link_html("https://lexical.dev", "Lexical")));
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > Can insert link into empty paragraph (2)
#[test]
fn can_insert_link_into_empty_paragraph_2() {
    let html = render_after_update(|| {
        get_root().append(&[create_paragraph_node()])?;
        insert_nodes(vec![
            link_of("https://lexical.dev", "Lexical"),
            create_text_node("..."),
        ])
    });
    assert_eq!(
        html,
        p(&format!(
            "{}{}",
            link_html("https://lexical.dev", "Lexical"),
            span("...")
        ))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::LexicalSelectionHelpers tests > can insert block element nodes correctly > Can insert an ElementNode after ShadowRoot
#[test]
fn can_insert_an_element_node_after_shadow_root() {
    let editor = create_test_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<TestShadowRootNode>()],
        ..CreateEditorArgs::default()
    });
    set_test_root_element(&editor);
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select_start()?;
        insert_nodes(vec![
            create_test_shadow_root_node(),
            create_test_element_node(),
        ])
    });
    let html = rendered_html(&editor);
    let empty_div = "<div dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></div>";
    assert!(
        [
            format!("{empty_div}{empty_div}"),
            format!("{empty_div}<p dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p>"),
        ]
        .contains(&html),
        "{html}"
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::extract > Should return the selected node when collapsed on a TextNode
#[test]
fn extract_should_return_the_selected_node_when_collapsed_on_a_text_node() {
    render_after_update(|| {
        let text = create_text_node("Existing text...");
        get_root().append(&[paragraph_of(std::slice::from_ref(&text))])?;
        let selection = select_text(&text, 16, 16)?;
        assert_eq!(keys(&selection.extract()?), [text.get_key()]);
        Ok(())
    });
}

// insertNodes

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::insertNodes > can insert element next to top level decorator node
#[test]
fn can_insert_element_next_to_top_level_decorator_node() {
    // `vi.spyOn(TestDecoratorNode.prototype, 'isInline')
    // .mockReturnValue(false)`: the test nodes with a block decorator.
    let editor = crate::create_editor(CreateEditorArgs {
        nodes: vec![
            NodeClass::of::<TestElementNode>(),
            NodeClass::of::<TestSegmentedNode>(),
            NodeClass::of::<BlockTestDecoratorNode>(),
            NodeClass::of::<TestInlineElementNode>(),
        ],
        ..CreateEditorArgs::default()
    });
    set_test_root_element(&editor);
    update(&editor, || {
        get_root().append(&[
            create_paragraph_node(),
            crate::create::<BlockTestDecoratorNode>(),
            paragraph_of(&[create_text_node("Text after")]),
        ])
    });
    update(&editor, || {
        let selection_node = get_root().get_first_child().unwrap();
        assert!(selection_node.is_element());
        let selection = selection_node.select(None, None)?;
        selection.insert_nodes(vec![paragraph_of(&[create_text_node("Text before")])])
    });
    assert_eq!(
        rendered_html(&editor),
        format!(
            "{}<span data-lexical-decorator=\"true\" contenteditable=\"false\"></span>{}",
            p(&span("Text before")),
            p(&span("Text after"))
        )
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::insertNodes > can insert when previous selection was null
#[test]
fn can_insert_when_previous_selection_was_null() {
    let editor = crate::create_headless_editor(CreateEditorArgs::default());
    update(&editor, || {
        let selection = create_range_selection();
        selection.point_set(crate::Which::Anchor, "root", 0, E)?;
        selection.point_set(crate::Which::Focus, "root", 0, E)?;
        selection.insert_nodes(vec![paragraph_of(&[create_text_node("Text")])])?;
        assert_eq!(get_root().get_text_content(), "Text");
        set_selection(None);
        Ok(())
    });
    update(&editor, || {
        let selection = create_range_selection();
        let text = get_root().get_last_descendant().unwrap();
        selection.point_set(crate::Which::Anchor, text.get_key(), 0, T)?;
        selection.point_set(crate::Which::Focus, text.get_key(), 0, T)?;
        selection.insert_nodes(vec![paragraph_of(&[create_text_node("Before ")])])?;
        assert_eq!(get_root().get_text_content(), "Before Text");
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::insertNodes > can insert when before empty text node
#[test]
fn can_insert_when_before_empty_text_node() {
    render_after_update(|| {
        // Empty text node to test empty text split
        let empty_text_node = create_text_node("");
        get_root().append(&[paragraph_of(&[
            empty_text_node.clone(),
            create_text_node("text"),
        ])])?;
        empty_text_node.select(Some(0), Some(0))?;
        current().insert_nodes(vec![create_text_node("foo")])?;
        assert_eq!(get_root().get_text_content(), "footext");
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::insertNodes > last node is LineBreakNode
#[test]
fn last_node_is_line_break_node() {
    let editor = mounted_test_editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        let selection = paragraph.select(None, None)?;
        assert!(selection.is_range());
        selection.insert_nodes(vec![heading_of("heading"), create_line_break_node()])
    });
    // the lone trailing LineBreakNode collapses to an empty paragraph,
    // rendered with only the managed linebreak
    assert_eq!(
        rendered_html(&editor),
        format!(
            "<h1 dir=\"auto\">{}</h1><p dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p>",
            span("heading")
        )
    );
    editor
        .read_with_mode(crate::EditorReadMode::Latest, || {
            let selected_node = current()
                .point(crate::Which::Anchor)
                .unwrap()
                .get_node()
                .unwrap();
            assert_eq!(selected_node.get_type(), "paragraph");
            assert!(is_heading_node(
                &selected_node.get_previous_sibling().unwrap()
            ));
        })
        .unwrap();
}

// $patchStyleText

fn patch(property: &str, value: StylePatchValue) -> [(String, StylePatchValue); 1] {
    [(property.to_owned(), value)]
}

fn emphasis() -> [(String, StylePatchValue); 1] {
    patch("text-emphasis", "filled".into())
}

fn styled_span(text: &str) -> String {
    format!("<span style=\"text-emphasis: filled;\" data-lexical-text=\"true\">{text}</span>")
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can patch a selection anchored to the end of a TextNode before an inline element
#[test]
fn can_patch_a_selection_anchored_to_the_end_of_a_text_node_before_an_inline_element() {
    let editor = mounted_test_editor();
    update(&editor, || {
        let paragraph = create_paragraph_with_nodes(&[("a", "a", false), ("b", "b", false)]);
        get_root().append(&[paragraph])?;
        get_node_by_key("a")
            .unwrap()
            .insert_after(&link_of("https://", "link"), true)?;
        set_anchor_point("a", 1, T)?;
        set_focus_point("b", 1, T)?;
        patch_style_text(&current(), &emphasis())
    });
    assert_eq!(
        rendered_html(&editor),
        p(&format!(
            "{}<a href=\"https://\">{}</a>{}",
            span("a"),
            styled_span("link"),
            styled_span("b")
        ))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can patch a selection anchored to the end of a TextNode at the end of a paragraph
#[test]
fn can_patch_a_selection_anchored_to_the_end_of_a_text_node_at_the_end_of_a_paragraph() {
    let editor = mounted_test_editor();
    update(&editor, || {
        let paragraph1 = create_paragraph_with_nodes(&[("a", "a", false)]);
        let paragraph2 = create_paragraph_with_nodes(&[("b", "b", false)]);
        get_root().append(&[paragraph1])?;
        get_root().append(&[paragraph2])?;
        set_anchor_point("a", 1, T)?;
        set_focus_point("b", 1, T)?;
        patch_style_text(&current(), &emphasis())
    });
    assert_eq!(
        rendered_html(&editor),
        format!("{}{}", p(&span("a")), p(&styled_span("b")))
    );
}

/// "a" then a link holding "link" (or the link alone, without `with_a`),
/// the selection from `anchor` to `focus` (`None` keys are the link),
/// patched with text-emphasis; the rendered HTML.
fn patch_next_to_link(anchor: At, focus: At, with_a: bool) -> String {
    let editor = mounted_test_editor();
    update(&editor, move || {
        let link = link_of("https://", "link");
        if with_a {
            get_root().append(&[create_paragraph_with_nodes(&[("a", "a", false)])])?;
            get_node_by_key("a").unwrap().insert_after(&link, true)?;
        } else {
            let paragraph = create_paragraph_node();
            get_root().append(std::slice::from_ref(&paragraph))?;
            paragraph.append(std::slice::from_ref(&link))?;
        }
        set_anchor_point(anchor.0.unwrap_or(link.get_key()), anchor.1, anchor.2)?;
        set_focus_point(focus.0.unwrap_or(link.get_key()), focus.1, focus.2)?;
        patch_style_text(&current(), &emphasis())
    });
    rendered_html(&editor)
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can patch a selection that ends on an element
#[test]
fn can_patch_a_selection_that_ends_on_an_element() {
    // Select to end of the link _element_
    assert_eq!(
        patch_next_to_link((Some("a"), 0, T), (None, 1, E), true),
        p(&format!(
            "{}<a href=\"https://\">{}</a>",
            styled_span("a"),
            styled_span("link")
        ))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can patch a reversed selection that ends on an element
#[test]
fn can_patch_a_reversed_selection_that_ends_on_an_element() {
    // Select from the end of the link _element_
    assert_eq!(
        patch_next_to_link((None, 1, E), (Some("a"), 0, T), true),
        p(&format!(
            "{}<a href=\"https://\">{}</a>",
            styled_span("a"),
            styled_span("link")
        ))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can patch a selection that starts and ends on an element
#[test]
fn can_patch_a_selection_that_starts_and_ends_on_an_element() {
    assert_eq!(
        patch_next_to_link((None, 0, E), (None, 1, E), false),
        p(&format!("<a href=\"https://\">{}</a>", styled_span("link")))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can clear a style
#[test]
fn can_clear_a_style() {
    let html = render_after_update(|| {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        let text = create_text_node("text");
        paragraph.append(std::slice::from_ref(&text))?;
        let selection = select_text(&text, 0, text.get_text_content_size())?;
        patch_style_text(&selection, &emphasis())?;
        patch_style_text(&selection, &patch("text-emphasis", StylePatchValue::Remove))
    });
    assert_eq!(html, p(&span("text")));
}

/// A paragraph holding "text", with the caret at its start.
fn caret_in_text() -> Result<SelectionRef, LexicalError> {
    let paragraph = create_paragraph_node();
    get_root().append(std::slice::from_ref(&paragraph))?;
    let text = create_text_node("text");
    paragraph.append(std::slice::from_ref(&text))?;
    select_text(&text, 0, 0)
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can toggle a style on a collapsed selection
#[test]
fn can_toggle_a_style_on_a_collapsed_selection() {
    render_after_update(|| {
        let selection = caret_in_text()?;
        let value =
            |s: &SelectionRef| get_selection_style_value_for_property(s, "text-emphasis", "");
        patch_style_text(&selection, &emphasis())?;
        assert_eq!(value(&selection)?, "filled");
        patch_style_text(&selection, &patch("text-emphasis", StylePatchValue::Remove))?;
        assert_eq!(value(&selection)?, "");
        patch_style_text(&selection, &emphasis())?;
        assert_eq!(value(&selection)?, "filled");
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > updates cached styles when setting on a collapsed selection
#[test]
fn updates_cached_styles_when_setting_on_a_collapsed_selection() {
    render_after_update(|| {
        // First fetch the initial style -- this will cause the CSS cache to
        // be populated with an empty string pointing to an empty style
        // object.
        let selection = caret_in_text()?;
        get_selection_style_value_for_property(&selection, "color", "")?;
        // Now when we set the style, we should _not_ touch the previously
        // created empty style object, but create a new one instead.
        patch_style_text(&selection, &patch("color", "red".into()))?;
        // We can check that result by clearing the style and re-querying
        // it.
        let selection = current();
        selection.set_style("");
        assert_eq!(
            get_selection_style_value_for_property(&selection, "color", "")?,
            ""
        );
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > $getSelectionStyleValueForProperty returns consistent value regardless of selection direction
#[test]
fn get_selection_style_value_for_property_returns_consistent_value_regardless_of_selection_direction()
 {
    render_after_update(|| {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        let unstyled = create_text_node("plain");
        let styled = create_text_node("colored");
        styled.set_style("color: red");
        paragraph.append(std::slice::from_ref(&unstyled))?;
        paragraph.append(std::slice::from_ref(&styled))?;
        // Forward selection: unstyled -> styled
        set_anchor_point(unstyled.get_key(), 0, T)?;
        set_focus_point(styled.get_key(), 7, T)?;
        let forward_value = get_selection_style_value_for_property(&current(), "color", "")?;
        // Backward selection: styled -> unstyled
        set_anchor_point(styled.get_key(), 7, T)?;
        set_focus_point(unstyled.get_key(), 0, T)?;
        let backward_value = get_selection_style_value_for_property(&current(), "color", "")?;
        assert_eq!(forward_value, "");
        assert_eq!(backward_value, "");
        assert_eq!(forward_value, backward_value);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > $getSelectionStyleValueForProperty ignores nodes with zero characters selected at boundaries
#[test]
fn get_selection_style_value_for_property_ignores_nodes_with_zero_characters_selected_at_boundaries()
 {
    render_after_update(|| {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        let styled_a = create_text_node("aaa");
        styled_a.set_style("color: red");
        let styled_b = create_text_node("bbb");
        styled_b.set_style("color: red");
        let different = create_text_node("ccc");
        different.set_style("color: blue");
        paragraph.append(std::slice::from_ref(&styled_a))?;
        paragraph.append(&[styled_b])?;
        paragraph.append(std::slice::from_ref(&different))?;
        // Select from end of styledA to start of different: styledA has 0
        // chars selected (offset at end), different has 0 chars (offset 0),
        // only styledB is fully selected.
        set_anchor_point(styled_a.get_key(), 3, T)?;
        set_focus_point(different.get_key(), 0, T)?;
        assert_eq!(
            get_selection_style_value_for_property(&current(), "color", "")?,
            "red"
        );
        Ok(())
    });
}

/// `can update style of text node that is in %s mode`.
fn can_update_style_of_text_node_that_is_in_mode(mode: &'static str) {
    let html = render_after_update(move || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        let text = create_text_node("first");
        text.set_format_type("bold");
        paragraph.append(std::slice::from_ref(&text))?;
        let text_in_mode = create_text_node("second");
        text_in_mode.set_mode(mode)?;
        paragraph.append(std::slice::from_ref(&text_in_mode))?;
        set_anchor_point(text.get_key(), 3, T)?;
        set_focus_point(text_in_mode.get_key(), 3, T)?;
        patch_style_text(
            &get_selection().unwrap(),
            &patch("font-size", "15px".into()),
        )
    });
    assert_eq!(
        html,
        p(concat!(
            "<strong data-lexical-text=\"true\">fir</strong>",
            "<strong style=\"font-size: 15px;\" data-lexical-text=\"true\">st</strong>",
            "<span style=\"font-size: 15px;\" data-lexical-text=\"true\">second</span>"
        ))
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can update style of text node that is in token mode
#[test]
fn can_update_style_of_text_node_that_is_in_token_mode() {
    can_update_style_of_text_node_that_is_in_mode("token");
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > can update style of text node that is in segmented mode
#[test]
fn can_update_style_of_text_node_that_is_in_segmented_mode() {
    can_update_style_of_text_node_that_is_in_mode("segmented");
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > preserve backward selection when changing style of 2 different text nodes
#[test]
fn preserve_backward_selection_when_changing_style_of_2_different_text_nodes() {
    let editor = mounted_test_editor();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        let first_text = create_text_node("first ");
        first_text.set_format_type("bold");
        paragraph.append(std::slice::from_ref(&first_text))?;
        let second_text = create_text_node("second");
        second_text.set_format_type("italic");
        paragraph.append(std::slice::from_ref(&second_text))?;
        set_anchor_point(second_text.get_key(), 3, T)?;
        set_focus_point(first_text.get_key(), 3, T)?;
        let selection = get_selection().unwrap();
        patch_style_text(&selection, &patch("font-size", "11px".into()))?;
        // `getStartEndPoints()`: the anchor and the focus.
        let new_anchor = selection.point(crate::Which::Anchor).unwrap();
        let new_focus = selection.point(crate::Which::Focus).unwrap();
        assert_eq!(new_anchor.get_node()?.get_text_content(), "sec");
        assert_eq!(new_anchor.offset, 3);
        assert_eq!(new_focus.get_node()?.get_text_content(), "st ");
        assert_eq!(new_focus.offset, 0);
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalSelectionHelpers.test.ts::$patchStyleText > applies a function-valued patch exactly once to an empty element
#[test]
fn applies_a_function_valued_patch_exactly_once_to_an_empty_element() {
    let editor = mounted_test_editor();
    let (selection_style, paragraph_style) =
        crate::upstream_tests::support::update_with(&editor, || {
            let paragraph = create_paragraph_node();
            get_root().append(std::slice::from_ref(&paragraph))?;
            paragraph.select_start()?;
            let selection = get_selection().unwrap();
            // Mirrors the playground font-size +/- buttons, which patch with a
            // function of the previous value rather than a constant.
            let grow = StylePatchValue::Compute(Rc::new(|current_value, _| {
                // `parseInt(currentValue || '', 10)`, NaN being 16.
                let digits: String = current_value
                    .unwrap_or("")
                    .trim_start()
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                let size = digits.parse::<i64>().unwrap_or(16);
                format!("{}px", size + 2)
            }));
            patch_style_text(&selection, &patch("font-size", grow))?;
            Ok((
                current().range().unwrap().style,
                paragraph.element_data().get_text_style().to_owned(),
            ))
        });
    assert_eq!(selection_style, "font-size: 18px;");
    assert_eq!(paragraph_style, "font-size: 18px;");
}

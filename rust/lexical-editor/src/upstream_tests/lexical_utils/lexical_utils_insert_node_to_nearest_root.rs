//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx`.

#![cfg(all(feature = "html", feature = "list"))]

use super::support::{
    create_headless_test_editor_with_dom_decorator, create_test_decorator_node, set_is_inline,
    strip_values,
};
use crate::dom::parse_from_string;
use crate::html::{generate_html_from_nodes, generate_nodes_from_dom};
use crate::upstream_tests::support::update;
use crate::utils::insert_node_to_nearest_root;
use crate::{PointType, Which, create_range_selection, get_root, set_selection};

fn check(
    initial_html: &'static str,
    selection_path: &'static [usize],
    selection_offset: usize,
    expected_html: &'static str,
) {
    let editor = create_headless_test_editor_with_dom_decorator();
    let ed = editor.clone();
    update(&editor, move || {
        // Running init, update, assert in the same update loop to skip
        // text nodes normalization.
        let dom = parse_from_string(initial_html);
        let nodes_to_insert = generate_nodes_from_dom(&ed, &dom)?;
        let root = get_root();
        root.clear()?;
        root.append(&nodes_to_insert)?;
        let mut selection_node = root;
        for &index in selection_path {
            assert!(
                selection_node.is_element(),
                "Expected node to be element (to traverse the tree)"
            );
            selection_node = selection_node.get_child_at_index(index).unwrap();
        }
        // Calling select() would normalize the selection; the test wants
        // it where it is put (on the root, say).
        let selection = create_range_selection();
        let type_ = if selection_node.is_element() {
            PointType::Element
        } else {
            PointType::Text
        };
        for which in [Which::Anchor, Which::Focus] {
            selection.point_set(which, selection_node.get_key(), selection_offset, type_)?;
        }
        set_selection(Some(selection));
        insert_node_to_nearest_root(&set_is_inline(&create_test_decorator_node(), false))?;
        let actual_html = strip_values(&generate_html_from_nodes(&ed, None)?);
        assert_eq!(actual_html, expected_html);
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert into paragraph in between two text nodes
#[test]
fn insert_into_paragraph_in_between_two_text_nodes() {
    check(
        r#"<p><span>Helloworld</span></p>"#,
        &[0, 0],
        5,
        r#"<p><span style="white-space: pre-wrap;">Hello</span></p><test-decorator></test-decorator><p><span style="white-space: pre-wrap;">world</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert into nested list items
#[test]
fn insert_into_nested_list_items() {
    check(
        r#"<ul><li><span>Before</span></li><ul><li><span>Helloworld</span></li></ul><li><span>After</span></li></ul>"#,
        &[0, 1, 0, 0, 0],
        5,
        r#"<ul><li><span style="white-space: pre-wrap;">Before</span><ul><li><span style="white-space: pre-wrap;">Hello</span></li></ul></li></ul><test-decorator></test-decorator><ul><li><ul><li><span style="white-space: pre-wrap;">world</span></li></ul></li><li><span style="white-space: pre-wrap;">After</span></li></ul>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert into empty paragraph
#[test]
fn insert_into_empty_paragraph() {
    check(
        r#"<p></p>"#,
        &[0],
        0,
        r#"<p><br></p><test-decorator></test-decorator><p><br></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert in the end of paragraph
#[test]
fn insert_in_the_end_of_paragraph() {
    check(
        r#"<p>Hello world</p>"#,
        &[0, 0],
        11,
        r#"<p><span style="white-space: pre-wrap;">Hello world</span></p><test-decorator></test-decorator><p><br></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert in the end of paragraph that is followed by another block
#[test]
fn insert_in_the_end_of_paragraph_that_is_followed_by_another_block() {
    check(
        r#"<p>Hello world</p><p>Second</p>"#,
        &[0, 0],
        11,
        r#"<p><span style="white-space: pre-wrap;">Hello world</span></p><test-decorator></test-decorator><p><span style="white-space: pre-wrap;">Second</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert in the end of the last nested list item
#[test]
fn insert_in_the_end_of_the_last_nested_list_item() {
    check(
        r#"<ul><li><span>Before</span></li><ul><li><span>Hello</span></li></ul><li><span>After</span></li></ul>"#,
        &[0, 1, 0, 0, 0],
        5,
        r#"<ul><li><span style="white-space: pre-wrap;">Before</span><ul><li><span style="white-space: pre-wrap;">Hello</span></li></ul></li></ul><test-decorator></test-decorator><ul><li><span style="white-space: pre-wrap;">After</span></li></ul>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert in the beginning of paragraph
#[test]
fn insert_in_the_beginning_of_paragraph() {
    check(
        r#"<p>Hello world</p>"#,
        &[0, 0],
        0,
        r#"<p><br></p><test-decorator></test-decorator><p><span style="white-space: pre-wrap;">Hello world</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert with selection on root start
#[test]
fn insert_with_selection_on_root_start() {
    check(
        r#"<test-decorator></test-decorator><p><span>Before</span></p><p><span>After</span></p>"#,
        &[],
        0,
        r#"<test-decorator></test-decorator><test-decorator></test-decorator><p><span style="white-space: pre-wrap;">Before</span></p><p><span style="white-space: pre-wrap;">After</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert with selection on root child
#[test]
fn insert_with_selection_on_root_child() {
    check(
        r#"<p>Before</p><p>After</p>"#,
        &[],
        1,
        r#"<p><span style="white-space: pre-wrap;">Before</span></p><test-decorator></test-decorator><p><span style="white-space: pre-wrap;">After</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert with selection on root end
#[test]
fn insert_with_selection_on_root_end() {
    check(
        r#"<p>Before</p>"#,
        &[],
        1,
        r#"<p><span style="white-space: pre-wrap;">Before</span></p><test-decorator></test-decorator>"#,
    );
}

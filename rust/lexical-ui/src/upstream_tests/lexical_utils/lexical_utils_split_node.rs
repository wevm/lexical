//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx`.

#![cfg(all(feature = "html", feature = "list"))]

use super::support::{create_headless_test_editor, strip_values};
use crate::dom::parse_from_string;
use crate::get_root;
use crate::html::{generate_html_from_nodes, generate_nodes_from_dom};
use crate::upstream_tests::support::update;
use crate::utils::split_node;

fn check(
    initial_html: &'static str,
    split_path: &'static [usize],
    split_offset: usize,
    expected_html: &str,
) {
    let editor = create_headless_test_editor();
    let ed = editor.clone();
    let expected = expected_html.to_owned();
    update(&editor, move || {
        // Running init, update, assert in the same update loop to skip
        // text nodes normalization.
        let dom = parse_from_string(initial_html);
        let nodes_to_insert = generate_nodes_from_dom(&ed, &dom)?;
        let root = get_root();
        root.clear()?;
        root.append(&nodes_to_insert)?;
        let mut node_to_split = root;
        for &index in split_path {
            let child = node_to_split.get_child_at_index(index).unwrap();
            assert!(child.is_element(), "Expected node to be element");
            node_to_split = child;
        }
        split_node(&node_to_split, split_offset)?;
        let actual_html = strip_values(&generate_html_from_nodes(&ed, None)?);
        assert_eq!(actual_html, expected);
        Ok(())
    });
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx::LexicalUtils#splitNode > split paragraph in between two text nodes
#[test]
fn split_paragraph_in_between_two_text_nodes() {
    check(
        "<p><span>Hello</span><span>world</span></p>",
        &[0],
        1,
        "<p><span style=\"white-space: pre-wrap;\">Hello</span></p><p><span style=\"white-space: pre-wrap;\">world</span></p>",
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx::LexicalUtils#splitNode > split paragraph before the first text node
#[test]
fn split_paragraph_before_the_first_text_node() {
    check(
        "<p><span>Hello</span><span>world</span></p>",
        &[0],
        0,
        "<p><br></p><p><span style=\"white-space: pre-wrap;\">Hello</span><span style=\"white-space: pre-wrap;\">world</span></p>",
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx::LexicalUtils#splitNode > split paragraph after the last text node
#[test]
fn split_paragraph_after_the_last_text_node() {
    check(
        "<p><span>Hello</span><span>world</span></p>",
        &[0],
        2,
        "<p><span style=\"white-space: pre-wrap;\">Hello</span><span style=\"white-space: pre-wrap;\">world</span></p><p><br></p>",
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx::LexicalUtils#splitNode > split list items between two text nodes
#[test]
fn split_list_items_between_two_text_nodes() {
    check(
        "<ul><li><span>Hello</span><span>world</span></li></ul>",
        &[0, 0],
        1,
        "<ul><li><span style=\"white-space: pre-wrap;\">Hello</span></li></ul><ul><li><span style=\"white-space: pre-wrap;\">world</span></li></ul>",
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx::LexicalUtils#splitNode > split list items before the first text node
#[test]
fn split_list_items_before_the_first_text_node() {
    check(
        "<ul><li><span>Hello</span><span>world</span></li></ul>",
        &[0, 0],
        0,
        "<ul><li></li></ul><ul><li><span style=\"white-space: pre-wrap;\">Hello</span><span style=\"white-space: pre-wrap;\">world</span></li></ul>",
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx::LexicalUtils#splitNode > split nested list items
#[test]
fn split_nested_list_items() {
    check(
        "<ul><li><span>Before</span></li><ul><li><span>Hello</span><span>world</span></li></ul><li><span>After</span></li></ul>",
        &[0, 1, 0, 0],
        1,
        concat!(
            "<ul>",
            "<li><span style=\"white-space: pre-wrap;\">Before</span><ul><li><span style=\"white-space: pre-wrap;\">Hello</span></li></ul></li>",
            "</ul>",
            "<ul>",
            "<li><ul><li><span style=\"white-space: pre-wrap;\">world</span></li></ul></li>",
            "<li><span style=\"white-space: pre-wrap;\">After</span></li>",
            "</ul>",
        ),
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx::LexicalUtils#splitNode > throws when splitting root
#[test]
fn throws_when_splitting_root() {
    let editor = create_headless_test_editor();
    update(&editor, || {
        assert!(split_node(&get_root(), 0).is_err());
        Ok(())
    });
}

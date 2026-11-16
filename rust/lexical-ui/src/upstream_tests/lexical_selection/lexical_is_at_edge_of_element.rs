//! Ports of
//! `packages/lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts`.

#![cfg(all(feature = "extension", feature = "link"))]

use crate::extension::build_editor_from_extensions;
use crate::link::{LinkAttributes, create_link_node, link_extension};
use crate::selection::is_at_edge_of_element;
use crate::upstream_tests::support::{TestShadowRootNode, create_test_shadow_root_node, update};
use crate::{
    AnyLexicalExtensionArgument, CaretDirection, LexicalError, LexicalExtension, NodeClass,
    NodeRef, Point, PointType, create_paragraph_node, create_text_node, define_extension, get_root,
    set_slot,
};

use CaretDirection::{Next, Previous};

/// `$pointAt(node, offset, type)`.
fn point_at(node: &NodeRef, offset: usize, point_type: PointType) -> Point {
    Point::new(node.get_key(), offset, point_type)
}

fn at_edge(point: Point, element: &NodeRef, direction: CaretDirection) -> bool {
    is_at_edge_of_element(&point, element, direction).unwrap()
}

/// `runInEditor(fn, spec)`.
fn run_in_editor_with(
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
    dependencies: Vec<AnyLexicalExtensionArgument>,
    nodes: Vec<NodeClass>,
) {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        nodes,
        ..LexicalExtension::<(), (), ()>::new("@isAtEdgeOfElement-test", ())
    })
    .into()])
    .unwrap();
    update(&editor, f);
    editor.dispose();
}

fn run_in_editor(f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    run_in_editor_with(f, Vec::new(), Vec::new());
}

fn append(parent: &NodeRef, children: &[NodeRef]) -> NodeRef {
    parent.append(children).unwrap();
    parent.clone()
}

// upstream: lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > text point at the start/end of a block
#[test]
fn text_point_at_the_start_end_of_a_block() {
    run_in_editor(|| {
        let block = create_paragraph_node();
        let text = create_text_node("hello");
        get_root().append(&[append(&block, std::slice::from_ref(&text))])?;

        // Start of the block.
        assert!(at_edge(
            point_at(&text, 0, PointType::Text),
            &block,
            Previous
        ));
        assert!(!at_edge(point_at(&text, 0, PointType::Text), &block, Next));
        // End of the block.
        assert!(at_edge(point_at(&text, 5, PointType::Text), &block, Next));
        assert!(!at_edge(
            point_at(&text, 5, PointType::Text),
            &block,
            Previous
        ));
        // Middle of the text is at neither edge.
        assert!(!at_edge(
            point_at(&text, 2, PointType::Text),
            &block,
            Previous
        ));
        assert!(!at_edge(point_at(&text, 2, PointType::Text), &block, Next));
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > a sibling between the point and the edge disqualifies it
#[test]
fn a_sibling_between_the_point_and_the_edge_disqualifies_it() {
    run_in_editor(|| {
        let block = create_paragraph_node();
        let a = create_text_node("a");
        let b = create_text_node("b");
        get_root().append(&[append(&block, &[a.clone(), b.clone()])])?;

        // offset 0 of b is not the start (a precedes it).
        assert!(!at_edge(point_at(&b, 0, PointType::Text), &block, Previous));
        // end of b is the end of the block.
        assert!(at_edge(point_at(&b, 1, PointType::Text), &block, Next));
        // end of a is not the end (b follows it).
        assert!(!at_edge(point_at(&a, 1, PointType::Text), &block, Next));
        // start of a is the start of the block.
        assert!(at_edge(point_at(&a, 0, PointType::Text), &block, Previous));
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > descends through nested inline elements
#[test]
fn descends_through_nested_inline_elements() {
    run_in_editor_with(
        || {
            let block = create_paragraph_node();
            let link = create_link_node("https://lexical.dev", &LinkAttributes::default());
            let text = create_text_node("link");
            get_root().append(&[append(
                &block,
                &[append(&link, std::slice::from_ref(&text))],
            )])?;

            // The point is at the edge of both the inline link and the block.
            assert!(at_edge(
                point_at(&text, 0, PointType::Text),
                &block,
                Previous
            ));
            assert!(at_edge(
                point_at(&text, 0, PointType::Text),
                &link,
                Previous
            ));
            assert!(at_edge(point_at(&text, 4, PointType::Text), &block, Next));
            assert!(at_edge(point_at(&text, 4, PointType::Text), &link, Next));
            Ok(())
        },
        vec![link_extension().into()],
        Vec::new(),
    );
}

// upstream: lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > an empty element is at both of its edges
#[test]
fn an_empty_element_is_at_both_of_its_edges() {
    run_in_editor(|| {
        let block = create_paragraph_node();
        get_root().append(std::slice::from_ref(&block))?;

        // This is the geometric answer that backs $isAtStartOfNode /
        // $isAtEndOfNode; the block-aware $setBlocksType path layers an
        // empty guard on top of it.
        assert!(at_edge(
            point_at(&block, 0, PointType::Element),
            &block,
            Previous
        ));
        assert!(at_edge(
            point_at(&block, 0, PointType::Element),
            &block,
            Next
        ));
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > element points at the leading/trailing child edge
#[test]
fn element_points_at_the_leading_trailing_child_edge() {
    run_in_editor(|| {
        let block = create_paragraph_node();
        let a = create_text_node("a");
        let b = create_text_node("b");
        get_root().append(&[append(&block, &[a, b])])?;

        // Before the first child / after the last child.
        assert!(at_edge(
            point_at(&block, 0, PointType::Element),
            &block,
            Previous
        ));
        assert!(at_edge(
            point_at(&block, 2, PointType::Element),
            &block,
            Next
        ));
        // Between the two children is at neither edge.
        assert!(!at_edge(
            point_at(&block, 1, PointType::Element),
            &block,
            Previous
        ));
        assert!(!at_edge(
            point_at(&block, 1, PointType::Element),
            &block,
            Next
        ));
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > a point outside the element is never at its edge
#[test]
fn a_point_outside_the_element_is_never_at_its_edge() {
    run_in_editor(|| {
        let block1 = create_paragraph_node();
        let block2 = create_paragraph_node();
        let text1 = create_text_node("one");
        let text2 = create_text_node("two");
        get_root().append(&[
            append(&block1, std::slice::from_ref(&text1)),
            append(&block2, &[text2]),
        ])?;

        assert!(!at_edge(
            point_at(&text1, 0, PointType::Text),
            &block2,
            Previous
        ));
        assert!(!at_edge(
            point_at(&text1, 3, PointType::Text),
            &block2,
            Next
        ));
        Ok(())
    });
}

// upstream: lexical-selection/src/__tests__/unit/LexicalIsAtEdgeOfElement.test.ts::$isAtEdgeOfElement > resolves against a named slot value (slot boundary)
#[test]
fn resolves_against_a_named_slot_value_slot_boundary() {
    run_in_editor_with(
        || {
            let host = create_paragraph_node();
            let slot = create_test_shadow_root_node();
            let slot_para = create_paragraph_node();
            let slot_text = create_text_node("slotted");
            slot_para.append(std::slice::from_ref(&slot_text))?;
            slot.append(&[slot_para])?;
            host.append(&[create_text_node("body")])?;
            get_root().append(std::slice::from_ref(&host))?;
            set_slot(&host, "title", &slot)?;

            // Start / end of the slot's content, against the slot value
            // itself.
            assert!(at_edge(
                point_at(&slot_text, 0, PointType::Text),
                &slot,
                Previous
            ));
            assert!(at_edge(
                point_at(&slot_text, 7, PointType::Text),
                &slot,
                Next
            ));
            // Not at the opposite edges.
            assert!(!at_edge(
                point_at(&slot_text, 0, PointType::Text),
                &slot,
                Next
            ));
            assert!(!at_edge(
                point_at(&slot_text, 7, PointType::Text),
                &slot,
                Previous
            ));
            // Mid-text is at neither edge.
            assert!(!at_edge(
                point_at(&slot_text, 3, PointType::Text),
                &slot,
                Previous
            ));
            assert!(!at_edge(
                point_at(&slot_text, 3, PointType::Text),
                &slot,
                Next
            ));
            Ok(())
        },
        Vec::new(),
        vec![NodeClass::of::<TestShadowRootNode>()],
    );
}

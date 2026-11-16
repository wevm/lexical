//! Ports of `packages/lexical-mark/__tests__/unit/LexicalMarkNode.test.ts`.
//!
//! "createDOM adds the overlap class for more than one id" is ported in
//! `mark/mark_node.rs`; the two `updateDOM` tests read the element the
//! reconciler patched, and are skipped (rust/parity/skips.json).

use crate::mark::{MarkNode, is_mark_node, wrap_selection_in_mark_node};
use crate::upstream_tests::support::{
    create_test_decorator_node, create_test_element_node, create_test_inline_element_node,
    initialize_unit_test, update,
};
use crate::{
    CreateEditorArgs, Editor, NodeClass, NodeRef, PointType, Which, create_paragraph_node,
    create_range_selection, create_text_node, get_root, set_selection, utf16,
};

/// `initializeUnitTest`'s editor, whose `DEFAULT_NODES` include `MarkNode`,
/// and the `beforeEach` of `$wrapSelectionInMarkNode`: a root holding one
/// empty paragraph.
fn wrap_editor() -> Editor {
    let editor = initialize_unit_test(CreateEditorArgs {
        nodes: vec![NodeClass::of::<MarkNode>()],
        ..CreateEditorArgs::default()
    });
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[create_paragraph_node()])
    });
    editor
}

/// `$assertNodeType($getRoot().getFirstChild(), $isParagraphNode)`.
fn first_paragraph() -> NodeRef {
    let paragraph = get_root().get_first_child().unwrap();
    assert_eq!(paragraph.get_type(), "paragraph");
    paragraph
}

fn assert_mark(node: Option<NodeRef>) -> NodeRef {
    let node = node.expect("a node");
    assert!(is_mark_node(&node), "Expecting MarkNode");
    node
}

fn keys(nodes: &[NodeRef]) -> Vec<String> {
    nodes.iter().map(|n| n.get_key().to_owned()).collect()
}

/// A selection from element offset 0 to the end of `paragraph`.
fn select_paragraph(paragraph: &NodeRef) -> Result<crate::SelectionRef, crate::LexicalError> {
    let selection = create_range_selection();
    selection.point_set(Which::Anchor, paragraph.get_key(), 0, PointType::Element)?;
    selection.point_set(
        Which::Focus,
        paragraph.get_key(),
        paragraph.get_children_size(),
        PointType::Element,
    )?;
    Ok(selection)
}

// upstream: lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > wraps a whole text node
#[test]
fn wraps_a_whole_text_node() {
    let editor = wrap_editor();
    update(&editor, || {
        let text_node = create_text_node("marked");
        let paragraph_node = first_paragraph();
        paragraph_node.append(std::slice::from_ref(&text_node))?;
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, text_node.get_key(), 0, PointType::Text)?;
        selection.point_set(
            Which::Focus,
            text_node.get_key(),
            utf16::len(&text_node.get_text_content()),
            PointType::Text,
        )?;
        wrap_selection_in_mark_node(&selection, false, "my-id", None)?;

        assert_eq!(paragraph_node.get_children().len(), 1);
        let mark_node = assert_mark(paragraph_node.get_first_child());
        assert_eq!(mark_node.get_type(), "mark");
        assert_eq!(mark_node.get_ids().unwrap(), ["my-id"]);
        assert_eq!(mark_node.get_children().len(), 1);
        assert_eq!(
            mark_node.get_first_child_or_throw()?.get_key(),
            text_node.get_key()
        );
        assert_eq!(
            mark_node.get_first_child_or_throw()?.get_text_content(),
            "marked"
        );
        Ok(())
    });
}

// upstream: lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > splits a text node if the selection is not at the start/end
#[test]
fn splits_a_text_node_if_the_selection_is_not_at_the_start_end() {
    let editor = wrap_editor();
    update(&editor, || {
        let text_node = create_text_node("unmarked marked unmarked");
        let paragraph_node = first_paragraph();
        paragraph_node.append(std::slice::from_ref(&text_node))?;
        let selection = create_range_selection();
        selection.point_set(
            Which::Anchor,
            text_node.get_key(),
            "unmarked ".len(),
            PointType::Text,
        )?;
        selection.point_set(
            Which::Focus,
            text_node.get_key(),
            "unmarked marked".len(),
            PointType::Text,
        )?;
        wrap_selection_in_mark_node(&selection, false, "my-id", None)?;

        assert_eq!(
            paragraph_node.get_text_content(),
            "unmarked marked unmarked"
        );
        let children = paragraph_node.get_children();
        assert_eq!(
            children.iter().map(|c| c.get_type()).collect::<Vec<_>>(),
            ["text", "mark", "text"]
        );
        assert_eq!(
            children
                .iter()
                .map(|c| c.get_text_content())
                .collect::<Vec<_>>(),
            ["unmarked ", "marked", " unmarked"]
        );
        Ok(())
    });
}

// upstream: lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > includes inline decorator nodes
#[test]
fn includes_inline_decorator_nodes() {
    let editor = wrap_editor();
    update(&editor, || {
        let decorator_node = create_test_decorator_node();
        let text_node = create_text_node("more text");
        let paragraph_node = first_paragraph();
        paragraph_node.append(&[decorator_node.clone(), text_node.clone()])?;
        let selection = select_paragraph(&paragraph_node)?;
        wrap_selection_in_mark_node(&selection, false, "my-id", None)?;

        assert_eq!(paragraph_node.get_children().len(), 1);
        let mark_node = assert_mark(paragraph_node.get_first_child());
        assert_eq!(mark_node.get_type(), "mark");
        assert_eq!(
            keys(&mark_node.get_children()),
            keys(&[decorator_node, text_node])
        );
        Ok(())
    });
}

// upstream: lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > keeps decorator nodes inside an existing mark
#[cfg(feature = "utils")]
#[test]
fn keeps_decorator_nodes_inside_an_existing_mark() {
    let editor = wrap_editor();
    update(&editor, || {
        let paragraph_node = first_paragraph();
        let text_node = create_text_node("aaa x^2 bbb");
        paragraph_node.append(std::slice::from_ref(&text_node))?;

        let selection = text_node.select(Some(0), None)?;
        wrap_selection_in_mark_node(&selection, false, "comment-id", None)?;

        let mark_node = assert_mark(Some(paragraph_node.get_first_child_or_throw()?));
        let marked_text_node = mark_node.get_first_child_or_throw()?;
        assert!(marked_text_node.is_text(), "Expecting TextNode");
        let equation_selection =
            marked_text_node.select(Some("aaa ".len()), Some("aaa x^2".len()))?;
        set_selection(Some(equation_selection));

        crate::utils::insert_node_into_leaf(&create_test_decorator_node())?;

        assert_eq!(
            paragraph_node
                .get_children()
                .iter()
                .map(|c| c.get_type())
                .collect::<Vec<_>>(),
            ["mark"]
        );
        let updated_mark_node = assert_mark(Some(paragraph_node.get_first_child_or_throw()?));
        assert_eq!(updated_mark_node.get_ids().unwrap(), ["comment-id"]);
        let mark_children = updated_mark_node.get_children();
        assert_eq!(mark_children.len(), 3);
        assert_eq!(mark_children[0].get_text_content(), "aaa ");
        assert_eq!(mark_children[1].get_type(), "test_decorator");
        assert_eq!(mark_children[2].get_text_content(), " bbb");
        Ok(())
    });
}

// upstream: lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > includes inline element nodes
#[test]
fn includes_inline_element_nodes() {
    let editor = wrap_editor();
    update(&editor, || {
        let element_node = create_test_inline_element_node();
        let text_node = create_text_node("more text");
        let paragraph_node = first_paragraph();
        paragraph_node.append(&[element_node.clone(), text_node.clone()])?;
        let selection = select_paragraph(&paragraph_node)?;
        wrap_selection_in_mark_node(&selection, false, "my-id", None)?;

        assert_eq!(paragraph_node.get_children().len(), 1);
        let mark_node = assert_mark(paragraph_node.get_first_child());
        assert_eq!(mark_node.get_type(), "mark");
        assert_eq!(
            keys(&mark_node.get_children()),
            keys(&[element_node, text_node])
        );
        Ok(())
    });
}

// upstream: lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $wrapSelectionInMarkNode > does not include block element nodes
#[test]
fn does_not_include_block_element_nodes() {
    let editor = wrap_editor();
    update(&editor, || {
        let element_node = create_test_element_node();
        let text_node = create_text_node("more text");
        let paragraph_node = first_paragraph();
        paragraph_node.append(&[element_node.clone(), text_node.clone()])?;
        let selection = select_paragraph(&paragraph_node)?;
        wrap_selection_in_mark_node(&selection, false, "my-id", None)?;

        assert_eq!(paragraph_node.get_children().len(), 2);
        assert_eq!(
            paragraph_node.get_child_at_index(0).unwrap().get_key(),
            element_node.get_key()
        );

        // the text part of the selection should still be marked
        let mark_node = assert_mark(paragraph_node.get_child_at_index(1));
        assert_eq!(mark_node.get_type(), "mark");
        assert_eq!(mark_node.get_children().len(), 1);
        assert_eq!(mark_node.get_text_content(), "more text");
        Ok(())
    });
}

// upstream: lexical-mark/__tests__/unit/LexicalMarkNode.test.ts::LexicalMarkNode tests > $generateNodesFromDOM > retains spaces around mark elements
#[cfg(feature = "html")]
#[test]
fn retains_spaces_around_mark_elements() {
    let editor = initialize_unit_test(CreateEditorArgs {
        nodes: vec![NodeClass::of::<MarkNode>()],
        ..CreateEditorArgs::default()
    });
    update(&editor, || get_root().clear());
    let inner = editor.clone();
    update(&editor, move || {
        let dom = crate::dom::parse_from_string(
            "<html><body><p><span>Foo </span><mark>Bar</mark><span> !</span></p></body></html>",
        );
        let nodes = crate::html::generate_nodes_from_dom(&inner, &dom)?;

        assert_eq!(nodes.len(), 1);
        let paragraph_node = &nodes[0];
        assert!(paragraph_node.is_element());
        assert_eq!(paragraph_node.get_children().len(), 3);
        // The <mark> element is imported as plain text in this test, so all
        // three children are TextNodes.
        let texts: Vec<NodeRef> = (0..3)
            .map(|i| paragraph_node.get_child_at_index(i).unwrap())
            .collect();
        assert!(texts.iter().all(NodeRef::is_text));
        assert_eq!(texts[0].get_text_content(), "Foo ");
        assert_eq!(texts[1].get_text_content(), "Bar");
        assert_eq!(texts[2].get_text_content(), " !");
        Ok(())
    });
}

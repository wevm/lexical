//! Ports of `packages/lexical/src/__tests__/unit/IsBlockFullySelected.test.ts`.

use crate::upstream_tests::support::{create_test_decorator_node, initialize_unit_test, update};
use crate::{
    CreateEditorArgs, NodeRef, PointType, RangeSelection, Which, caret_range_from_selection,
    create_paragraph_node, create_text_node, get_root, get_selection, is_block_fully_selected,
    is_block_fully_selected_by_caret_range,
};

/// `$getBlock(index)`.
fn get_block(index: usize) -> NodeRef {
    let node = get_root().get_child_at_index(index);
    let node = node.unwrap_or_else(|| panic!("block {index} must be an ElementNode"));
    assert!(node.is_element(), "block {index} must be an ElementNode");
    node
}

/// `$getBlockText(index)`.
fn get_block_text(index: usize) -> NodeRef {
    let text = get_block(index).get_first_descendant();
    let text = text.unwrap_or_else(|| panic!("block {index} must contain a TextNode"));
    assert!(text.is_text(), "block {index} must contain a TextNode");
    text
}

/// `$currentSelection()`.
fn current_selection() -> RangeSelection {
    get_selection()
        .and_then(|s| s.range())
        .expect("expecting a RangeSelection")
}

fn fully(block: &NodeRef, selection: &RangeSelection) -> bool {
    is_block_fully_selected(block, selection).unwrap()
}

fn paragraph_with(text: &str) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)]).unwrap();
    paragraph
}

fn test_editor() -> crate::Editor {
    initialize_unit_test(CreateEditorArgs {
        namespace: "test".to_owned(),
        ..CreateEditorArgs::default()
    })
}

// upstream: lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > reports whether the selection covers the block
#[test]
fn reports_whether_the_selection_covers_the_block() {
    let editor = test_editor();
    update(&editor, || {
        get_root().append(&[paragraph_with("first"), paragraph_with("second")])?;
        let block = get_block(1);
        block.select(Some(0), Some(block.get_children_size()))?;
        let selection = current_selection();
        assert!(fully(&block, &selection));
        assert!(!fully(&get_root(), &selection));
        assert!(!fully(&get_block(0), &selection));

        get_block_text(1).select(Some(1), Some(3))?;
        assert!(!fully(&block, &current_selection()));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > accepts a CaretRange and backward selections
#[test]
fn accepts_a_caret_range_and_backward_selections() {
    let editor = test_editor();
    update(&editor, || {
        get_root().append(&[paragraph_with("first")])?;
        // backward selection over all of the text
        get_block_text(0).select(Some("first".len()), Some(0))?;
        let selection = current_selection();
        assert!(fully(&get_block(0), &selection));
        assert!(is_block_fully_selected_by_caret_range(
            &get_block(0),
            &caret_range_from_selection(&selection)?
        )?);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > a selection that extends beyond the block still fully selects it
#[test]
fn a_selection_that_extends_beyond_the_block_still_fully_selects_it() {
    let editor = test_editor();
    update(&editor, || {
        get_root().append(&[paragraph_with("first"), paragraph_with("second")])?;
        // whole document, text points
        let selection = get_block_text(0).select(Some(0), Some(0))?;
        selection.point_set(
            Which::Focus,
            get_block_text(1).get_key(),
            "second".len(),
            PointType::Text,
        )?;
        let range = selection.range().unwrap();
        assert!(fully(&get_root(), &range));
        assert!(fully(&get_block(0), &range));
        assert!(fully(&get_block(1), &range));

        // mid-first to mid-second covers neither block fully
        let partial = get_block_text(0).select(Some(2), Some(2))?;
        partial.point_set(
            Which::Focus,
            get_block_text(1).get_key(),
            2,
            PointType::Text,
        )?;
        let range = partial.range().unwrap();
        assert!(!fully(&get_block(0), &range));
        assert!(!fully(&get_block(1), &range));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > handles element points on an ancestor
#[test]
fn handles_element_points_on_an_ancestor() {
    let editor = test_editor();
    update(&editor, || {
        get_root().append(&[paragraph_with("first"), paragraph_with("second")])?;
        // element points on the root covering only the first block
        let selection = get_root().select(Some(0), Some(1))?.range().unwrap();
        assert!(fully(&get_block(0), &selection));
        assert!(!fully(&get_block(1), &selection));
        assert!(!fully(&get_root(), &selection));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > handles blocks that end with a decorator
#[test]
fn handles_blocks_that_end_with_a_decorator() {
    let editor = test_editor();
    update(&editor, || {
        let block = create_paragraph_node();
        block.append(&[create_text_node("text"), create_test_decorator_node()])?;
        get_root().append(std::slice::from_ref(&block))?;
        // element points on the block around all of its children
        let selection = block
            .select(Some(0), Some(block.get_children_size()))?
            .range()
            .unwrap();
        assert!(fully(&block, &selection));

        // stopping before the trailing decorator is not a full selection
        let partial = block
            .select(Some(0), Some(block.get_children_size() - 1))?
            .range()
            .unwrap();
        assert!(!fully(&block, &partial));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > an empty block is fully selected by its caret
#[test]
fn an_empty_block_is_fully_selected_by_its_caret() {
    let editor = test_editor();
    update(&editor, || {
        get_root().append(&[paragraph_with("first"), create_paragraph_node()])?;
        let empty_block = get_block(1);
        empty_block.select(Some(0), Some(0))?;
        assert!(fully(&empty_block, &current_selection()));

        // a selection in another block does not select the empty block
        get_block_text(0).select(Some(0), Some("first".len()))?;
        assert!(!fully(&empty_block, &current_selection()));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > a fully selected block fully selects its ancestor block
#[test]
fn a_fully_selected_block_fully_selects_its_ancestor_block() {
    let editor = test_editor();
    update(&editor, || {
        let text = create_text_node("quoted");
        let inner = create_paragraph_node();
        inner.append(std::slice::from_ref(&text))?;
        let quote = create_paragraph_node();
        quote.append(std::slice::from_ref(&inner))?;
        get_root().append(std::slice::from_ref(&quote))?;
        text.select(Some(0), Some("quoted".len()))?;
        let selection = current_selection();
        assert!(fully(&inner, &selection));
        // the selection covers all of the ancestor's content as well
        assert!(fully(&quote, &selection));

        text.select(Some(1), Some("quoted".len()))?;
        let partial = current_selection();
        assert!(!fully(&inner, &partial));
        assert!(!fully(&quote, &partial));
        Ok(())
    });
}

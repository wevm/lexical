//! Ports of `packages/lexical-list/src/__tests__/unit/utils.test.ts`.

use crate::list::{
    ListType, create_list_item_node, create_list_node, get_list_depth, get_top_list_node,
    is_last_item_in_list,
};
use crate::upstream_tests::support::{initialize_unit_test, update};
use crate::{CreateEditorArgs, Editor, LexicalError, NodeRef, create_paragraph_node, get_root};

fn editor() -> Editor {
    initialize_unit_test(CreateEditorArgs::default())
}

fn bullet() -> NodeRef {
    create_list_node(ListType::Bullet, 1.0)
}

fn items<const N: usize>() -> [NodeRef; N] {
    std::array::from_fn(|_| create_list_item_node(None))
}

fn run(f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    update(&editor(), f);
}

// upstream: lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getListDepth should return the 1-based depth of a list with one levels
#[test]
fn get_list_depth_with_one_level() {
    run(|| {
        let top = bullet();
        get_root().append(std::slice::from_ref(&top))?;
        assert_eq!(get_list_depth(&top)?, 1);
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getListDepth should return the 1-based depth of a list with two levels
#[test]
fn get_list_depth_with_two_levels() {
    run(|| {
        let (top, second) = (bullet(), bullet());
        let [i1, i2, i3] = items();
        get_root().append(std::slice::from_ref(&top))?;
        top.append(&[i1])?;
        top.append(&[i2])?;
        top.append(std::slice::from_ref(&second))?;
        second.append(&[i3])?;
        assert_eq!(get_list_depth(&second)?, 2);
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getListDepth should return the 1-based depth of a list with five levels
#[test]
fn get_list_depth_with_five_levels() {
    run(|| {
        let [l1, l2, l3, l4, l5] = std::array::from_fn(|_| bullet());
        let [i1, i2, i3, i4] = items();
        get_root().append(std::slice::from_ref(&l1))?;
        l1.append(std::slice::from_ref(&i1))?;
        i1.append(std::slice::from_ref(&l2))?;
        l2.append(std::slice::from_ref(&i2))?;
        i2.append(std::slice::from_ref(&l3))?;
        l3.append(std::slice::from_ref(&i3))?;
        i3.append(std::slice::from_ref(&l4))?;
        l4.append(std::slice::from_ref(&i4))?;
        i4.append(std::slice::from_ref(&l5))?;
        assert_eq!(get_list_depth(&l5)?, 5);
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getTopListNode should return the top list node when the list is a direct child of the RootNode
#[test]
fn get_top_list_node_direct_child_of_root() {
    run(|| {
        let (top, second) = (bullet(), bullet());
        let [i1, i2, i3] = items();
        get_root().append(std::slice::from_ref(&top))?;
        top.append(&[i1])?;
        top.append(&[i2])?;
        top.append(std::slice::from_ref(&second))?;
        second.append(std::slice::from_ref(&i3))?;
        assert_eq!(get_top_list_node(&i3)?.get_key(), top.get_key());
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getTopListNode should return the top list node when the list is not a direct child of the RootNode
#[test]
fn get_top_list_node_not_direct_child_of_root() {
    run(|| {
        let paragraph = create_paragraph_node();
        let (top, second) = (bullet(), bullet());
        let [i1, i2, i3] = items();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(std::slice::from_ref(&top))?;
        top.append(&[i1])?;
        top.append(&[i2])?;
        top.append(std::slice::from_ref(&second))?;
        second.append(std::slice::from_ref(&i3))?;
        assert_eq!(get_top_list_node(&i3)?.get_key(), top.get_key());
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getTopListNode should return the top list node when the list item is deeply nested.
#[test]
fn get_top_list_node_deeply_nested() {
    run(|| {
        let paragraph = create_paragraph_node();
        let (top, second, third) = (bullet(), bullet(), bullet());
        let [i1, i2, i3, i4] = items();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(std::slice::from_ref(&top))?;
        top.append(std::slice::from_ref(&i1))?;
        i1.append(std::slice::from_ref(&second))?;
        second.append(std::slice::from_ref(&i2))?;
        i2.append(std::slice::from_ref(&third))?;
        third.append(&[i3])?;
        top.append(std::slice::from_ref(&i4))?;
        assert_eq!(get_top_list_node(&i4)?.get_key(), top.get_key());
        Ok(())
    });
}

/// Root > list > item1 > list > item2 > list > item3.
fn nested() -> Result<[NodeRef; 3], LexicalError> {
    let (top, second, third) = (bullet(), bullet(), bullet());
    let [i1, i2, i3] = items();
    get_root().append(std::slice::from_ref(&top))?;
    top.append(std::slice::from_ref(&i1))?;
    i1.append(std::slice::from_ref(&second))?;
    second.append(std::slice::from_ref(&i2))?;
    i2.append(std::slice::from_ref(&third))?;
    third.append(std::slice::from_ref(&i3))?;
    Ok([i1, i2, i3])
}

/// Root > list > item1, item2.
fn flat() -> Result<[NodeRef; 2], LexicalError> {
    let top = bullet();
    let [i1, i2] = items();
    get_root().append(std::slice::from_ref(&top))?;
    top.append(std::slice::from_ref(&i1))?;
    top.append(std::slice::from_ref(&i2))?;
    Ok([i1, i2])
}

// upstream: lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > isLastItemInList should return true if the listItem is the last in a nested list.
#[test]
fn is_last_item_in_list_last_in_nested() {
    run(|| {
        let [_, _, i3] = nested()?;
        assert!(is_last_item_in_list(&i3));
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > isLastItemInList should return true if the listItem is the last in a non-nested list.
#[test]
fn is_last_item_in_list_last_in_non_nested() {
    run(|| {
        let [_, i2] = flat()?;
        assert!(is_last_item_in_list(&i2));
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > isLastItemInList should return false if the listItem is not the last in a nested list.
#[test]
fn is_last_item_in_list_not_last_in_nested() {
    run(|| {
        let [_, i2, _] = nested()?;
        assert!(!is_last_item_in_list(&i2));
        Ok(())
    });
}

// upstream: lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > isLastItemInList should return true if the listItem is not the last in a non-nested list.
#[test]
fn is_last_item_in_list_not_last_in_non_nested() {
    run(|| {
        let [i1, _] = flat()?;
        assert!(!is_last_item_in_list(&i1));
        Ok(())
    });
}

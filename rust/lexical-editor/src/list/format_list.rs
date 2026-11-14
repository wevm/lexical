//! `formatList.ts`: inserting and removing lists, merging them, numbering
//! their items, indenting and outdenting items, and Enter in an empty item.
//!
//! What JS does differently where `NodeStructureExtension` enforces node
//! structure (`$keptTops`, `$listTarget`, joining only items whose only
//! child is a nested list) and for lists in named slots isn't ported, as
//! neither is.

use super::ListItemNode;
use super::list_item_node::{create_list_item_node, is_list_item_node};
use super::list_node::{ListType, create_list_node, is_list_node};
use super::utils::{
    get_all_list_items, get_new_list_start, get_top_list_node, is_nested_list_node,
    remove_highest_empty_list_parent,
};
use crate::LexicalError;
use crate::caret::CaretDirection;
use crate::caret_utils::{get_child_caret_at_index, normalize_caret, set_point_from_caret};
use crate::error::invariant;
use crate::lexical_selection::{PointType, SelectionRef, Which, get_selection};
use crate::lexical_utils::copy_node;
use crate::node::NodeRef;
use crate::nodes::line_break::LineBreakNode;
use crate::nodes::paragraph::create_paragraph_node;
use crate::selection_deletion::is_js_whitespace;
use crate::selection_insert_nodes::is_block_container;
use crate::utils::get_nearest_node_of_type;

/// `$isLeafNode(node)`: text, a line break (or a decorator, once ported).
fn is_leaf_node(node: &NodeRef) -> bool {
    node.is_text() || node.get_latest().is::<LineBreakNode>()
}

/// `$isSelectingEmptyListItem(anchorNode, nodes)`.
fn is_selecting_empty_list_item(anchor_node: &NodeRef, nodes: &[NodeRef]) -> bool {
    is_list_item_node(anchor_node)
        && (nodes.is_empty()
            || (nodes.len() == 1
                && anchor_node == &nodes[0]
                && anchor_node.get_children_size() == 0))
}

/// `$newListFrom(list, listType)`: a copy of the list with the new type, or
/// a new list for anything else.
fn new_list_from(list: &NodeRef, list_type: ListType) -> NodeRef {
    if is_list_node(list) {
        let copy = copy_node(list);
        copy.set_list_type(list_type);
        copy
    } else {
        create_list_node(list_type, 1.0)
    }
}

/// `append(node, nodesToAppend)`: `ElementNode.splice` at the end, so that
/// `ListItemNode.append`'s merging doesn't apply.
fn append(node: &NodeRef, nodes_to_append: &[NodeRef]) -> Result<(), LexicalError> {
    node.splice(node.get_children_size(), 0, nodes_to_append)
}

/// `$insertList(listType)`: turns the selected blocks into items of a list
/// of `list_type` (merging with a list of the type next to them), and the
/// selected lists into lists of that type.
pub fn insert_list(list_type: ListType) -> Result<(), LexicalError> {
    let Some(selection) = get_selection() else {
        return Ok(());
    };
    let mut nodes = selection.get_nodes()?;
    if let Some(anchor) = selection.point(Which::Anchor) {
        let anchor_node = anchor.get_node()?;
        let anchor_node_parent = anchor_node.get_parent();
        if is_block_container(&anchor_node) {
            match anchor_node.get_first_child() {
                Some(first_child) => nodes = first_child.select_start()?.get_nodes()?,
                None => {
                    let paragraph = create_paragraph_node();
                    anchor_node.append(std::slice::from_ref(&paragraph))?;
                    nodes = paragraph.select(None, None)?.get_nodes()?;
                }
            }
        } else if is_selecting_empty_list_item(&anchor_node, &nodes) {
            if anchor_node_parent.as_ref().is_some_and(is_block_container) {
                let list = create_list_node(list_type, 1.0);
                anchor_node.replace(&list, false)?;
                let list_item = create_list_item_node(None);
                if anchor_node.is_element() {
                    list_item.set_element_format(anchor_node.element_data().get_format_type());
                    list_item.set_indent(anchor_node.get_indent()?)?;
                }
                list.append(&[list_item])?;
            } else if is_list_item_node(&anchor_node) {
                let parent = anchor_node.get_parent_or_throw()?;
                let list = new_list_from(&parent, list_type);
                append(&list, &parent.get_children())?;
                parent.replace(&list, false)?;
            }
            return Ok(());
        }
    }

    let mut handled: Vec<String> = Vec::new();
    for node in &nodes {
        if node.is_element()
            && node.is_empty()
            && !is_list_item_node(node)
            // An empty container the selection passes over stays a container.
            && !is_block_container(node)
            && !handled.iter().any(|k| k == node.get_key())
        {
            create_list_or_merge(node, list_type)?;
            continue;
        }

        let mut parent = if is_leaf_node(node) {
            node.get_parent()
        } else if is_list_item_node(node) && node.is_empty() {
            Some(node.clone())
        } else {
            None
        };

        while let Some(p) = parent {
            if is_block_container(&p) {
                break;
            }
            let parent_key = p.get_key().to_owned();
            if is_list_node(&p) {
                if !handled.contains(&parent_key) {
                    let new_list_node = new_list_from(&p, list_type);
                    append(&new_list_node, &p.get_children())?;
                    p.replace(&new_list_node, false)?;
                    handled.push(parent_key);
                }
                break;
            }
            let next_parent = p.get_parent();
            if next_parent.as_ref().is_some_and(is_block_container) {
                if !handled.contains(&parent_key) {
                    handled.push(parent_key);
                    create_list_or_merge(&p, list_type)?;
                }
                break;
            }
            parent = next_parent;
        }
    }
    Ok(())
}

/// `$createListOrMerge(node, listType)`: the block becomes an item of the
/// list of `list_type` next to it, or of a new list in its place.
fn create_list_or_merge(node: &NodeRef, list_type: ListType) -> Result<NodeRef, LexicalError> {
    if is_list_node(node) {
        return Ok(node.clone());
    }
    let previous_sibling = node.get_previous_sibling();
    let next_sibling = node.get_next_sibling();
    let list_item = create_list_item_node(None);
    append(&list_item, &node.get_children())?;

    let same_type = |n: &Option<NodeRef>| {
        n.as_ref()
            .filter(|n| is_list_node(n) && n.get_list_type() == Some(list_type))
            .cloned()
    };
    let target_list = if let Some(previous_sibling) = same_type(&previous_sibling) {
        previous_sibling.append(std::slice::from_ref(&list_item))?;
        // if the same type of list is on both sides, merge them.
        if let Some(next_sibling) = same_type(&next_sibling) {
            append(&previous_sibling, &next_sibling.get_children())?;
            next_sibling.remove(false)?;
        }
        previous_sibling
    } else if let Some(next_sibling) = same_type(&next_sibling) {
        let first = next_sibling.get_first_child().ok_or_else(|| {
            LexicalError::new(format!(
                "Expected node {} to have a first child.",
                next_sibling.get_key()
            ))
        })?;
        first.insert_before(&list_item, true)?;
        next_sibling
    } else {
        let list = create_list_node(list_type, 1.0);
        list.append(std::slice::from_ref(&list_item))?;
        node.replace(&list, false)?;
        list
    };
    // listItem needs to be attached to root prior to setting indent
    list_item.set_element_format(node.element_data().get_format_type());
    list_item.set_indent(node.get_indent()?)?;

    // Preserve element-anchored selections by updating them to anchor to the
    // listItem instead of the listNode.
    if let Some(selection) = get_selection().filter(SelectionRef::is_range) {
        for which in [Which::Anchor, Which::Focus] {
            let point = selection.point(which).expect("a range selection");
            if point.key == target_list.get_key() {
                selection.point_set(
                    which,
                    list_item.get_key(),
                    point.offset,
                    PointType::Element,
                )?;
            }
        }
    }

    node.remove(false)?;
    Ok(target_list)
}

/// `mergeLists(list1, list2)`: appends the second list's items to the
/// first's, joining nested lists of the same type where they meet, and
/// removes the second.
pub fn merge_lists(list1: &NodeRef, list2: &NodeRef) -> Result<(), LexicalError> {
    join_lists(list1, list2)
}

/// `joinLists(list1, list2, onlyChild)` with `onlyChild` false.
fn join_lists(list1: &NodeRef, list2: &NodeRef) -> Result<(), LexicalError> {
    merge_nested_lists_at_seam(list1.get_last_child(), list2.get_first_child())?;
    let to_merge = list2.get_children();
    if !to_merge.is_empty() {
        list1.append(&to_merge)?;
    }
    list2.remove(false)
}

/// `mergeNestedListsAtSeam(listItem1, listItem2, onlyChild)` with
/// `onlyChild` false: two items, one at the end of a list and one at the
/// start of the next, that wrap nested lists of the same type become one.
fn merge_nested_lists_at_seam(
    list_item1: Option<NodeRef>,
    list_item2: Option<NodeRef>,
) -> Result<(), LexicalError> {
    let (Some(list_item1), Some(list_item2)) = (list_item1, list_item2) else {
        return Ok(());
    };
    if !is_nested_list_node(&list_item1) || !is_nested_list_node(&list_item2) {
        return Ok(());
    }
    let inner1 = list_item1.get_first_child().expect("a nested list");
    let inner2 = list_item2.get_first_child().expect("a nested list");
    // A nested <ul> must not swallow a nested <ol>, same rule as
    // mergeNextSiblingListIfSameType applies at the top level.
    if inner1.get_list_type() == inner2.get_list_type() {
        join_lists(&inner1, &inner2)?;
        list_item2.remove(false)?;
    }
    Ok(())
}

/// `$removeList()`: the lists holding the selection become paragraphs, one
/// per item that holds content.
pub fn remove_list() -> Result<(), LexicalError> {
    let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
        return Ok(());
    };
    let mut list_nodes: Vec<NodeRef> = Vec::new();
    let nodes = selection.get_nodes()?;
    let anchor_node = selection
        .point(Which::Anchor)
        .expect("a range selection")
        .get_node()?;
    if is_selecting_empty_list_item(&anchor_node, &nodes) {
        list_nodes.push(get_top_list_node(&anchor_node)?);
    } else {
        for node in &nodes {
            if is_leaf_node(node)
                && let Some(list_item_node) = get_nearest_node_of_type::<ListItemNode>(node)
            {
                let top = get_top_list_node(&list_item_node)?;
                if !list_nodes.contains(&top) {
                    list_nodes.push(top);
                }
            }
        }
    }

    for list_node in &list_nodes {
        let mut insertion_point = list_node.clone();
        for list_item_node in get_all_list_items(list_node) {
            let (format, style) = selection
                .with_range(|r| (r.format, r.style.clone()))
                .expect("a range selection");
            let paragraph = create_paragraph_node();
            paragraph.set_text_style(&style);
            paragraph.set_text_format(format);
            // The paragraph stands in for the list item, so it keeps the
            // item's own element state.
            let item = list_item_node.element_data();
            paragraph.set_element_format(item.get_format_type());
            paragraph.set_indent(list_item_node.get_indent()?)?;
            paragraph.set_direction(item.get_direction());
            paragraph.set_element_style(item.get_style());

            append(&paragraph, &list_item_node.get_children())?;

            insertion_point.insert_after(&paragraph, true)?;
            insertion_point = paragraph.clone();

            // Points in the item's children move with them. A point on the
            // item itself (an empty item, or a caret beside an inline
            // decorator) moves to the same offset in the paragraph, which
            // holds the same children in the same order.
            for which in [Which::Anchor, Which::Focus] {
                let point = selection.point(which).expect("a range selection");
                if point.key == list_item_node.get_key() {
                    let caret = normalize_caret(&get_child_caret_at_index(
                        &paragraph,
                        point.offset,
                        CaretDirection::Next,
                    ));
                    set_point_from_caret(&selection, which, &caret)?;
                }
            }

            list_item_node.remove(false)?;
        }
        list_node.remove(false)?;
    }
    Ok(())
}

/// `updateChildrenListItemValue(list)`: numbers the list's items from its
/// start (an item that only wraps a nested list has no number), and clears
/// `checked` outside a check list.
pub fn update_children_list_item_value(list: &NodeRef) {
    let is_not_checklist = list.get_list_type() != Some(ListType::Check);
    let mut value = list.get_list_start().unwrap_or(1.0);
    for child in list.get_children() {
        if !is_list_item_node(&child) {
            continue;
        }
        if child.get_list_item_value() != Some(value) {
            child.set_list_item_value(value);
        }
        let checked = child
            .get_latest()
            .downcast_ref::<ListItemNode>()
            .and_then(ListItemNode::checked);
        if is_not_checklist && checked.is_some() {
            child.set_checked(None);
        }
        if !child.get_first_child().is_some_and(|c| is_list_node(&c)) {
            value += 1.0;
        }
    }
}

/// `mergeNextSiblingListIfSameType(list)`: `<ul>` merges with `<ul>`, but
/// not with `<ol>`.
pub fn merge_next_sibling_list_if_same_type(list: &NodeRef) -> Result<(), LexicalError> {
    if let Some(next_sibling) = list.get_next_sibling()
        && is_list_node(&next_sibling)
        && list.get_list_type() == next_sibling.get_list_type()
    {
        merge_lists(list, &next_sibling)?;
    }
    Ok(())
}

/// `$handleIndent(listItemNode)`: nests the item one level deeper, into the
/// nested list next to it or a new one.
pub fn handle_indent(list_item_node: &NodeRef) -> Result<(), LexicalError> {
    if is_nested_list_node(list_item_node) {
        return Ok(());
    }
    let parent = list_item_node.get_parent();
    let next_sibling = list_item_node
        .get_next_sibling()
        .filter(is_nested_list_node);
    let previous_sibling = list_item_node
        .get_previous_sibling()
        .filter(is_nested_list_node);
    // if there are nested lists on either side, merge them all together.
    match (&previous_sibling, &next_sibling) {
        (Some(previous_sibling), Some(next_sibling)) => {
            let inner_list = previous_sibling.get_first_child().expect("a nested list");
            inner_list.append(std::slice::from_ref(list_item_node))?;
            let next_inner_list = next_sibling.get_first_child().expect("a nested list");
            // Only pull in the following sublist when it is the same kind of
            // list, otherwise it loses its listType.
            if inner_list.get_list_type() == next_inner_list.get_list_type() {
                append(&inner_list, &next_inner_list.get_children())?;
                next_sibling.remove(false)?;
            }
        }
        (None, Some(next_sibling)) => {
            // if the ListItemNode is next to a nested ListNode, merge them
            let inner_list = next_sibling.get_first_child().expect("a nested list");
            if let Some(first_child) = inner_list.get_first_child() {
                first_child.insert_before(list_item_node, true)?;
            }
        }
        (Some(previous_sibling), None) => {
            let inner_list = previous_sibling.get_first_child().expect("a nested list");
            inner_list.append(std::slice::from_ref(list_item_node))?;
        }
        (None, None) => {
            // otherwise, we need to create a new nested ListNode
            if let Some(parent) = parent.filter(is_list_node) {
                let previous_sibling = list_item_node.get_previous_sibling();
                let next_sibling = list_item_node.get_next_sibling();
                let new_list_item = copy_node(list_item_node);
                let new_list = copy_node(&parent);
                new_list_item.append(std::slice::from_ref(&new_list))?;
                new_list.append(std::slice::from_ref(list_item_node))?;
                if let Some(previous_sibling) = previous_sibling {
                    previous_sibling.insert_after(&new_list_item, true)?;
                } else if let Some(next_sibling) = next_sibling {
                    next_sibling.insert_before(&new_list_item, true)?;
                } else {
                    parent.append(&[new_list_item])?;
                }
            }
        }
    }
    Ok(())
}

/// `$handleOutdent(listItemNode)`: moves a nested item one level up, into
/// the list holding the item that wraps its list.
pub fn handle_outdent(list_item_node: &NodeRef) -> Result<(), LexicalError> {
    if is_nested_list_node(list_item_node) {
        return Ok(());
    }
    let parent_list = list_item_node.get_parent();
    let grandparent_list_item = parent_list.as_ref().and_then(NodeRef::get_parent);
    let great_grandparent_list = grandparent_list_item.as_ref().and_then(NodeRef::get_parent);
    // If it doesn't have these ancestors, it's not indented.
    let (Some(parent_list), Some(grandparent_list_item), Some(great_grandparent_list)) =
        (parent_list, grandparent_list_item, great_grandparent_list)
    else {
        return Ok(());
    };
    if !(is_list_node(&great_grandparent_list)
        && is_list_item_node(&grandparent_list_item)
        && is_list_node(&parent_list))
    {
        return Ok(());
    }
    let first_child = parent_list.get_first_child();
    let last_child = parent_list.get_last_child();
    let is_first = first_child.as_ref() == Some(list_item_node);
    let is_last = last_child.as_ref() == Some(list_item_node);
    // The general outdent below assumes the wrapper item holds only the
    // nested list: it removes or replaces the wrapper when the item is the
    // list's only item or a middle one, which would delete what else the
    // wrapper holds. Only those cases take the outdent that keeps it.
    if grandparent_list_item.get_children_size() > 1
        && (parent_list.get_children_size() == 1 || (!is_first && !is_last))
    {
        return outdent_from_item_with_content(
            list_item_node,
            &parent_list,
            &grandparent_list_item,
        );
    }
    if is_first {
        // if it's the first child in it's parent list, insert it into the
        // great grandparent list before the grandparent
        grandparent_list_item.insert_before(list_item_node, true)?;
        if parent_list.is_empty() {
            grandparent_list_item.remove(false)?;
        }
    } else if is_last {
        // if it's the last child in it's parent list, insert it into the
        // great grandparent list after the grandparent.
        grandparent_list_item.insert_after(list_item_node, true)?;
        if parent_list.is_empty() {
            grandparent_list_item.remove(false)?;
        }
    } else {
        // otherwise, we need to split the siblings into two new nested lists
        let previous_siblings_list_item = copy_node(list_item_node);
        let previous_siblings_list = copy_node(&parent_list);
        previous_siblings_list_item.append(std::slice::from_ref(&previous_siblings_list))?;
        for sibling in list_item_node.get_previous_siblings() {
            previous_siblings_list.append(&[sibling])?;
        }
        let next_siblings_list_item = copy_node(list_item_node);
        let next_siblings_list = copy_node(&parent_list);
        next_siblings_list_item.append(std::slice::from_ref(&next_siblings_list))?;
        append(&next_siblings_list, &list_item_node.get_next_siblings())?;
        // put the sibling nested lists on either side of the grandparent list
        // item in the great grandparent.
        grandparent_list_item.insert_before(&previous_siblings_list_item, true)?;
        grandparent_list_item.insert_after(&next_siblings_list_item, true)?;
        // replace the grandparent list item (now between the siblings) with
        // the outdented list item.
        grandparent_list_item.replace(list_item_node, false)?;
    }
    Ok(())
}

/// `$outdentFromItemWithContent(listItemNode, parentList, wrapper)`: the
/// outdent when the item wrapping the nested list holds other content too,
/// which stays in reading order around the outdented item.
fn outdent_from_item_with_content(
    list_item_node: &NodeRef,
    parent_list: &NodeRef,
    wrapper: &NodeRef,
) -> Result<(), LexicalError> {
    let next_siblings = list_item_node.get_next_siblings();
    let after = parent_list.get_next_siblings();
    wrapper.insert_after(list_item_node, true)?;
    let mut last = list_item_node.clone();
    if !next_siblings.is_empty() {
        let next_siblings_list_item = copy_node(list_item_node);
        let next_siblings_list = copy_node(parent_list);
        next_siblings_list_item.append(std::slice::from_ref(&next_siblings_list))?;
        append(&next_siblings_list, &next_siblings)?;
        last.insert_after(&next_siblings_list_item, true)?;
        last = next_siblings_list_item;
    }
    if !after.is_empty() {
        let after_list_item = copy_node(wrapper);
        last.insert_after(&after_list_item, true)?;
        append(&after_list_item, &after)?;
    }
    if parent_list.is_empty() {
        parent_list.remove(false)?;
    }
    if wrapper.is_empty() {
        wrapper.remove(false)?;
    }
    Ok(())
}

/// `$handleListInsertParagraph(restoreNumbering)`: Enter in an empty (or
/// blank) item leaves the list, or a nested list, splitting it there.
/// Returns whether it handled the Enter.
pub fn handle_list_insert_paragraph(restore_numbering: bool) -> Result<bool, LexicalError> {
    let Some(selection) = get_selection().filter(SelectionRef::is_range) else {
        return Ok(false);
    };
    let anchor_point = selection.point(Which::Anchor).expect("a range selection");
    if Some(&anchor_point) != selection.point(Which::Focus).as_ref() {
        return Ok(false);
    }
    // Only run this code on empty list items (including whitespace-only)
    let anchor = anchor_point.get_node()?;
    let list_item = if is_list_item_node(&anchor) && anchor.get_children_size() == 0 {
        // Truly empty list item (element selection)
        Some(anchor)
    } else if anchor.is_text() {
        // Check if the entire list item contains only whitespace text nodes
        anchor.get_parent().filter(|parent| {
            is_list_item_node(parent)
                && parent.get_children().iter().all(|node| {
                    node.is_text() && node.get_text_content().chars().all(is_js_whitespace)
                })
        })
    } else {
        None
    };
    let Some(list_item) = list_item else {
        return Ok(false);
    };

    let top_list_node = get_top_list_node(&list_item)?;
    let parent = list_item.get_parent();
    invariant!(
        parent.as_ref().is_some_and(is_list_node),
        "A ListItemNode must have a ListNode for a parent."
    );
    let parent = parent.expect("checked above");
    let grandparent = parent.get_parent();

    let replacement_node = match &grandparent {
        Some(g) if is_block_container(g) => {
            let paragraph = create_paragraph_node();
            top_list_node.insert_after(&paragraph, true)?;
            paragraph
        }
        Some(g) if is_list_item_node(g) => {
            let item = copy_node(g);
            g.insert_after(&item, true)?;
            item
        }
        _ => return Ok(false),
    };
    let (format, style) = selection
        .with_range(|r| (r.format, r.style.clone()))
        .expect("a range selection");
    replacement_node.set_text_style(&style);
    replacement_node.set_text_format(format);
    replacement_node.select(None, None)?;

    let next_siblings = list_item.get_next_siblings();
    if !next_siblings.is_empty() {
        let new_start = if restore_numbering {
            get_new_list_start(&parent, &list_item)
        } else {
            1.0
        };
        let new_list = copy_node(&parent);
        new_list.set_list_start(new_start);
        if is_list_item_node(&replacement_node) {
            let new_list_item = copy_node(&replacement_node);
            new_list_item.append(std::slice::from_ref(&new_list))?;
            replacement_node.insert_after(&new_list_item, true)?;
        } else {
            replacement_node.insert_after(&new_list, true)?;
        }
        new_list.append(&next_siblings)?;
    }

    // Don't leave hanging nested empty lists
    remove_highest_empty_list_parent(&list_item)?;
    Ok(true)
}

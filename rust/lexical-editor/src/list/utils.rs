//! `packages/lexical-list/src/utils.ts`.

use super::list_item_node::is_list_item_node;
use super::list_node::is_list_node;
use crate::LexicalError;
use crate::error::invariant;
use crate::node::NodeRef;

/// `$getListDepth(listNode)`: how many lists, counting this one, the list
/// is nested in.
pub fn get_list_depth(list_node: &NodeRef) -> Result<usize, LexicalError> {
    let mut depth = 1;
    let mut parent = list_node.get_parent();
    while let Some(p) = parent {
        if is_list_item_node(&p) {
            let parent_list = p.get_parent();
            if let Some(parent_list) = parent_list.filter(is_list_node) {
                depth += 1;
                parent = parent_list.get_parent();
                continue;
            }
            invariant!(false, "A ListItemNode must have a ListNode for a parent.");
        }
        return Ok(depth);
    }
    Ok(depth)
}

/// `$getTopListNode(listItem)`: the outermost list holding the item.
pub fn get_top_list_node(list_item: &NodeRef) -> Result<NodeRef, LexicalError> {
    let parent_list = list_item.get_parent();
    let Some(parent_list) = parent_list.filter(is_list_node) else {
        return Err(LexicalError::new(
            "A ListItemNode must have a ListNode for a parent.",
        ));
    };
    let mut list = parent_list.clone();
    let mut parent = Some(parent_list);
    while let Some(p) = parent {
        parent = p.get_parent();
        if let Some(p) = parent.as_ref().filter(|p| is_list_node(p)) {
            list = p.clone();
        }
    }
    Ok(list)
}

/// `$getAllListItems(node)`: the list's items that hold content, nested
/// lists' included, in document order.
pub fn get_all_list_items(node: &NodeRef) -> Vec<NodeRef> {
    let mut list_item_nodes = Vec::new();
    for list_item_node in node.get_children().into_iter().filter(is_list_item_node) {
        match list_item_node.get_first_child().filter(is_list_node) {
            Some(first_child) => list_item_nodes.extend(get_all_list_items(&first_child)),
            None => list_item_nodes.push(list_item_node),
        }
    }
    list_item_nodes
}

/// `$isNestedListNode(node)`: a list item whose first child is a list.
pub fn is_nested_list_node(node: &NodeRef) -> bool {
    is_list_item_node(node) && node.get_first_child().is_some_and(|c| is_list_node(&c))
}

/// `$removeHighestEmptyListParent(sublist)`: removes the item or list, or
/// the highest ancestor item or list that holds nothing else.
pub fn remove_highest_empty_list_parent(sublist: &NodeRef) -> Result<(), LexicalError> {
    let mut empty_list_ptr = sublist.clone();
    while empty_list_ptr.get_next_sibling().is_none()
        && empty_list_ptr.get_previous_sibling().is_none()
    {
        let Some(parent) = empty_list_ptr
            .get_parent()
            .filter(|p| is_list_item_node(p) || is_list_node(p))
        else {
            break;
        };
        empty_list_ptr = parent;
    }
    empty_list_ptr.remove(false)
}

/// `$getNewListStart(list, listItem)`: the start of a list split off at
/// the item, the number it was rendered with.
pub fn get_new_list_start(_list: &NodeRef, list_item: &NodeRef) -> f64 {
    list_item.get_list_item_value().unwrap_or(1.0)
}

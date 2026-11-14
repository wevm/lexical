//! `packages/lexical-selection/src/range-selection.ts`: block types,
//! wrapping, caret movement and the style of the selected text.

use crate::caret::{CaretDirection, RootMode, extend_caret_to_range, flip_direction};
use crate::caret_utils::{caret_from_point, is_extendable_text_point_caret};
use crate::error::invariant;
use crate::lexical_selection::{RangeSelection, get_selection, has_ancestor, set_selection};
use crate::lexical_utils::is_block;
use crate::nodes::line_break::LineBreakNode;
use crate::nodes::tab::TabNode;
use crate::selection_insert_nodes::{
    find_matching_parent, get_previous_selection, is_block_container,
};
use crate::set_dom_style::get_style_object_from_css;
use crate::{
    LexicalError, NodeRef, Point, PointType, SelectionAlter, SelectionGranularity, SelectionRef,
    Which,
};
use std::collections::{HashMap, HashSet};

/// `$copyBlockFormatIndent(srcNode, destNode)`: gives `dest_node` the
/// alignment and the indent of `src_node`, setting each only where they
/// differ. What [`set_blocks_type`] does by default to the block it makes.
pub fn copy_block_format_indent(
    src_node: &NodeRef,
    dest_node: &NodeRef,
) -> Result<(), LexicalError> {
    let format = src_node.element_data().get_format_type();
    let indent = src_node.get_indent()?;
    if format != dest_node.element_data().get_format_type() {
        dest_node.set_element_format(format);
    }
    if indent != dest_node.get_indent()? {
        dest_node.set_indent(indent)?;
    }
    Ok(())
}

/// `$isAtEdgeOfElement(point, element, direction)`: whether the collapsed
/// `point` has nothing before it (`Previous`) or after it (`Next`) inside
/// `element`, at every level from its node up to `element`.
pub fn is_at_edge_of_element(
    point: &Point,
    element: &NodeRef,
    direction: CaretDirection,
) -> Result<bool, LexicalError> {
    // An extendable TextPointCaret has text remaining in `direction`, so the
    // point is in the middle of a TextNode rather than at the element edge.
    let mut caret = Some(caret_from_point(point, direction)?);
    if caret.as_ref().is_some_and(is_extendable_text_point_caret) {
        return Ok(false);
    }
    // Walk up towards element: the point is at the edge only when nothing
    // precedes it in `direction` at every level up to element.
    while let Some(c) = caret {
        let Some(parent) = c.get_parent_at_caret() else {
            return Ok(false);
        };
        if c.get_node_at_caret().is_some() {
            return Ok(false);
        }
        if parent.get_key() == element.get_key() {
            return Ok(true);
        }
        caret = c.get_parent_caret(RootMode::Root);
    }
    Ok(false)
}

/// `$isPointAtBlockEdge(point, block, direction)`: like
/// [`is_at_edge_of_element`], except that a point in an empty element is
/// never at the edge: the selection's intent is then the entire block.
fn is_point_at_block_edge(
    point: &Point,
    block: &NodeRef,
    direction: CaretDirection,
) -> Result<bool, LexicalError> {
    let node = point.get_node()?;
    if node.is_element() && node.is_empty() {
        return Ok(false);
    }
    is_at_edge_of_element(point, block, direction)
}

/// `$afterCreateElement(prevNodeSrc, newNodeDest)` of [`set_blocks_type`]:
/// updates the new block from the one it replaces.
pub type AfterCreateElement = dyn Fn(&NodeRef, &NodeRef) -> Result<(), LexicalError>;

/// `$setBlocksType(selection, $createElement, $afterCreateElement)`:
/// replaces each block the selection touches with a new element from
/// `create_element`, moving the block's children into it. The blocks are
/// those holding the anchor and the focus, and the blocks among
/// `selection.getNodes()`, or, for a selection without points (a node
/// selection), the nearest block holding each of its nodes; a focus that
/// overshoots to the leading edge of the next block leaves that block out.
/// `after_create_element(prev_node, new_node)` runs on each new element
/// before the replacement, [`copy_block_format_indent`] when `None`, as in
/// JS. `NodeRef::replace` (and `ListItemNode`'s override) remaps the points
/// on a replaced block. Nothing happens without a selection.
///
/// Slots aren't ported, so JS's skipping of a block that is a named slot's
/// value (`$getSlotHost(prevNode) !== null`) has nothing to skip.
pub fn set_blocks_type(
    selection: Option<&SelectionRef>,
    create_element: impl Fn() -> NodeRef,
    after_create_element: Option<&AfterCreateElement>,
) -> Result<(), LexicalError> {
    let Some(selection) = selection else {
        return Ok(());
    };
    let after_create_element = after_create_element.unwrap_or(&copy_block_format_indent);
    // Selections tend to not include their containing blocks so we
    // effectively expand it here
    let anchor_and_focus = selection.range().map(|range| (range.anchor, range.focus));
    let mut skip_focus = false;
    let mut focus_block: Option<NodeRef> = None;
    // A JS Map keyed by node key: insertion-ordered, and a key set again
    // keeps its place.
    let mut block_map: Vec<NodeRef> = Vec::new();
    let mut set = |node: NodeRef| {
        if !block_map.contains(&node) {
            block_map.push(node);
        }
    };
    if let Some((anchor, focus)) = &anchor_and_focus {
        let anchor_block = find_matching_parent(&anchor.get_node()?, is_block);
        focus_block = find_matching_parent(&focus.get_node()?, is_block);
        // The focus is the moving edge of the selection, travelling in
        // `direction` (towards the end of the document for a forward
        // selection). When a selection overshoots, its focus lands at the
        // leading edge of focusBlock in that direction — the edge opposite
        // to travel — so focusBlock holds none of the selection and is
        // skipped.
        let direction = if selection.is_backward()? {
            CaretDirection::Previous
        } else {
            CaretDirection::Next
        };
        skip_focus = match &focus_block {
            Some(block) if block.is_element() && anchor_block.as_ref() != Some(block) => {
                is_point_at_block_edge(focus, block, flip_direction(direction))?
            }
            _ => false,
        };
        if let Some(block) = anchor_block.filter(NodeRef::is_element) {
            set(block);
        }
        if let Some(block) = focus_block.clone().filter(NodeRef::is_element)
            && !skip_focus
        {
            set(block);
        }
    }
    for node in selection.get_nodes()? {
        if node.is_element() && is_block(&node) {
            if skip_focus && focus_block.as_ref() == Some(&node) {
                continue;
            }
            set(node);
        } else if anchor_and_focus.is_none()
            && let Some(ancestor_block) =
                find_matching_parent(&node, is_block).filter(NodeRef::is_element)
        {
            set(ancestor_block);
        }
    }
    // Selection remapping is delegated to LexicalNode.replace (and the
    // ListItemNode.replace override): both remap an element-anchored point
    // on the replaced block to {key: replacement, offset: prevSize + offset}.
    for prev_node in block_map {
        if is_block_container(&prev_node) {
            // An empty declared container (roots and shadow roots are never
            // in the map) is not a block to convert: with a selection point
            // in it (either end, in either direction), the new block goes in
            // it, as with $insertList and $wrapNodes. The container stays,
            // so its indent and format are not copied, and the points in it
            // move into the new block (a range keeps its other end). A
            // replacement above may have made a new active selection, so its
            // points count too.
            let mut selections = vec![selection.clone()];
            if let Some(active) = get_selection()
                && !active.ptr_eq(selection)
            {
                selections.push(active);
            }
            let mut points: Vec<(SelectionRef, Which)> = Vec::new();
            for s in selections.into_iter().filter(SelectionRef::is_range) {
                for which in [Which::Anchor, Which::Focus] {
                    if s.point(which).is_some_and(|p| p.key == prev_node.get_key()) {
                        points.push((s.clone(), which));
                    }
                }
            }
            if !points.is_empty() && prev_node.is_empty() {
                let element = create_element();
                prev_node.append(std::slice::from_ref(&element))?;
                for (s, which) in points {
                    s.point_set(which, element.get_key(), 0, PointType::Element)?;
                }
            }
            continue;
        }
        let element = create_element();
        after_create_element(&prev_node, &element)?;
        prev_node.replace(&element, true)?;
    }
    Ok(())
}

/// `isPointAttached(point)`.
fn is_point_attached(point: &Point) -> Result<bool, LexicalError> {
    Ok(point.get_node()?.is_attached())
}

/// `$isLeafNode(node)`: a text node, a line break or a decorator.
fn is_leaf_node(node: &NodeRef) -> bool {
    node.is_text() || node.get_latest().is::<LineBreakNode>() || node.is_decorator()
}

/// `INTERNAL_isInlineContent(node)`: what the node structure normalization
/// treats as inline content: text, line breaks, the character limit's
/// `OverflowNode` (an element whose `isInline()` is false), and any node
/// whose `isInline()` is true. JS tells an `OverflowNode` by its class's
/// `$config` (type `overflow`, extending `ElementNode`); the port by its type.
fn is_inline_content(node: &NodeRef) -> bool {
    node.is_text()
        || node.get_latest().is::<LineBreakNode>()
        || (node.is_element() && node.get_type() == "overflow")
        || node.is_inline()
}

/// `$removeParentEmptyElements(startingNode)`: removes the node and each
/// ancestor below the nearest block container that has no children.
fn remove_parent_empty_elements(starting_node: &NodeRef) -> Result<(), LexicalError> {
    let mut node = Some(starting_node.clone());
    while let Some(n) = node.filter(|n| !is_block_container(n)) {
        let parent_node = n.get_parent();
        if n.get_children_size() == 0 {
            n.remove(true)?;
        }
        node = parent_node;
    }
    Ok(())
}

/// `$wrappedBlock(node, cache)`: the block whose content [`wrap_nodes`]
/// moves for `node`: `node` itself or its nearest ancestor that is an
/// element and not inline content, as the node structure normalization
/// counts it (so a link inside a mark, or the character limit's
/// `OverflowNode`, whose `isInline()` is false, stays in its block). The
/// walk never leaves the block container (root or shadow root) that holds
/// `node`: it returns that container when it gets there. `cache` holds the
/// block found for each node already walked through.
fn wrapped_block(node: &NodeRef, cache: &mut HashMap<String, Option<NodeRef>>) -> Option<NodeRef> {
    let mut walked: Vec<String> = Vec::new();
    let mut block = Some(node.clone());
    let found = loop {
        let Some(b) = block else {
            break None;
        };
        if let Some(found) = cache.get(b.get_key()) {
            break found.clone();
        }
        if is_block_container(&b) || (b.is_element() && !is_inline_content(&b)) {
            break Some(b);
        }
        walked.push(b.get_key().to_owned());
        block = b.get_parent();
    };
    for key in walked {
        cache.insert(key, found.clone());
    }
    found
}

/// `$isBlockLeafInElement(node, cache)`: a leaf that is a block of its own
/// (a block decorator, such as a horizontal rule) in an element that is not
/// a block container but holds blocks only: it is wrapped on its own, as it
/// is in a block container (#9638). Where the parent also holds inline
/// content directly, the parent is the block of that content, and of the
/// leaf. `cache` holds the answer for each parent already looked at.
fn is_block_leaf_in_element(node: &NodeRef, cache: &mut HashMap<String, bool>) -> bool {
    let Some(parent) = node.get_parent() else {
        return false;
    };
    if !is_leaf_node(node) || is_inline_content(node) || is_block_container(&parent) {
        return false;
    }
    *cache
        .entry(parent.get_key().to_owned())
        .or_insert_with(|| parent.get_children().iter().all(|c| !is_inline_content(c)))
}

/// `$isEmptyBlock(node)`: an empty element that is not inline content.
fn is_empty_block(node: &NodeRef) -> bool {
    node.is_element() && node.get_children_size() == 0 && !is_inline_content(node)
}

/// `$childIn(container, node, cache)`: the child of the block container
/// `container` that holds `node`. `cache` holds the child found for each
/// node already walked through.
fn child_in(container: &NodeRef, node: &NodeRef, cache: &mut HashMap<String, NodeRef>) -> NodeRef {
    let mut walked: Vec<String> = Vec::new();
    let mut child = node.clone();
    let mut found = cache.get(child.get_key()).cloned();
    while found.is_none() {
        match child.get_parent() {
            Some(parent) if parent != *container => {
                walked.push(child.get_key().to_owned());
                child = parent;
                found = cache.get(child.get_key()).cloned();
            }
            _ => {
                found = Some(child.clone());
            }
        }
    }
    let found = found.expect("set above");
    for key in walked {
        cache.insert(key, found.clone());
    }
    cache.insert(node.get_key().to_owned(), found.clone());
    found
}

/// `$inlineRun(child)`: the run of inline content `child`, inline content
/// that is a child of a block container, is in: it and its siblings on
/// both sides up to the nearest ones that are not inline content.
fn inline_run(child: &NodeRef) -> Vec<NodeRef> {
    let mut first = child.clone();
    while let Some(prev) = first.get_previous_sibling().filter(is_inline_content) {
        first = prev;
    }
    let mut run = Vec::new();
    let mut it = Some(first);
    while let Some(node) = it.filter(is_inline_content) {
        it = node.get_next_sibling();
        run.push(node);
    }
    run
}

/// `$wrapNodes(selection, createElement, wrappingElement)` (deprecated
/// upstream for `$setBlocksType`): wraps the content of the blocks the
/// selection touches in new elements from `create_element`, which take the
/// blocks' alignment and indent and go where the first block is, all inside
/// `wrapping_element` when given. A selection in an empty element wraps
/// that element's block alone; separate block containers among the nodes
/// are wrapped apart. (A selection in a slot value is left alone in JS;
/// slots aren't ported.)
pub fn wrap_nodes(
    selection: &SelectionRef,
    create_element: impl Fn() -> NodeRef,
    wrapping_element: Option<&NodeRef>,
) -> Result<(), LexicalError> {
    let anchor = selection.point(Which::Anchor);
    let nodes = selection.get_nodes()?;
    let nodes_length = nodes.len();
    if let Some(anchor) = &anchor {
        let anchor_node = anchor.get_node()?;
        if nodes_length == 0
            || (nodes_length == 1
                && anchor.point_type == PointType::Element
                && anchor_node.get_children_size() == 0)
        {
            let Some(target) = wrapped_block(&anchor_node, &mut HashMap::new()) else {
                return Ok(());
            };
            let children = target.get_children();
            let mut element = create_element();
            // An empty container stays and the new block goes in it, so the new
            // block does not take the container's indent and format.
            let into_container = is_block_container(&target);
            if !into_container {
                element.set_element_format(target.element_data().get_format_type());
                element.set_indent(target.get_indent()?)?;
            }
            for child in children {
                element.append(&[child])?;
            }
            if let Some(wrapping_element) = wrapping_element {
                wrapping_element.append(std::slice::from_ref(&element))?;
                element = wrapping_element.clone();
            }
            if into_container {
                target.append(std::slice::from_ref(&element))?;
                element.select_start()?;
            } else {
                target.replace(&element, false)?;
            }
            return Ok(());
        }
    }
    let mut top_level_node: Option<NodeRef> = None;
    let mut descendants: Vec<NodeRef> = Vec::new();
    for node in nodes {
        // Determine whether wrapping has to be broken down into multiple chunks. This can happen if the
        // user selected multiple Root-like nodes that have to be treated separately as if they are
        // their own branch. I.e. you don't want to wrap a whole table, but rather the contents of each
        // of each of the cell nodes.
        if is_block_container(&node) {
            wrap_nodes_impl(selection, &descendants, &create_element, wrapping_element)?;
            descendants = Vec::new();
            top_level_node = Some(node);
        } else if top_level_node
            .as_ref()
            .is_none_or(|top| has_ancestor(&node, top))
        {
            descendants.push(node);
        } else {
            wrap_nodes_impl(selection, &descendants, &create_element, wrapping_element)?;
            descendants = vec![node];
        }
    }
    wrap_nodes_impl(selection, &descendants, &create_element, wrapping_element)
}

/// What `$wrapNodesImpl` wraps for one node: the block whose children move,
/// the key the new element is mapped by, and, for inline content directly
/// in a block container or a block leaf, the run that moves instead.
#[derive(Clone)]
struct Unit {
    block: NodeRef,
    key: String,
    run: Option<Vec<NodeRef>>,
}

/// A removal `$wrapNodesImpl` defers until the new elements are in place.
enum Removal {
    /// `$removeParentEmptyElements(parent)`.
    ParentEmptyElements(NodeRef),
    /// `node.remove(true)`.
    Node(NodeRef),
}

/// `$wrapNodesImpl(selection, nodes, nodesLength, createElement,
/// wrappingElement)` with `nodesLength` the length of `nodes`, as
/// `$wrapNodes` calls it: wraps each block of `nodes` into a new element.
fn wrap_nodes_impl(
    selection: &SelectionRef,
    nodes: &[NodeRef],
    create_element: &dyn Fn() -> NodeRef,
    wrapping_element: Option<&NodeRef>,
) -> Result<(), LexicalError> {
    if nodes.is_empty() {
        return Ok(());
    }
    // The first node that is moved, or that an empty block stands for: the
    // nodes before it give nothing to the new elements (a block that the
    // range starts at the end of), so the elements go where it is.
    let first_node = nodes
        .iter()
        .find(|node| (is_leaf_node(node) && node.get_parent().is_some()) || is_empty_block(node))
        .unwrap_or(&nodes[0])
        .clone();
    let mut element_mapping: HashMap<String, NodeRef> = HashMap::new();
    let mut elements: Vec<NodeRef> = Vec::new();
    // The below logic is to find the right target for us to
    // either insertAfter/insertBefore/append the corresponding
    // elements to. This is made more complicated due to nested
    // structures.
    let mut block_of: HashMap<String, Option<NodeRef>> = HashMap::new();
    let mut child_of: HashMap<String, NodeRef> = HashMap::new();
    let mut block_leaf_parents: HashMap<String, bool> = HashMap::new();
    let Some(first_node_block) = wrapped_block(&first_node, &mut block_of) else {
        return Ok(());
    };
    // For inline content directly in a block container, its run (see
    // $inlineRun) stands for the block, and a block leaf there for itself.
    let mut target = if is_block_container(&first_node_block) {
        let child = child_in(&first_node_block, &first_node, &mut child_of);
        if is_inline_content(&child) {
            inline_run(&child).swap_remove(0)
        } else {
            child
        }
    } else if is_block_leaf_in_element(&first_node, &mut block_leaf_parents) {
        first_node.clone()
    } else {
        first_node_block
    };
    let mut target_is_prev_sibling = false;
    loop {
        if let Some(prev_sibling) = target.get_previous_sibling() {
            target = prev_sibling;
            target_is_prev_sibling = true;
            break;
        }
        target = target.get_parent_or_throw()?;
        if is_block_container(&target) {
            break;
        }
    }
    // Find any top level empty elements
    let empty_elements: HashSet<String> = nodes
        .iter()
        .filter(|node| is_empty_block(node))
        .map(|node| node.get_key().to_owned())
        .collect();
    let mut moved_nodes: HashSet<String> = HashSet::new();
    // The nodes the moves empty are removed once the new elements are in the
    // document: a removal that adjusts the selection (a list item's) compared
    // a point in a new element with one in the document, which then had no
    // common ancestor.
    let mut removals: Vec<Removal> = Vec::new();
    // The block of each node, decided before anything moves: once a block's
    // content is in its new element, a node deep inside it would otherwise
    // find the new element as its block. For inline content directly in a
    // block container, the run it is in (see $inlineRun) stands for the
    // block: the container's blocks are wrapped on their own.
    let mut units: Vec<Option<Unit>> = Vec::new();
    let mut run_of: HashMap<String, Unit> = HashMap::new();
    for node in nodes {
        let node_parent = node.get_parent();
        if let Some(parent) = &node_parent
            && is_block_leaf_in_element(node, &mut block_leaf_parents)
        {
            units.push(Some(Unit {
                block: parent.clone(),
                key: node.get_key().to_owned(),
                run: Some(vec![node.clone()]),
            }));
            continue;
        }
        let block = node_parent.and_then(|parent| wrapped_block(&parent, &mut block_of));
        let Some(block) = block.clone().filter(is_block_container) else {
            units.push(block.map(|block| Unit {
                key: block.get_key().to_owned(),
                block,
                run: None,
            }));
            continue;
        };
        let child = child_in(&block, node, &mut child_of);
        let inline = is_inline_content(&child);
        if !inline && !is_leaf_node(&child) {
            // A block element in the container: its own content is wrapped.
            units.push(None);
            continue;
        }
        let unit = match run_of.get(child.get_key()) {
            Some(unit) => unit.clone(),
            None => {
                // A block leaf (a decorator) is wrapped on its own.
                let run = if inline {
                    inline_run(&child)
                } else {
                    vec![child.clone()]
                };
                let unit = Unit {
                    block,
                    key: run[0].get_key().to_owned(),
                    run: Some(run.clone()),
                };
                for node in &run {
                    run_of.insert(node.get_key().to_owned(), unit.clone());
                }
                unit
            }
        };
        units.push(Some(unit));
    }
    // Move out all leaf nodes into our elements array.
    // If we find a top level empty element, also move make
    // an element for that.
    for (node, unit) in nodes.iter().zip(&units) {
        if let Some(unit) = unit
            && is_leaf_node(node)
            && !moved_nodes.contains(node.get_key())
        {
            let parent = &unit.block;
            if !element_mapping.contains_key(&unit.key) {
                let target_element = create_element();
                if unit.run.is_none() {
                    target_element.set_element_format(parent.element_data().get_format_type());
                    target_element.set_indent(parent.get_indent()?)?;
                }
                elements.push(target_element.clone());
                element_mapping.insert(unit.key.clone(), target_element.clone());
                // Move node and its siblings to the new
                // element.
                let children = match &unit.run {
                    Some(run) => run.clone(),
                    None => parent.get_children(),
                };
                target_element.splice(target_element.get_children_size(), 0, &children)?;
                for child in &children {
                    moved_nodes.insert(child.get_key().to_owned());
                    if child.is_element() {
                        // Skip nested leaf nodes if the parent has already been moved
                        for grandchild in child.get_children() {
                            moved_nodes.insert(grandchild.get_key().to_owned());
                        }
                    }
                }
                removals.push(Removal::ParentEmptyElements(parent.clone()));
            }
        } else if empty_elements.contains(node.get_key()) {
            invariant!(
                node.is_element(),
                "Expected node in emptyElements to be an ElementNode"
            );
            let target_element = create_element();
            target_element.set_element_format(node.element_data().get_format_type());
            target_element.set_indent(node.get_indent()?)?;
            elements.push(target_element);
            removals.push(Removal::Node(node.clone()));
        }
    }
    if let Some(wrapping_element) = wrapping_element {
        for element in &elements {
            wrapping_element.append(std::slice::from_ref(element))?;
        }
    }
    let mut last_element: Option<NodeRef> = None;
    // If our target is Root-like, let's see if we can re-adjust
    // so that the target is the first child instead.
    if is_block_container(&target) {
        if target_is_prev_sibling {
            if let Some(wrapping_element) = wrapping_element {
                target.insert_after(wrapping_element, true)?;
            } else {
                for element in elements.iter().rev() {
                    target.insert_after(element, true)?;
                }
            }
        } else {
            let root_target = target.clone();
            match root_target.get_first_child() {
                None => {
                    if let Some(wrapping_element) = wrapping_element {
                        root_target.append(std::slice::from_ref(wrapping_element))?;
                    } else {
                        for element in &elements {
                            root_target.append(std::slice::from_ref(element))?;
                            last_element = Some(element.clone());
                        }
                    }
                }
                Some(first_child) => {
                    if let Some(wrapping_element) = wrapping_element {
                        first_child.insert_before(wrapping_element, true)?;
                    } else {
                        for element in &elements {
                            first_child.insert_before(element, true)?;
                            last_element = Some(element.clone());
                        }
                    }
                }
            }
        }
    } else if let Some(wrapping_element) = wrapping_element {
        target.insert_after(wrapping_element, true)?;
    } else {
        for element in elements.iter().rev() {
            target.insert_after(element, true)?;
            last_element = Some(element.clone());
        }
    }
    for removal in removals {
        match removal {
            Removal::ParentEmptyElements(parent) => remove_parent_empty_elements(&parent)?,
            Removal::Node(node) => node.remove(true)?,
        }
    }
    let prev_selection = get_previous_selection();
    let prev_range = prev_selection.as_ref().and_then(SelectionRef::range);
    if let (Some(prev_selection), Some(prev_range)) = (&prev_selection, &prev_range)
        && is_point_attached(&prev_range.anchor)?
        && is_point_attached(&prev_range.focus)?
    {
        set_selection(Some(prev_selection.clone_selection()));
    } else if let Some(last_element) = last_element {
        last_element.select_end()?;
    } else {
        selection.set_dirty(true);
    }
    Ok(())
}

/// `$isEditorVerticalOrientation(selection)`: whether the computed style of
/// the anchor's element (or its parent's) has `writing-mode: vertical-rl`.
/// Without a DOM there is no computed style, as for a JS editor that
/// renders nothing, so it is false.
fn is_editor_vertical_orientation(_selection: &RangeSelection) -> bool {
    false
}

/// `$shouldOverrideDefaultCharacterSelection(selection, isBackward)`:
/// whether moving the focus one character in that direction must be done by
/// Lexical (`modify`) rather than the platform: next to a decorator or a
/// block boundary, or at an unmergeable text node beside other text. (The
/// writing mode and direction it adjusts for come from computed styles;
/// see [`is_parent_element_rtl`].)
pub fn should_override_default_character_selection(
    selection: &RangeSelection,
    is_backward: bool,
) -> Result<bool, LexicalError> {
    let is_vertical = is_editor_vertical_orientation(selection);
    // In vertical writing mode, we adjust the direction for correct caret movement
    let mut adjusted_is_backward = if is_vertical {
        !is_backward
    } else {
        is_backward
    };
    // In right-to-left writing mode, we invert the direction for correct caret movement
    if is_parent_element_rtl(selection) {
        adjusted_is_backward = !adjusted_is_backward;
    }
    let focus_caret = caret_from_point(
        &selection.focus,
        if adjusted_is_backward {
            CaretDirection::Previous
        } else {
            CaretDirection::Next
        },
    )?;
    if is_extendable_text_point_caret(&focus_caret) {
        return Ok(false);
    }
    // At an unmergeable TextNode boundary adjacent to another plain TextNode,
    // override so Lexical's modify() can pre-normalize across inline-grid/flex
    // spans (#7301). Restricted to unmergeable nodes to avoid disrupting
    // format-affinity at normal bold/italic boundaries.
    let is_tab = |node: &NodeRef| node.get_latest().is::<TabNode>();
    let origin = focus_caret.origin();
    if focus_caret.is_text_point_caret() && !is_tab(origin) && origin.is_unmergeable() {
        let sibling = focus_caret.get_node_at_caret();
        if sibling.is_some_and(|s| s.is_text() && !is_tab(&s)) {
            return Ok(true);
        }
    }
    for next_caret in extend_caret_to_range(&focus_caret)?.iter() {
        if next_caret.is_child_caret() {
            return Ok(!next_caret.origin().is_inline());
        } else if next_caret.origin().is_element() {
            continue;
        } else if next_caret.origin().is_decorator() {
            return Ok(true);
        }
        break;
    }
    Ok(false)
}

/// `$moveCaretSelection(selection, isHoldingShift, isBackward,
/// granularity)`: `selection.modify`, extending with Shift and moving
/// otherwise.
pub fn move_caret_selection(
    selection: &SelectionRef,
    is_holding_shift: bool,
    is_backward: bool,
    granularity: SelectionGranularity,
) -> Result<(), LexicalError> {
    selection.modify(
        if is_holding_shift {
            SelectionAlter::Extend
        } else {
            SelectionAlter::Move
        },
        is_backward,
        granularity,
    )
}

/// `$isParentElementRTL(selection)`: whether the computed style of the
/// anchor's element (or its parent's) has `direction: rtl`. Without a DOM
/// there is no computed style, as for a JS editor that renders nothing, so
/// it is false.
pub fn is_parent_element_rtl(_selection: &RangeSelection) -> bool {
    false
}

/// `$moveCharacter(selection, isHoldingShift, isBackward)`: moves (or with
/// Shift extends) the selection by one character in the visual direction,
/// which right-to-left text and vertical writing reverse.
pub fn move_character(
    selection: &SelectionRef,
    is_holding_shift: bool,
    is_backward: bool,
) -> Result<(), LexicalError> {
    let (is_rtl, is_vertical) = match selection.range() {
        Some(range) => (
            is_parent_element_rtl(&range),
            is_editor_vertical_orientation(&range),
        ),
        None => (false, false),
    };
    // In vertical-rl writing mode, arrow key directions need to be flipped
    // to match the visual flow of text (top to bottom, right to left)
    let adjusted_is_backward = if is_vertical || is_rtl {
        !is_backward
    } else {
        is_backward
    };
    // Apply the direction adjustment to move the caret
    move_caret_selection(
        selection,
        is_holding_shift,
        adjusted_is_backward,
        SelectionGranularity::Character,
    )
}

/// `$getNodeStyleValueForProperty(node, styleProperty, defaultValue)`.
fn get_node_style_value_for_property(
    node: &NodeRef,
    style_property: &str,
    default_value: &str,
) -> String {
    let css = node.text_data().style;
    let style_object = get_style_object_from_css(&css);
    style_object
        .get(style_property)
        .filter(|v| !v.is_empty())
        .unwrap_or(default_value)
        .to_owned()
}

/// `$getSelectionStyleValueForProperty(selection, styleProperty,
/// defaultValue)`: the property's value in the selected text nodes
/// (`default_value` for one without it), `""` when they disagree, or
/// `default_value` without any. A collapsed range selection with the
/// property in its own style gives that; a range's first node when the
/// selection starts at its end, and its last when it ends at offset 0, hold
/// none of the selected text and are left out.
pub fn get_selection_style_value_for_property(
    selection: &SelectionRef,
    style_property: &str,
    default_value: &str,
) -> Result<String, LexicalError> {
    let mut style_value: Option<String> = None;
    let nodes = selection.get_nodes()?;
    // The anchor/focus boundary handling below is specific to RangeSelection;
    // other selection types (e.g. table) style every node they contain.
    let mut start_node: Option<NodeRef> = None;
    let mut end_node: Option<NodeRef> = None;
    if let Some(range) = selection.range() {
        if range.is_collapsed() && !range.style.is_empty() {
            let style_object = get_style_object_from_css(&range.style);
            if let Some(value) = style_object.get(style_property) {
                return Ok(value.to_owned());
            }
        }
        let is_backward = selection.is_backward()?;
        let (first, last) = if is_backward {
            (&range.focus, &range.anchor)
        } else {
            (&range.anchor, &range.focus)
        };
        let first_node = first.get_node()?;
        let last_node = last.get_node()?;
        // A boundary node contributes no styled text when the selection merely
        // touches its edge: the first node when the start offset is at its very
        // end, and the last node when the end offset is at its very beginning.
        if first_node.is_text() && first.offset == first_node.get_text_content_size() {
            start_node = Some(first_node);
        }
        if last.offset == 0 {
            end_node = Some(last_node);
        }
    }
    for (i, node) in nodes.iter().enumerate() {
        // Skip the excluded boundary node for this position (startNode at the
        // head, endNode elsewhere); both are None when nothing is excluded.
        let excluded = if i == 0 { &start_node } else { &end_node };
        if node.is_text() && excluded.as_ref() != Some(node) {
            let node_style_value =
                get_node_style_value_for_property(node, style_property, default_value);
            match &style_value {
                None => style_value = Some(node_style_value),
                Some(value) if *value != node_style_value => {
                    // multiple text nodes are in the selection and they don't all
                    // have the same style.
                    style_value = Some(String::new());
                    break;
                }
                Some(_) => {}
            }
        }
    }
    Ok(style_value.unwrap_or_else(|| default_value.to_owned()))
}

//! `insertNodes`, `insertParagraph`, `insertLineBreak` and
//! `insertRawText` from `LexicalSelection.ts`, for both selection kinds,
//! with `$insertNodes`, `$generateNodesFromRawText` and `$selectAll`.
//!
//! Slots, `DecoratorNode`, `CodeNode` and declared node structure
//! (`NodeStructureExtension`) are not ported, so the branches for a slot
//! value, a code block, a decorator and a declared block container do
//! nothing here.

use crate::LexicalError;
use crate::caret::CaretDirection;
use crate::caret_utils::{caret_from_point, normalize_caret, set_point_from_caret};
use crate::lexical_selection::{
    Point, PointType, RangeSelection, Selection, SelectionRef, Which, get_selection, set_selection,
};
use crate::lexical_utils::{get_root, is_block};
use crate::node::NodeRef;
use crate::nodes::element::is_root_or_shadow_root;
use crate::nodes::line_break::{LineBreakNode, create_line_break_node};
use crate::nodes::paragraph::create_paragraph_node;
use crate::nodes::tab::create_tab_node;
use crate::nodes::text::create_text_node;
use crate::updates::{get_active_editor, get_active_editor_state};
use std::rc::Rc;

impl SelectionRef {
    /// `selection.insertNodes(nodes)`: `RangeSelection.insertNodes`, which
    /// replaces the selected content with `nodes`, splitting blocks as
    /// needed, and selects the end of what it inserted; or
    /// `NodeSelection.insertNodes`, which inserts them after the last
    /// selected node and removes the selected nodes.
    pub fn insert_nodes(&self, nodes: Vec<NodeRef>) -> Result<(), LexicalError> {
        if self.is_node() {
            node_selection_insert_nodes(self, &nodes)
        } else {
            range_selection_insert_nodes(self, &nodes)
        }
    }

    /// `selection.insertRawText(text)`: on a range selection, inserts the
    /// text with its tabs and line breaks as nodes; on a node selection,
    /// nothing.
    pub fn insert_raw_text(&self, text: &str) -> Result<(), LexicalError> {
        if self.is_node() {
            // `NodeSelection.insertRawText`: "Do nothing?"
            return Ok(());
        }
        self.insert_nodes(generate_nodes_from_raw_text(text))
    }

    /// `selection.insertParagraph()`: splits the block at the selection,
    /// and returns the new block, if one was inserted, with the selection at
    /// its start.
    pub fn insert_paragraph(&self) -> Result<Option<NodeRef>, LexicalError> {
        // The root/shadow-root branch below only splices a paragraph in, so
        // the selected content has to go first.
        if !is_collapsed(self) {
            self.remove_text()?;
        }
        let anchor = anchor_of(self);
        let anchor_node = anchor.get_node()?;
        if anchor.point_type == PointType::Element && is_block_container(&anchor_node) {
            let paragraph = create_paragraph_node();
            anchor_node.splice(anchor.offset, 0, std::slice::from_ref(&paragraph))?;
            paragraph.select(None, None)?;
            return Ok(Some(paragraph));
        }
        let (_, index) = remove_text_and_split_block(self, false)?;
        let block = find_matching_parent(&anchor_of(self).get_node()?, is_block);
        // A block that is a slot value has no room for a sibling paragraph,
        // once slots are ported.
        let Some(block) = block.filter(NodeRef::is_element) else {
            return Err(LexicalError::new(
                "Expected ancestor to be a block ElementNode",
            ));
        };
        let nodes_to_insert = match block.get_child_at_index(index) {
            Some(first_to_append) => {
                let mut nodes = vec![first_to_append.clone()];
                nodes.extend(get_next_siblings(&first_to_append));
                nodes
            }
            None => Vec::new(),
        };
        match block
            .insert_new_after(self, false)?
            .filter(NodeRef::is_element)
        {
            Some(new_block) => {
                new_block.append(&nodes_to_insert)?;
                new_block.select_start()?;
                Ok(Some(new_block))
            }
            // A block that can't be split, such as a CodeNode.
            None => Ok(None),
        }
    }

    /// `selection.insertLineBreak(selectStart)`: inserts a line break,
    /// with the selection after it, or before it when `select_start`.
    pub fn insert_line_break(&self, select_start: bool) -> Result<(), LexicalError> {
        let line_break = create_line_break_node();
        self.insert_nodes(vec![line_break.clone()])?;
        // this is used in MacOS with the command 'ctrl-O' (openLineBreak)
        if select_start {
            let parent = line_break.get_parent_or_throw()?;
            let index = line_break.get_index_within_parent();
            parent.select(index, index)?;
        }
        Ok(())
    }
}

/// A range selection's anchor.
fn anchor_of(selection: &SelectionRef) -> Point {
    selection.point(Which::Anchor).expect("a range selection")
}

/// `isCollapsed()` of a range selection.
fn is_collapsed(selection: &SelectionRef) -> bool {
    selection.with_range(|r| r.is_collapsed()).unwrap_or(false)
}

/// `NodeSelection.insertNodes(nodes)`.
fn node_selection_insert_nodes(
    selection: &SelectionRef,
    nodes: &[NodeRef],
) -> Result<(), LexicalError> {
    // Slotted nodes would be skipped here, once slots are ported.
    let selected_nodes = selection.get_nodes()?;
    let Some(last_selected_node) = selected_nodes.last() else {
        return Ok(());
    };
    // Insert nodes
    let selection_at_end = if last_selected_node.is_text() {
        last_selected_node.select(None, None)?
    } else {
        let index = last_selected_node
            .get_index_within_parent()
            .map_or(0, |i| i + 1);
        last_selected_node
            .get_parent_or_throw()?
            .select(Some(index), Some(index))?
    };
    selection_at_end.insert_nodes(nodes.to_vec())?;
    // Remove selected nodes
    for node in &selected_nodes {
        node.remove(false)?;
    }
    Ok(())
}

/// `RangeSelection.insertNodes(nodes)`.
fn range_selection_insert_nodes(
    selection: &SelectionRef,
    nodes: &[NodeRef],
) -> Result<(), LexicalError> {
    if nodes.is_empty() {
        return Ok(());
    }
    if !is_collapsed(selection) {
        selection.remove_text()?;
    }
    // An anchor on a slot value root redirects into the slot's subtree, once
    // slots are ported.
    let anchor = anchor_of(selection);
    let anchor_node = anchor.get_node()?;

    // The anchor is an element point directly on a block container: splice
    // the nodes in at the anchor offset, inline runs wrapped in a block.
    if anchor.point_type == PointType::Element && is_block_container(&anchor_node) {
        let blocks_parent = wrap_inline_nodes(nodes)?;
        let node_to_select = blocks_parent.get_last_descendant();
        anchor_node.splice(anchor.offset, 0, &blocks_parent.get_children())?;
        if let Some(node_to_select) = node_to_select {
            node_to_select.select_end()?;
        }
        return Ok(());
    }

    let first_point_which = if selection.is_backward()? {
        Which::Focus
    } else {
        Which::Anchor
    };
    let first_point = selection
        .point(first_point_which)
        .expect("a range selection");
    let mut first_node = first_point.get_node()?;
    let mut first_block = find_matching_parent(&first_node, is_block);
    // JS compares `firstBlock` with `lastInsertedBlock` by object identity
    // below. The object it holds is the latest version only while no
    // `getWritable` clones the node, which happens unless the node is
    // already writable in this update.
    let mut first_block_is_latest = first_block.as_ref().is_some_and(is_clone_not_needed);

    let last = nodes.last().expect("not empty");

    // CASE 1: insert inside a code block. CodeNode (`__language`) is not
    // ported.

    // CASE 2: All elements of the array are inline
    // (A DecoratorNode that is not inline counts too, once it is ported.)
    let not_inline = |node: &NodeRef| node.is_element() && !node.is_inline();

    if !nodes.iter().any(not_inline) {
        let Some(first_block) = first_block.filter(NodeRef::is_element) else {
            return Err(expected_block_ancestor(&first_node, "block ElementNode"));
        };
        // The split walk normally ends on firstBlock, but it stops early on
        // an inline ElementNode that cannot be split (#6477).
        let (container, index) = remove_text_and_split_block(selection, true)?;
        let insertion_parent = if container.is_element() {
            container
        } else {
            first_block
        };
        insertion_parent.splice(index, 0, nodes)?;
        last.select_end()?;
        return Ok(());
    }

    // CASE 3a: there is non-inline content but no block ancestor to insert
    // it relative to: a malformed document. JS inserts the blocks with
    // `$insertNodeToNearestRootAtCaret`, which is not ported yet.
    let Some(found_block) = first_block.clone() else {
        return Err(LexicalError::new(
            "insertNodes: inserting blocks with no block ancestor ($insertNodeToNearestRootAtCaret) is not ported yet",
        ));
    };

    // CASE 3b: the block cannot receive block siblings (a block inside an
    // inline-only element, #8724; a slot value too, once slots are ported).
    // Flatten the incoming content.
    if found_block.is_element()
        && !found_block.is_parent_required()
        && !is_block_container(&found_block.get_parent_or_throw()?)
    {
        let (_, index) = remove_text_and_split_block(selection, false)?;
        let inline_nodes = extract_inline_from_blocks(nodes);
        found_block.splice(index, 0, &inline_nodes)?;
        match inline_nodes.last() {
            Some(last_inserted) => {
                last_inserted.select_end()?;
            }
            None => {
                found_block.select(Some(index), Some(index))?;
            }
        }
        return Ok(());
    }

    // CASE 3: At least 1 element of the array is not inline
    let blocks_parent = wrap_inline_nodes(nodes)?;
    let node_to_select = blocks_parent
        .get_last_descendant()
        .ok_or_else(|| LexicalError::new("insertNodes: nothing to select"))?;
    let blocks = blocks_parent.get_children();
    // An empty line before the insertion point reads as a block boundary
    // (#4815).
    let is_after_empty_line = is_point_after_empty_line(&first_point)?;
    let is_mergeable = |node: &NodeRef, first_block: Option<&NodeRef>| {
        !is_after_empty_line
            && node.is_element()
            && is_block(node)
            && !node.is_empty()
            && first_block
                .is_some_and(|b| b.is_element() && (!b.is_empty() || b.can_merge_when_empty()))
    };

    let should_insert = !found_block.is_element() || !found_block.is_empty();
    let inserted_paragraph = if should_insert {
        selection.insert_paragraph()?
    } else {
        None
    };
    if inserted_paragraph.is_some() && !found_block.is_attached() {
        first_node = anchor_of(selection).get_node()?;
        first_block = find_matching_parent(&first_node, is_block);
        first_block_is_latest = first_block.as_ref().is_some_and(is_clone_not_needed);
    }
    let last_to_insert = blocks.last();
    let mut first_to_insert = blocks.first();
    if let Some(first) = first_to_insert
        && is_mergeable(first, first_block.as_ref())
    {
        let Some(first_block) = first_block.as_ref().filter(|b| b.is_element()) else {
            return Err(expected_block_ancestor(&first_node, "block ElementNode"));
        };
        first_block.append(&first.get_children())?;
        first_to_insert = blocks.get(1);
    }
    if let Some(first_to_insert) = first_to_insert {
        let Some(first_block) = &first_block else {
            return Err(expected_block_ancestor(&first_node, "block"));
        };
        insert_range_after(first_block, first_to_insert)?;
    }
    let last_inserted_block = find_matching_parent(&node_to_select, is_block);

    // Mark where the inserted content ends before the split-off content is
    // moved in after it.
    let insert_selection = node_to_select.select_end()?;

    if let Some(inserted_paragraph) = &inserted_paragraph {
        match last_inserted_block.as_ref().filter(|b| {
            b.is_element()
                && (inserted_paragraph.can_merge_when_empty()
                    || last_to_insert.is_some_and(is_block))
        }) {
            Some(last_inserted_block) => {
                last_inserted_block.append(&inserted_paragraph.get_children())?;
                inserted_paragraph.remove(false)?;
            }
            None => {
                // The split-off paragraph could not be merged into the last
                // inserted block, but it holds no content either.
                if inserted_paragraph.is_empty() {
                    inserted_paragraph.remove(false)?;
                }
            }
        }
    }
    if let Some(first_block) = first_block.as_ref().filter(|b| b.is_element())
        && first_block.is_empty()
    {
        first_block.remove(false)?;
    }

    // To understand this take a look at the test "can wrap post-linebreak
    // nodes into new element"
    let last_child = first_block
        .as_ref()
        .filter(|b| b.is_element())
        .and_then(NodeRef::get_last_child);
    // `lastInsertedBlock !== firstBlock` compares objects: the same node
    // differs from a `firstBlock` that has been cloned since.
    let same_block =
        first_block.is_some() && last_inserted_block == first_block && first_block_is_latest;
    if let Some(last_child) = last_child
        && last_child.get_latest().is::<LineBreakNode>()
        && !same_block
    {
        last_child.remove(false)?;
    }

    // Resolve the marked caret onto the text beside it.
    let normalized_caret = normalize_caret(&caret_from_point(
        &anchor_of(&insert_selection),
        CaretDirection::Next,
    )?);
    set_point_from_caret(&insert_selection, Which::Anchor, &normalized_caret)?;
    set_point_from_caret(&insert_selection, Which::Focus, &normalized_caret)?;
    Ok(())
}

/// The invariant `Expected node %s of type %s to have a block ElementNode
/// ancestor` (or `a block ancestor`).
fn expected_block_ancestor(node: &NodeRef, what: &str) -> LexicalError {
    LexicalError::new(format!(
        "Expected node {} of type {} to have a {what} ancestor",
        node.get_key(),
        node.get_type()
    ))
}

/// Whether the node is already writable in this update (JS's
/// `_cloneNotNeeded`), so `getWritable` changes the object JS code holds
/// rather than replacing it: nothing else, such as the previous editor
/// state, shares its value.
fn is_clone_not_needed(node: &NodeRef) -> bool {
    get_active_editor_state()
        .0
        .node_map
        .borrow()
        .get(node.get_key())
        .is_some_and(|n| Rc::strong_count(n) == 1)
}

/// `$isBlockContainer(node)`: the root or a shadow root. A block element
/// whose class declares `children: 'block'` counts too in an editor that
/// enforces node structure, once that is ported.
pub(crate) fn is_block_container(node: &NodeRef) -> bool {
    node.is_element() && is_root_or_shadow_root(&*node.get_latest())
}

/// `$findMatchingParent(startingNode, findFn)`: the node or its nearest
/// ancestor below the root that matches.
pub fn find_matching_parent(
    starting_node: &NodeRef,
    find_fn: impl Fn(&NodeRef) -> bool,
) -> Option<NodeRef> {
    let mut curr = Some(starting_node.clone());
    while let Some(node) = curr.filter(|n| !n.is_root()) {
        if find_fn(&node) {
            return Some(node);
        }
        curr = node.get_parent();
    }
    None
}

/// `getNextSiblings()`.
fn get_next_siblings(node: &NodeRef) -> Vec<NodeRef> {
    let mut siblings = Vec::new();
    let mut sibling = node.get_next_sibling();
    while let Some(s) = sibling {
        sibling = s.get_next_sibling();
        siblings.push(s);
    }
    siblings
}

/// `insertRangeAfter(node, firstToInsert)`: inserts `first_to_insert` and
/// its next siblings after `node`.
fn insert_range_after(node: &NodeRef, first_to_insert: &NodeRef) -> Result<(), LexicalError> {
    let mut nodes_to_insert = vec![first_to_insert.clone()];
    nodes_to_insert.extend(get_next_siblings(first_to_insert));
    let mut current_node = node.clone();
    for node_to_insert in &nodes_to_insert {
        current_node = current_node.insert_after(node_to_insert, true)?;
    }
    Ok(())
}

/// `$getNodeBeforePoint(point)`: the node just before the point in its
/// block, looking through the boundaries of inline elements.
fn get_node_before_point(point: &Point) -> Result<Option<NodeRef>, LexicalError> {
    let node = point.get_node()?;
    if point.offset > 0 {
        // An element point has the child before it; a text point has
        // characters before it, so no node is adjacent.
        return Ok(
            if point.point_type == PointType::Element && node.is_element() {
                node.get_child_at_index(point.offset - 1)
            } else {
                None
            },
        );
    }
    let mut child = Some(node);
    while let Some(c) = child.filter(|c| !is_block(c) && !is_root_or_shadow_root(&*c.get_latest()))
    {
        if let Some(previous_sibling) = c.get_previous_sibling() {
            return Ok(Some(previous_sibling));
        }
        child = c.get_parent();
    }
    Ok(None)
}

/// `$isPointAfterEmptyLine(point)`: whether two line breaks come just
/// before the point.
fn is_point_after_empty_line(point: &Point) -> Result<bool, LexicalError> {
    let is_line_break = |node: &NodeRef| node.get_latest().is::<LineBreakNode>();
    Ok(get_node_before_point(point)?.is_some_and(|node_before| {
        is_line_break(&node_before)
            && node_before
                .get_previous_sibling()
                .is_some_and(|n| is_line_break(&n))
    }))
}

/// `$extractInlineFromBlocks(nodes)`: the inline content of `nodes`, with
/// blocks replaced by their inline content and line breaks dropped. (A
/// block `DecoratorNode` is dropped too, once it is ported.)
fn extract_inline_from_blocks(nodes: &[NodeRef]) -> Vec<NodeRef> {
    let mut inline_nodes = Vec::new();
    for node in nodes {
        if node.get_latest().is::<LineBreakNode>() {
            continue;
        }
        if node.is_element() && !node.is_inline() {
            inline_nodes.extend(extract_inline_from_blocks(&node.get_children()));
            continue;
        }
        inline_nodes.push(node.clone());
    }
    inline_nodes
}

/// `$removeTextAndSplitBlock(selection, stopAtUnsplittableNode)`: removes
/// the selected text and splits the ancestors of the anchor up to the
/// nearest block, and returns the node to insert into and the index in it.
fn remove_text_and_split_block(
    selection: &SelectionRef,
    stop_at_unsplittable_node: bool,
) -> Result<(NodeRef, usize), LexicalError> {
    let mut selection_ = selection.clone();
    if !is_collapsed(selection) {
        selection_.remove_text()?;
    }
    // A new selection can originate as a result of node replacement, in
    // which case is registered via $setSelection
    if let Some(new_selection) = get_selection().filter(SelectionRef::is_range) {
        selection_ = new_selection;
    }
    let anchor = anchor_of(&selection_);
    let mut node = anchor.get_node()?;
    let mut offset = anchor.offset;
    // The walk stops at a slot value too, once slots are ported.
    while !is_block(&node) {
        let prev_node = node.clone();
        (node, offset) = split_node_at_point(&node, offset, stop_at_unsplittable_node)?;
        if prev_node == node {
            break;
        }
    }
    Ok((node, offset))
}

/// `$splitNodeAtPoint(node, offset, stopAtUnsplittableNode)`: splits
/// `node` at `offset`, and returns the parent that holds the two halves and
/// the index between them.
fn split_node_at_point(
    node: &NodeRef,
    offset: usize,
    stop_at_unsplittable_node: bool,
) -> Result<(NodeRef, usize), LexicalError> {
    let Some(parent) = node.get_parent() else {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select(None, None)?;
        return Ok((get_root(), 0));
    };
    let index_within_parent = || {
        node.get_index_within_parent()
            .expect("a child of its parent")
    };

    if node.is_text() {
        let split = node.split_text(&[offset as i64])?;
        let Some(first) = split.first() else {
            return Ok((parent, index_within_parent()));
        };
        let x = usize::from(offset != 0);
        let index = first
            .get_index_within_parent()
            .expect("a child of its parent")
            + x;
        return Ok((parent, index));
    }

    if !node.is_element() || offset == 0 {
        return Ok((parent, index_within_parent()));
    }

    if let Some(first_to_append) = node.get_child_at_index(offset) {
        let insert_point = SelectionRef::new(Selection::Range(RangeSelection::new(
            Point::new(node.get_key(), offset, PointType::Element),
            Point::new(node.get_key(), offset, PointType::Element),
            0.0,
            "",
        )));
        // JS passes no `restoreSelection`, which `insertAfter` defaults to
        // true.
        match node.insert_new_after(&insert_point, true)? {
            Some(new_element) if new_element.is_element() => {
                let mut nodes = vec![first_to_append.clone()];
                nodes.extend(get_next_siblings(&first_to_append));
                new_element.append(&nodes)?;
            }
            _ if stop_at_unsplittable_node => return Ok((node.clone(), offset)),
            _ => {}
        }
    }
    Ok((parent, index_within_parent() + 1))
}

/// `$isInlineRunNode(node)`: a node `$wrapInlineNodes` puts in a block of
/// its own run.
fn is_inline_run_node(node: &NodeRef) -> bool {
    let latest = node.get_latest();
    latest.is::<LineBreakNode>()
        // `$isInlineElementOrDecoratorNode`; inline decorators come with
        // DecoratorNode.
        || (latest.element().is_some() && latest.is_inline())
        || latest.text().is_some()
        || latest.is_parent_required()
}

/// `$wrapInlineNodes(nodes)`: a detached paragraph holding `nodes`, each
/// run of inline nodes wrapped in its required parent (a paragraph).
fn wrap_inline_nodes(nodes: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    // We temporarily insert the topLevelNodes into an arbitrary ElementNode,
    // since insertAfter does not work on nodes that have no parent (TO-DO:
    // fix that).
    let virtual_root = create_paragraph_node();
    let mut current_block: Option<NodeRef> = None;
    for (i, node) in nodes.iter().enumerate() {
        if is_inline_run_node(node) {
            let block = match &current_block {
                Some(block) => block.clone(),
                None => {
                    let block = node.create_parent_element_node();
                    virtual_root.append(std::slice::from_ref(&block))?;
                    current_block = Some(block.clone());
                    // A LineBreakNode that is an entire run by itself
                    // collapses to an empty paragraph, since the block
                    // boundary already provides the visual newline.
                    let next_node = nodes.get(i + 1);
                    if node.get_latest().is::<LineBreakNode>()
                        && next_node.is_none_or(|n| !is_inline_run_node(n))
                    {
                        continue;
                    }
                    block
                }
            };
            block.append(std::slice::from_ref(node))?;
        } else {
            virtual_root.append(std::slice::from_ref(node))?;
            current_block = None;
        }
    }
    Ok(virtual_root)
}

/// `RawTextVisitor`: what [`tokenize_raw_text`] calls for each token.
pub trait RawTextVisitor {
    fn linebreak(&mut self);
    fn tab(&mut self);
    /// A non-empty run of text.
    fn text(&mut self, text: &str);
}

/// `tokenizeRawText(text, visitor)`: splits `text` at `\n`, `\r\n` and
/// `\t` (`text.split(/(\r?\n|\t)/)`), and calls the visitor for each token
/// in order, skipping empty text runs.
pub fn tokenize_raw_text(text: &str, visitor: &mut impl RawTextVisitor) {
    let mut start = 0;
    for (i, c) in text.char_indices() {
        match c {
            '\n' => {
                let end = if text[..i].ends_with('\r') && i > start {
                    i - 1
                } else {
                    i
                };
                if end > start {
                    visitor.text(&text[start..end]);
                }
                visitor.linebreak();
                start = i + 1;
            }
            '\t' => {
                if i > start {
                    visitor.text(&text[start..i]);
                }
                visitor.tab();
                start = i + 1;
            }
            _ => {}
        }
    }
    if text.len() > start {
        visitor.text(&text[start..]);
    }
}

/// `$generateNodesFromRawText(text)`: text, line break and tab nodes for
/// `text`, split at `\n`, `\r\n` and `\t`.
pub fn generate_nodes_from_raw_text(text: &str) -> Vec<NodeRef> {
    struct Nodes(Vec<NodeRef>);
    impl RawTextVisitor for Nodes {
        fn linebreak(&mut self) {
            self.0.push(create_line_break_node());
        }
        fn tab(&mut self) {
            self.0.push(create_tab_node());
        }
        fn text(&mut self, text: &str) {
            self.0.push(create_text_node(text));
        }
    }
    let mut nodes = Nodes(Vec::new());
    tokenize_raw_text(text, &mut nodes);
    nodes.0
}

/// `$getPreviousSelection()`: the committed editor state's selection.
pub(crate) fn get_previous_selection() -> Option<SelectionRef> {
    let editor = get_active_editor()?;
    editor.get_editor_state().0.selection.borrow().clone()
}

/// `$insertNodes(nodes)`: inserts `nodes` at the selection, the previous
/// one if there is none, or else the end of the root.
pub fn insert_nodes(nodes: Vec<NodeRef>) -> Result<(), LexicalError> {
    let selection = match get_selection().or_else(get_previous_selection) {
        Some(selection) => selection,
        None => get_root().select_end()?,
    };
    selection.insert_nodes(nodes)
}

/// `$selectAll()`: selects the whole root, with the points moved down to
/// text where they can be, and returns the selection. (JS's
/// `$selectAll(selection)`, which scopes an existing selection, is not
/// ported.)
pub fn select_all() -> Result<SelectionRef, LexicalError> {
    let root = get_root();
    let new_selection = root.select(Some(0), Some(root.get_children_size()))?;
    set_selection(Some(normalize_selection_for_select_all(
        &new_selection,
        &root,
    )?));
    Ok(new_selection)
}

/// `$normalizeSelectionForSelectAll(selection, container)`: normalizes the
/// points, unless the select-all spans a document whose points would both
/// descend into the same top-level shadow root (#6938).
fn normalize_selection_for_select_all(
    selection: &SelectionRef,
    container: &NodeRef,
) -> Result<SelectionRef, LexicalError> {
    let range = selection.range().expect("a range selection");
    normalize_selection(selection)?;
    if !container.is_root() {
        return Ok(selection.clone());
    }
    let anchor_top = get_root_child_ancestor(&anchor_of(selection).get_node()?);
    let focus = selection.point(Which::Focus).expect("a range selection");
    if let Some(anchor_top) = anchor_top
        && anchor_top.is_element()
        && anchor_top.get_latest().is_shadow_root()
        && get_root_child_ancestor(&focus.get_node()?).as_ref() == Some(&anchor_top)
    {
        let (anchor, focus) = (&range.anchor, &range.focus);
        selection.point_set(Which::Anchor, &anchor.key, anchor.offset, anchor.point_type)?;
        selection.point_set(Which::Focus, &focus.key, focus.offset, focus.point_type)?;
    }
    Ok(selection.clone())
}

/// `$getRootChildAncestor(node)`: the node or its ancestor that is a child
/// of the root.
fn get_root_child_ancestor(node: &NodeRef) -> Option<NodeRef> {
    let mut current = node.clone();
    loop {
        // A detached node.
        let parent = current.get_parent()?;
        if parent.is_root() {
            return Some(current);
        }
        current = parent;
    }
}

/// `$normalizeSelection(selection)` (`LexicalNormalization.ts`): moves
/// element points down to the deepest text or element point.
pub(crate) fn normalize_selection(selection: &SelectionRef) -> Result<(), LexicalError> {
    normalize_point(selection, Which::Anchor)?;
    normalize_point(selection, Which::Focus)
}

/// `$normalizePoint(point)`.
fn normalize_point(selection: &SelectionRef, which: Which) -> Result<(), LexicalError> {
    loop {
        let point = selection.point(which).expect("a range selection");
        if point.point_type != PointType::Element {
            return Ok(());
        }
        let node = point.get_node()?;
        let offset = point.offset;
        let (next_node, next_offset_at_end) = if offset == node.get_children_size() {
            (
                offset
                    .checked_sub(1)
                    .and_then(|i| node.get_child_at_index(i)),
                true,
            )
        } else {
            (node.get_child_at_index(offset), false)
        };
        let Some(next_node) = next_node else {
            return Ok(());
        };
        // `set(..., onlyIfChanged)` always changes the point here: the node
        // differs.
        if next_node.is_text() {
            let offset = if next_offset_at_end {
                next_node.get_text_content_size()
            } else {
                0
            };
            return selection.point_set(which, next_node.get_key(), offset, PointType::Text);
        } else if !next_node.is_element() {
            return Ok(());
        }
        let offset = if next_offset_at_end {
            next_node.get_children_size()
        } else {
            0
        };
        selection.point_set(which, next_node.get_key(), offset, PointType::Element)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tokens(Vec<String>);
    impl RawTextVisitor for Tokens {
        fn linebreak(&mut self) {
            self.0.push("<br>".to_owned());
        }
        fn tab(&mut self) {
            self.0.push("<tab>".to_owned());
        }
        fn text(&mut self, text: &str) {
            self.0.push(text.to_owned());
        }
    }

    /// `tokenizeRawText` splits as `text.split(/(\r?\n|\t)/)` does: a
    /// `\r` only goes with the `\n` right after it.
    #[test]
    fn tokenize_raw_text_matches_js_split() {
        let tokens = |text: &str| {
            let mut tokens = Tokens(Vec::new());
            tokenize_raw_text(text, &mut tokens);
            tokens.0
        };
        assert_eq!(
            tokens("a\r\nb\tc\n"),
            ["a", "<br>", "b", "<tab>", "c", "<br>"]
        );
        assert_eq!(tokens("\r\r\n\r"), ["\r", "<br>", "\r"]);
        assert_eq!(tokens("\r\n\r\n"), ["<br>", "<br>"]);
        assert_eq!(tokens("é\t\t😀"), ["é", "<tab>", "<tab>", "😀"]);
        assert!(tokens("").is_empty());
    }
}

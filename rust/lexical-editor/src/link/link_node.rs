//! `LexicalLinkNode.ts`: `LinkNode` and `AutoLinkNode`, their `$create*`
//! and `$is*` functions, `$linkNodeTransform`, `TOGGLE_LINK_COMMAND`,
//! `$toggleLink` and `formatUrl`.

use super::generated_json::{
    export_auto_link_node, export_compact_auto_link_node, export_compact_link_node,
    export_link_node, update_auto_link_node, update_link_node,
};
use crate::caret::{CaretDirection, PointCaret, get_child_caret};
use crate::caret_utils::{
    SplitAtPointCaretNextOptions, caret_from_point, insert_node_to_nearest_root_at_caret,
    normalize_caret, rewind_sibling_caret, set_point_from_caret,
};
use crate::editor::LexicalCommand;
use crate::error::invariant;
use crate::json::Object;
use crate::lexical_selection::{
    Point, PointType, SelectionRef, Which, get_selection, set_selection,
};
use crate::lexical_utils::{copy_node, set_node_key, with_writable};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::ElementData;
use crate::selection_insert_nodes::normalize_selection;
use crate::utils::find_matching_parent;
use crate::{LexicalError, NodeKey};
use std::collections::HashSet;

/// `LinkAttributes`: a link's `rel`, `target` and `title`. Each is `None`
/// where JS's is `undefined` (left as it is, or `null` on a new link),
/// `Some(None)` where it is `null`, and otherwise the string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkAttributes {
    pub rel: Option<Option<String>>,
    pub target: Option<Option<String>>,
    pub title: Option<Option<String>>,
}

impl LinkAttributes {
    /// `{...self, ...overrides}`: each attribute `overrides` carries
    /// replaces this one's.
    pub fn merging(&self, overrides: &LinkAttributes) -> LinkAttributes {
        LinkAttributes {
            rel: overrides.rel.clone().or_else(|| self.rel.clone()),
            target: overrides.target.clone().or_else(|| self.target.clone()),
            title: overrides.title.clone().or_else(|| self.title.clone()),
        }
    }
}

/// `AutoLinkAttributes`: [`LinkAttributes`] and `isUnlinked` (`None` for
/// `undefined` or `null`, which mean `false`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutoLinkAttributes {
    pub rel: Option<Option<String>>,
    pub target: Option<Option<String>>,
    pub title: Option<Option<String>>,
    pub is_unlinked: Option<bool>,
}

/// `LinkNode`: an inline element linking its children to a URL.
#[derive(Clone, Debug, Default)]
pub struct LinkNode {
    base: NodeBase,
    pub(super) element: ElementData,
    pub(super) url: String,
    pub(super) target: Option<String>,
    pub(super) rel: Option<String>,
    pub(super) title: Option<String>,
}

/// `AutoLinkNode`: a [`LinkNode`] made while typing (by `registerAutoLink`,
/// the `autolink` feature) that records whether it was ever unlinked. A
/// `LinkNode` in every other respect: [`is_link_node`] holds for it.
#[derive(Clone, Debug, Default)]
pub struct AutoLinkNode {
    pub(super) link: LinkNode,
    pub(super) is_unlinked: bool,
}

impl LinkNode {
    /// `new LinkNode(url, attributes)`: an attribute that is `undefined`
    /// is `null`.
    fn new(
        url: &str,
        rel: &Option<Option<String>>,
        target: &Option<Option<String>>,
        title: &Option<Option<String>>,
    ) -> Self {
        LinkNode {
            url: url.to_owned(),
            rel: rel.clone().flatten(),
            target: target.clone().flatten(),
            title: title.clone().flatten(),
            ..LinkNode::default()
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    pub fn rel(&self) -> Option<&str> {
        self.rel.as_deref()
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

impl AutoLinkNode {
    /// The fields `AutoLinkNode` inherits from `LinkNode`.
    pub fn link(&self) -> &LinkNode {
        &self.link
    }

    pub fn is_unlinked(&self) -> bool {
        self.is_unlinked
    }
}

/// `$createLinkNode(url, attributes)`.
pub fn create_link_node(url: &str, attributes: &LinkAttributes) -> NodeRef {
    set_node_key(Box::new(LinkNode::new(
        url,
        &attributes.rel,
        &attributes.target,
        &attributes.title,
    )))
}

/// `$isLinkNode(node)`: a `LinkNode` or an `AutoLinkNode`.
pub fn is_link_node(node: &NodeRef) -> bool {
    link_of(&*node.get_latest()).is_some()
}

/// `$createAutoLinkNode(url, attributes)`.
pub fn create_auto_link_node(url: &str, attributes: &AutoLinkAttributes) -> NodeRef {
    set_node_key(Box::new(AutoLinkNode {
        link: LinkNode::new(url, &attributes.rel, &attributes.target, &attributes.title),
        is_unlinked: attributes.is_unlinked.unwrap_or(false),
    }))
}

/// `$isAutoLinkNode(node)`.
pub fn is_auto_link_node(node: &NodeRef) -> bool {
    node.get_latest().is::<AutoLinkNode>()
}

/// The `LinkNode` fields of a `LinkNode` or an `AutoLinkNode`.
fn link_of(node: &dyn LexicalNode) -> Option<&LinkNode> {
    node.downcast_ref::<LinkNode>()
        .or_else(|| node.downcast_ref::<AutoLinkNode>().map(|a| &a.link))
}

/// Changes the `LinkNode` fields of a link (`getWritable()`); does nothing
/// to any other node.
fn set_link(node: &NodeRef, f: impl FnOnce(&mut LinkNode)) {
    if is_link_node(node) {
        with_writable(node.get_key(), |n| {
            if n.is::<LinkNode>() {
                f(n.downcast_mut::<LinkNode>().expect("a link"));
            } else {
                f(&mut n.downcast_mut::<AutoLinkNode>().expect("an autolink").link);
            }
        });
    }
}

impl NodeRef {
    /// `LinkNode.getURL()`; `None` for any other node.
    pub fn get_url(&self) -> Option<String> {
        link_of(&*self.get_latest()).map(|l| l.url.clone())
    }

    /// `LinkNode.setURL(url)`. Does nothing to any other node.
    pub fn set_url(&self, url: &str) {
        set_link(self, |l| l.url = url.to_owned());
    }

    /// `LinkNode.getTarget()`; `None` when it is `null`, or for any other
    /// node.
    pub fn get_target(&self) -> Option<String> {
        link_of(&*self.get_latest()).and_then(|l| l.target.clone())
    }

    /// `LinkNode.setTarget(target)`. Does nothing to any other node.
    pub fn set_target(&self, target: Option<&str>) {
        set_link(self, |l| l.target = target.map(str::to_owned));
    }

    /// `LinkNode.getRel()`; `None` when it is `null`, or for any other node.
    pub fn get_rel(&self) -> Option<String> {
        link_of(&*self.get_latest()).and_then(|l| l.rel.clone())
    }

    /// `LinkNode.setRel(rel)`. Does nothing to any other node.
    pub fn set_rel(&self, rel: Option<&str>) {
        set_link(self, |l| l.rel = rel.map(str::to_owned));
    }

    /// `LinkNode.getTitle()`; `None` when it is `null`, or for any other
    /// node.
    pub fn get_title(&self) -> Option<String> {
        link_of(&*self.get_latest()).and_then(|l| l.title.clone())
    }

    /// `LinkNode.setTitle(title)`. Does nothing to any other node.
    pub fn set_title(&self, title: Option<&str>) {
        set_link(self, |l| l.title = title.map(str::to_owned));
    }

    /// `AutoLinkNode.getIsUnlinked()`; `None` for any other node.
    pub fn get_is_unlinked(&self) -> Option<bool> {
        self.get_latest()
            .downcast_ref::<AutoLinkNode>()
            .map(|a| a.is_unlinked)
    }

    /// `AutoLinkNode.setIsUnlinked(value)`. Does nothing to any other node.
    pub fn set_is_unlinked(&self, value: bool) {
        if is_auto_link_node(self) {
            with_writable(self.get_key(), |n| {
                n.downcast_mut::<AutoLinkNode>()
                    .expect("an autolink")
                    .is_unlinked = value;
            });
        }
    }

    /// `LinkNode.isEmailURI()`: whether the URL starts with `mailto:`;
    /// false for any other node.
    pub fn is_email_uri(&self) -> bool {
        self.get_url().is_some_and(|url| url.starts_with("mailto:"))
    }

    /// `LinkNode.isWebSiteURI()`: whether the URL starts with `https://` or
    /// `http://`; false for any other node.
    pub fn is_web_site_uri(&self) -> bool {
        self.get_url()
            .is_some_and(|url| url.starts_with("https://") || url.starts_with("http://"))
    }

    /// `LinkNode.shouldMergeAdjacentLink(otherLink)`: whether two links of
    /// the same type have the same URL and attributes. `AutoLinkNode`'s is
    /// always false, so adjacent autolinks never merge.
    pub fn should_merge_adjacent_link(&self, other_link: &NodeRef) -> bool {
        let this = self.get_latest();
        let other = other_link.get_latest();
        if this.is::<AutoLinkNode>() {
            return false;
        }
        match (link_of(&*this), link_of(&*other)) {
            (Some(a), Some(b)) => {
                this.get_type() == other.get_type()
                    && a.url == b.url
                    && a.target == b.target
                    && a.rel == b.rel
                    && a.title == b.title
            }
            _ => false,
        }
    }
}

/// `LinkNode.insertNewAfter(_, restoreSelection)`: a copy of the link
/// (`$copyNode`), after it.
fn insert_new_after(
    this: &NodeRef,
    restore_selection: bool,
) -> Result<Option<NodeRef>, LexicalError> {
    let link_node = copy_node(this);
    this.insert_after(&link_node, restore_selection)?;
    Ok(Some(link_node))
}

/// `LinkNode.extractWithChild(child, selection, destination)`: a range
/// selection that starts and ends in the link, and selects some text, takes
/// the link with it.
fn extract_with_child(
    this: &NodeRef,
    selection: Option<&SelectionRef>,
) -> Result<bool, LexicalError> {
    let Some(selection) = selection.filter(|s| s.is_range()) else {
        return Ok(false);
    };
    let range = selection.range().expect("a range selection");
    let anchor_node = range.anchor.get_node()?;
    let focus_node = range.focus.get_node()?;
    Ok((this == &anchor_node || this.is_parent_of(&anchor_node))
        && (this == &focus_node || this.is_parent_of(&focus_node))
        && !selection.get_text_content()?.is_empty())
}

impl LexicalNode for LinkNode {
    fn get_type(&self) -> &'static str {
        "link"
    }
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(link_dom::create_dom(self, config))
    }
    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        Some(link_dom::import_dom())
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }

    /// `LinkNode.isInline()`.
    fn is_inline(&self) -> bool {
        true
    }

    /// `LinkNode.canBeEmpty()`.
    fn can_be_empty(&self) -> bool {
        false
    }

    /// `LinkNode.canInsertTextBefore()`: text typed at the link's start
    /// goes before it.
    fn can_insert_text_before(&self) -> bool {
        false
    }

    /// `LinkNode.canInsertTextAfter()`: text typed at the link's end goes
    /// after it.
    fn can_insert_text_after(&self) -> bool {
        false
    }

    /// `LinkNode.insertNewAfter(selection, restoreSelection)`.
    fn insert_new_after(
        &self,
        this: &NodeRef,
        _: &SelectionRef,
        restore_selection: bool,
    ) -> Result<Option<NodeRef>, LexicalError> {
        insert_new_after(this, restore_selection)
    }

    /// `LinkNode.extractWithChild(child, selection, destination)`.
    fn extract_with_child(
        &self,
        this: &NodeRef,
        _: &NodeRef,
        selection: Option<&SelectionRef>,
        _: &str,
    ) -> Result<bool, LexicalError> {
        extract_with_child(this, selection)
    }

    /// `exportLinkNode` and `exportCompactLinkNode`.
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            export_compact_link_node(self)
        } else {
            export_link_node(self)
        }
    }

    /// `updateLinkNode`.
    fn update_from_json(&mut self, json: &Object) {
        update_link_node(self, json);
    }
}

/// `AutoLinkNode` overrides none of what `LinkNode` overrides here (not
/// `canInsertTextAfter` either, whatever its class comment says), so each
/// hook is `LinkNode`'s.
impl LexicalNode for AutoLinkNode {
    fn get_type(&self) -> &'static str {
        "autolink"
    }
    /// `AutoLinkNode.createDOM(config)`: a `<span>` once unlinked, else
    /// `LinkNode`'s `<a>`.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        if self.is_unlinked {
            return Ok(crate::lexical_dom::get_document().create_element("span"));
        }
        Ok(link_dom::create_dom(&self.link, config))
    }
    fn base(&self) -> &NodeBase {
        &self.link.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.link.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.link.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.link.element)
    }

    fn is_inline(&self) -> bool {
        true
    }

    fn can_be_empty(&self) -> bool {
        false
    }

    fn can_insert_text_before(&self) -> bool {
        false
    }

    fn can_insert_text_after(&self) -> bool {
        false
    }

    /// `LinkNode.insertNewAfter`, which `$copyNode` makes an `AutoLinkNode`
    /// with the same `isUnlinked`.
    fn insert_new_after(
        &self,
        this: &NodeRef,
        _: &SelectionRef,
        restore_selection: bool,
    ) -> Result<Option<NodeRef>, LexicalError> {
        insert_new_after(this, restore_selection)
    }

    fn extract_with_child(
        &self,
        this: &NodeRef,
        _: &NodeRef,
        selection: Option<&SelectionRef>,
        _: &str,
    ) -> Result<bool, LexicalError> {
        extract_with_child(this, selection)
    }

    /// `exportAutoLinkNode` and `exportCompactAutoLinkNode`.
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            export_compact_auto_link_node(self)
        } else {
            export_auto_link_node(self)
        }
    }

    /// `updateAutoLinkNode`.
    fn update_from_json(&mut self, json: &Object) {
        update_auto_link_node(self, json);
    }
}

/// `[next, next.getFlipped()]`: a selection point as carets both ways.
type CaretPair = [PointCaret; 2];

/// `$saveCaretPair(point)`.
fn save_caret_pair(point: &Point) -> Result<CaretPair, LexicalError> {
    let next = caret_from_point(point, CaretDirection::Next)?;
    let flipped = next.get_flipped()?;
    Ok([next, flipped])
}

/// `$restoreCaretPair(point, pair)`: the point at the first caret of the
/// pair whose node is still attached.
fn restore_caret_pair(
    selection: &SelectionRef,
    which: Which,
    pair: &CaretPair,
) -> Result<(), LexicalError> {
    for caret in pair {
        if caret.origin().is_attached() {
            let normalized = normalize_caret(caret);
            return set_point_from_caret(selection, which, &normalized);
        }
    }
    Ok(())
}

/// The merge boundary fix of `$linkNodeTransform`
/// (`$fixMergeBoundaryCaret`): a caret pair that points to the end of the
/// absorbing link, which would shift when the merging link's children are
/// appended to it, moves to the merging link's start.
fn fix_merge_boundary_caret(
    pair: CaretPair,
    absorbing_link: &NodeRef,
    merging_link: &NodeRef,
) -> Result<CaretPair, LexicalError> {
    let is_affected =
        |caret: &PointCaret| caret.is_sibling_caret() && caret.origin() == absorbing_link;
    if !is_affected(&pair[0]) && !is_affected(&pair[1]) {
        return Ok(pair);
    }
    // Resolve the merge boundary from the merging link's start, since
    // those children survive the move and represent the boundary position.
    let fixed = normalize_caret(
        &get_child_caret(merging_link, CaretDirection::Next).expect("a link is an element"),
    );
    let flipped = fixed.get_flipped()?;
    Ok([fixed, flipped])
}

/// `$linkNodeTransform(link)`: lifts block children out of a link, splitting
/// its ancestors as needed to keep the document valid, and merges the link
/// with an adjacent link that `shouldMergeAdjacentLink` matches, keeping the
/// selection where it was. `registerLink` registers it for `LinkNode`.
///
/// `NodeStructureExtension`, which does both itself where it enforces node
/// structure, isn't ported, so this always does them.
pub fn link_node_transform(link: &NodeRef) -> Result<(), LexicalError> {
    let selection = get_selection().filter(SelectionRef::is_range);
    let mut pairs = None;
    if let Some(selection) = &selection {
        pairs = Some((
            save_caret_pair(&selection.point(Which::Anchor).expect("a range"))?,
            save_caret_pair(&selection.point(Which::Focus).expect("a range"))?,
        ));
    }
    let restore_selection = |pairs: &Option<(CaretPair, CaretPair)>| -> Result<(), LexicalError> {
        if let (Some(selection), Some((anchor_pair, focus_pair))) = (&selection, pairs) {
            restore_caret_pair(selection, Which::Anchor, anchor_pair)?;
            restore_caret_pair(selection, Which::Focus, focus_pair)?;
            normalize_selection(selection)?;
        }
        Ok(())
    };

    let mut transformed = false;
    let children = get_child_caret(link, CaretDirection::Next).expect("a link is an element");
    for caret in children.iter() {
        let node = caret.origin().clone();
        if node.is_element() && !node.is_inline() {
            let block_children = node.get_children();
            if !block_children.is_empty() {
                let inner_link = copy_node(link);
                inner_link.append(&block_children)?;
                node.append(std::slice::from_ref(&inner_link))?;
                transformed = true;
            }
            insert_node_to_nearest_root_at_caret(
                &node,
                &rewind_sibling_caret(&caret)?,
                &SplitAtPointCaretNextOptions {
                    should_split: |_, _| false,
                    ..SplitAtPointCaretNextOptions::default()
                },
            )?;
        }
    }

    let fix_pairs = |pairs: Option<(CaretPair, CaretPair)>,
                     absorbing: &NodeRef,
                     merging: &NodeRef|
     -> Result<Option<(CaretPair, CaretPair)>, LexicalError> {
        pairs
            .map(|(anchor, focus)| {
                Ok((
                    fix_merge_boundary_caret(anchor, absorbing, merging)?,
                    fix_merge_boundary_caret(focus, absorbing, merging)?,
                ))
            })
            .transpose()
    };
    if link.is_attached() {
        if let Some(prev_sibling) = link
            .get_previous_sibling()
            .filter(|p| is_link_node(p) && p.should_merge_adjacent_link(link))
        {
            pairs = fix_pairs(pairs, &prev_sibling, link)?;
            prev_sibling.append(&link.get_children())?;
            link.remove(false)?;
            return restore_selection(&pairs);
        }
        if let Some(next_sibling) = link
            .get_next_sibling()
            .filter(|n| is_link_node(n) && link.should_merge_adjacent_link(n))
        {
            pairs = fix_pairs(pairs, link, &next_sibling)?;
            link.append(&next_sibling.get_children())?;
            next_sibling.remove(false)?;
            transformed = true;
        }
    }
    if !transformed {
        return Ok(());
    }
    if !link.can_be_empty() && link.is_empty() {
        let parent = link.get_parent();
        link.remove(false)?;
        if let Some(parent) = parent.filter(NodeRef::is_empty) {
            parent.remove(false)?;
        }
    }
    restore_selection(&pairs)
}

/// `TOGGLE_LINK_COMMAND`'s payload when it isn't `null` (`None`, which
/// removes the links): a URL, or a URL with attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToggleLinkPayload {
    Url(String),
    Attributes {
        url: String,
        attributes: LinkAttributes,
    },
}

// Like the core's commands (`commands.rs`), a constant, with an id counting
// down from where the list commands stop.

/// `TOGGLE_LINK_COMMAND`: dispatched to link the selection, change the
/// links in it, or with `None` remove them.
pub const TOGGLE_LINK_COMMAND: LexicalCommand<Option<ToggleLinkPayload>> =
    LexicalCommand::constant(u64::MAX - 25, "TOGGLE_LINK_COMMAND");

/// `$getPointNode(point, offset)`: the child at an element point's offset
/// plus `offset`, if any.
fn get_point_node(point: &Point, offset: isize) -> Result<Option<NodeRef>, LexicalError> {
    if point.point_type != PointType::Element {
        return Ok(None);
    }
    let node = point.get_node()?;
    invariant!(
        node.is_element(),
        "$getPointNode: element point is not an ElementNode"
    );
    Ok(point
        .offset
        .checked_add_signed(offset)
        .and_then(|index| node.get_child_at_index(index)))
}

/// `$withSelectedNodes($fn)`: runs `f`, keeping the logical start and end of
/// a range selection whose element points `f` may reparent.
fn with_selected_nodes<T>(f: impl FnOnce() -> Result<T, LexicalError>) -> Result<T, LexicalError> {
    let Some(initial_selection) = get_selection().filter(SelectionRef::is_range) else {
        return f();
    };
    normalize_selection(&initial_selection)?;
    let normalized = &initial_selection;
    let is_backwards = normalized.is_backward()?;
    let anchor_node = get_point_node(
        &normalized.point(Which::Anchor).expect("a range"),
        if is_backwards { -1 } else { 0 },
    )?;
    let focus_node = get_point_node(
        &normalized.point(Which::Focus).expect("a range"),
        if is_backwards { 0 } else { -1 },
    )?;
    let rval = f()?;
    if (anchor_node.is_some() || focus_node.is_some())
        && let Some(updated_selection) = get_selection().filter(SelectionRef::is_range)
    {
        let final_selection = updated_selection.clone_selection();
        for (which, node, after) in [
            (Which::Anchor, &anchor_node, is_backwards),
            (Which::Focus, &focus_node, !is_backwards),
        ] {
            if let Some(node) = node
                && let Some(parent) = node.get_parent()
            {
                let index = node
                    .get_index_within_parent()
                    .expect("a child of its parent");
                final_selection.point_set(
                    which,
                    parent.get_key(),
                    index + usize::from(after),
                    PointType::Element,
                )?;
            }
        }
        normalize_selection(&final_selection)?;
        set_selection(Some(final_selection));
    }
    Ok(rval)
}

/// `$splitLinkAtSelection(parentLink, extractedNodes)`: takes the selected
/// children out of a link, at its start, its end or its middle (splitting
/// it in two).
fn split_link_at_selection(
    parent_link: &NodeRef,
    extracted_nodes: &[NodeRef],
) -> Result<(), LexicalError> {
    let extracted_keys: HashSet<NodeKey> = extracted_nodes
        .iter()
        .filter(|n| parent_link.is_parent_of(n))
        .map(|n| n.get_key().to_owned())
        .collect();

    let all_children = parent_link.get_children();
    // Check if a child is an extracted node OR contains an extracted node
    // This handles nested structures like LinkNode > HeadingNode > TextNode
    let is_extracted_child = |child: &NodeRef| {
        extracted_keys.contains(child.get_key())
            || (child.is_element()
                && extracted_nodes
                    .iter()
                    .any(|n| parent_link.is_parent_of(n) && child.is_parent_of(n)))
    };

    let extracted_children: Vec<NodeRef> = all_children
        .iter()
        .filter(|c| is_extracted_child(c))
        .cloned()
        .collect();

    if extracted_children.is_empty() && !all_children.is_empty() {
        // The selection only touches the link at a boundary (e.g. it ends at
        // offset 0 of the link's first text node, or starts at the end of its
        // last one), so extract() returned the LinkNode itself but none of its
        // content. There is nothing to unlink.
        return Ok(());
    }

    if extracted_children.len() == all_children.len() {
        for child in &all_children {
            parent_link.insert_before(child, true)?;
        }
        return parent_link.remove(false);
    }

    let first_extracted_index = all_children
        .iter()
        .position(&is_extracted_child)
        .expect("an extracted child");
    let last_extracted_index = all_children
        .iter()
        .rposition(&is_extracted_child)
        .expect("an extracted child");

    let is_at_start = first_extracted_index == 0;
    let is_at_end = last_extracted_index == all_children.len() - 1;

    if is_at_start {
        for child in &extracted_children {
            parent_link.insert_before(child, true)?;
        }
    } else if is_at_end {
        for child in extracted_children.iter().rev() {
            parent_link.insert_after(child, true)?;
        }
    } else {
        for child in extracted_children.iter().rev() {
            parent_link.insert_after(child, true)?;
        }

        let trailing_children = &all_children[last_extracted_index + 1..];
        if !trailing_children.is_empty() {
            let new_link = copy_node(parent_link);
            extracted_children
                .last()
                .expect("an extracted child")
                .insert_after(&new_link, true)?;
            for child in trailing_children {
                new_link.append(std::slice::from_ref(child))?;
            }
        }
    }
    Ok(())
}

/// A `LinkNode` that is not an `AutoLinkNode`, which is what removing or
/// updating a link looks for.
fn is_non_auto_link(node: &NodeRef) -> bool {
    !is_auto_link_node(node) && is_link_node(node)
}

/// `$toggleLink(url, attributes)`: links the selection to `url`, or updates
/// the links it is in; with `url` `None` (JS `null`), removes the links
/// around it and keeps their children. `rel` defaults to `noreferrer`;
/// another attribute that is `None` (`undefined`) leaves an existing link's
/// as it is.
///
/// JS also takes `{url, ...attributes}` as the first argument, whose
/// attributes the second argument's override; here a caller passes the URL
/// and the merged attributes ([`LinkAttributes::merging`]).
pub fn toggle_link(url: Option<&str>, attributes: &LinkAttributes) -> Result<(), LexicalError> {
    let target = attributes.target.clone();
    let title = attributes.title.clone();
    let rel = match &attributes.rel {
        None => Some(Some("noreferrer".to_owned())),
        rel => rel.clone(),
    };
    let new_link_attributes = LinkAttributes {
        rel: rel.clone(),
        target: target.clone(),
        title: title.clone(),
    };
    let update_attributes = |link_node: &NodeRef| {
        if let Some(target) = &target {
            link_node.set_target(target.as_deref());
        }
        if let Some(rel) = &rel {
            link_node.set_rel(rel.as_deref());
        }
        if let Some(title) = &title {
            link_node.set_title(title.as_deref());
        }
    };
    let Some(selection) = get_selection() else {
        return Ok(());
    };

    if selection.is_node() {
        let nodes = selection.get_nodes()?;
        if nodes.is_empty() {
            return Ok(());
        }

        // Handle all selected nodes
        for node in &nodes {
            match url {
                None => {
                    // Remove link
                    if let Some(link_parent) = find_matching_parent(node, is_non_auto_link) {
                        link_parent.insert_before(node, true)?;
                        if link_parent.get_children().is_empty() {
                            link_parent.remove(false)?;
                        }
                    }
                }
                Some(url) => {
                    // Add/Update link
                    if let Some(existing_link) = find_matching_parent(node, is_non_auto_link) {
                        existing_link.set_url(url);
                        update_attributes(&existing_link);
                    } else {
                        let link_node = create_link_node(url, &new_link_attributes);
                        node.insert_before(&link_node, true)?;
                        link_node.append(std::slice::from_ref(node))?;
                    }
                }
            }
        }
        return Ok(());
    }

    let is_collapsed = selection.range().is_some_and(|r| r.is_collapsed());
    if is_collapsed
        && url.is_none()
        && let Some(node) = selection.get_nodes()?.first()
    {
        if let Some(parent_link) = find_matching_parent(node, is_non_auto_link) {
            parent_link.get_parent_or_throw()?.splice(
                parent_link
                    .get_index_within_parent()
                    .expect("a child of its parent"),
                0,
                &parent_link.get_children(),
            )?;
            parent_link.remove(false)?;
        }
        return Ok(());
    }

    // Handle RangeSelection
    let nodes = selection.extract()?;

    let Some(url) = url else {
        let mut processed_links = HashSet::new();
        for node in &nodes {
            if let Some(parent_link) = find_matching_parent(node, is_non_auto_link) {
                let link_key = parent_link.get_key().to_owned();
                if processed_links.contains(&link_key) {
                    continue;
                }
                split_link_at_selection(&parent_link, &nodes)?;
                processed_links.insert(link_key);
            }
        }
        return Ok(());
    };
    let mut updated_nodes = HashSet::new();
    let mut update_link_node = |link_node: &NodeRef| {
        if !updated_nodes.insert(link_node.get_key().to_owned()) {
            return;
        }
        link_node.set_url(url);
        update_attributes(link_node);
    };
    // Add or merge LinkNodes
    if nodes.len() == 1 {
        // if the first node is a LinkNode or if its
        // parent is a LinkNode, we update the URL, target and rel.
        if let Some(link_node) = find_matching_parent(&nodes[0], is_link_node) {
            update_link_node(&link_node);
            return Ok(());
        }
    }

    with_selected_nodes(|| {
        let mut link_node: Option<NodeRef> = None;
        for node in &nodes {
            if !node.is_attached() {
                continue;
            }
            if let Some(parent_link_node) = find_matching_parent(node, is_link_node) {
                update_link_node(&parent_link_node);
                continue;
            }
            if node.is_element() {
                if !node.is_inline() {
                    // Ignore block nodes, if there are any children we will see them
                    // later and wrap in a new LinkNode
                    continue;
                }
                if is_link_node(node) {
                    // If it's not an autolink node and we don't already have a LinkNode
                    // in this block then we can update it and re-use it
                    let reusable = match &link_node {
                        None => true,
                        Some(link_node) => !link_node.get_parent_or_throw()?.is_parent_of(node),
                    };
                    if !is_auto_link_node(node) && reusable {
                        update_link_node(node);
                        link_node = Some(node.clone());
                        continue;
                    }
                    // Unwrap LinkNode, we already have one or it's an AutoLinkNode
                    for child in node.get_children() {
                        node.insert_before(&child, true)?;
                    }
                    node.remove(false)?;
                    continue;
                }
            }
            if let Some(prev_link_node) = node.get_previous_sibling()
                && is_link_node(&prev_link_node)
                && link_node.as_ref() == Some(&prev_link_node)
            {
                prev_link_node.append(std::slice::from_ref(node))?;
                continue;
            }
            let new_link_node = create_link_node(url, &new_link_attributes);
            node.insert_after(&new_link_node, true)?;
            new_link_node.append(std::slice::from_ref(node))?;
            link_node = Some(new_link_node);
        }
        Ok(())
    })
}

/// `PHONE_NUMBER_REGEX`, `/^\+?[0-9\s()-]{5,}$/`.
fn is_phone_number(url: &str) -> bool {
    let rest = url.strip_prefix('+').unwrap_or(url);
    rest.chars().count() >= 5
        && rest
            .chars()
            .all(|c| c.is_ascii_digit() || is_js_whitespace(c) || matches!(c, '(' | ')' | '-'))
}

/// JS's `\s`.
fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{D}'
            | ' '
            | '\u{A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

/// Whether `/^[a-z][a-z0-9+.-]*:/i` matches: the URL starts with a scheme.
fn has_scheme(url: &str) -> bool {
    let mut chars = url.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    for c in chars {
        if c == ':' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')) {
            return false;
        }
    }
    false
}

/// `formatUrl(url)`: the URL with the protocol it is missing. One with a
/// scheme or a relative path (`/`, `#` or `.` first) is kept; an email
/// address gets `mailto:`, a phone number `tel:`, and anything else
/// `https://`.
pub fn format_url(url: &str) -> String {
    // Check if URL already has a protocol
    if has_scheme(url) {
        // URL already has a protocol, leave it as is
        url.to_owned()
    }
    // Check if it's a relative path (starting with '/', '.', or '#')
    else if url.starts_with(['/', '#', '.']) {
        // Relative path, leave it as is
        url.to_owned()
    }
    // Check for email address
    else if url.contains('@') {
        format!("mailto:{url}")
    }
    // Check for phone number
    else if is_phone_number(url) {
        format!("tel:{url}")
    }
    // For everything else, return with https:// prefix
    else {
        format!("https://{url}")
    }
}

#[cfg(test)]
mod tests {
    use super::format_url;

    /// `formatUrl` as Node gives it for these inputs.
    #[test]
    fn format_url_matches_js() {
        for (url, formatted) in [
            ("https://lexical.dev", "https://lexical.dev"),
            ("mailto:a@b.c", "mailto:a@b.c"),
            ("javascript:alert(1)", "javascript:alert(1)"),
            ("/path", "/path"),
            ("#anchor", "#anchor"),
            ("./rel", "./rel"),
            ("a@b.c", "mailto:a@b.c"),
            ("+1 (555) 123-4567", "tel:+1 (555) 123-4567"),
            ("12345", "tel:12345"),
            ("1234", "https://1234"),
            ("+1234", "https://+1234"),
            ("1\u{3000}2\u{A0}3 4", "tel:1\u{3000}2\u{A0}3 4"),
            ("123456\n", "tel:123456\n"),
            ("lexical.dev", "https://lexical.dev"),
            ("1a:b", "https://1a:b"),
            ("", "https://"),
        ] {
            assert_eq!(format_url(url), formatted, "{url:?}");
        }
    }
}

// The DOM side of `LinkNode` and `AutoLinkNode` (the `html` feature).

#[cfg(feature = "html")]
pub(super) mod link_dom {
    use super::{LinkNode, create_link_node, format_url, is_js_whitespace};
    use crate::dom::DomNode;
    use crate::lexical_dom::{
        DomConversionMap, DomConversionOutput, add_theme_class_names, get_document, importer,
    };
    use crate::{EditorConfig, LexicalError};

    /// `SUPPORTED_URL_PROTOCOLS`.
    const SUPPORTED_URL_PROTOCOLS: [&str; 5] = ["http:", "https:", "mailto:", "sms:", "tel:"];

    /// The scheme `/^([a-z][a-z0-9+.-]*):/i` matches at the start.
    fn leading_scheme(url: &str) -> Option<&str> {
        let end = url.find(':')?;
        let scheme = &url[..end];
        let mut chars = scheme.chars();
        (chars.next()?.is_ascii_alphabetic()
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')))
        .then_some(scheme)
    }

    /// The `protocol` of `new URL(url)`, or `None` where it throws. Like
    /// Swift's (`swift/Sources/LexicalLink/LinkNode.swift`), this follows
    /// the WHATWG URL parser as far as an absolute URL's scheme and host
    /// decide whether it parses, without IDNA or IPv4 parsing.
    fn whatwg_url_protocol(url: &str) -> Option<String> {
        let trimmed = url.trim_matches(|c: char| c <= ' ');
        let input: String = trimmed
            .chars()
            .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
            .collect();
        let scheme = leading_scheme(&input)?.to_ascii_lowercase();
        let is_special = ["http", "https", "ws", "wss", "ftp", "file"].contains(&scheme.as_str());
        let mut rest = &input[scheme.len() + 1..];
        if is_special {
            if scheme == "file" {
                return Some("file:".to_owned());
            }
            rest = rest.trim_start_matches(['/', '\\']);
        } else if let Some(r) = rest.strip_prefix("//") {
            rest = r;
        } else {
            return Some(format!("{scheme}:"));
        }
        let authority_end = rest
            .find(|c| matches!(c, '/' | '?' | '#') || (is_special && c == '\\'))
            .unwrap_or(rest.len());
        let mut authority = &rest[..authority_end];
        if let Some(at) = authority.rfind('@') {
            authority = &authority[at + 1..];
        }
        let mut host = authority;
        let mut port = None;
        if host.starts_with('[') {
            let close = host.find(']')?;
            let after = &host[close + 1..];
            if let Some(p) = after.strip_prefix(':') {
                port = Some(p);
            } else if !after.is_empty() {
                return None;
            }
            host = &host[..=close];
        } else if let Some(colon) = host.find(':') {
            port = Some(&host[colon + 1..]);
            host = &host[..colon];
        }
        if let Some(port) = port.filter(|p| !p.is_empty())
            && (!port.bytes().all(|b| b.is_ascii_digit())
                || port.trim_start_matches('0').len() > 5
                || port.trim_start_matches('0').parse::<u32>().unwrap_or(0) > 65535)
        {
            return None;
        }
        if host.is_empty() {
            return if is_special || port.is_some() {
                None
            } else {
                Some(format!("{scheme}:"))
            };
        }
        if !host.starts_with('[') {
            let forbidden = |c: char| {
                matches!(
                    c,
                    '\0' | '\t'
                        | '\n'
                        | '\r'
                        | ' '
                        | '#'
                        | '/'
                        | ':'
                        | '<'
                        | '>'
                        | '?'
                        | '@'
                        | '['
                        | '\\'
                        | ']'
                        | '^'
                        | '|'
                ) || (is_special && (c <= '\u{1f}' || c == '%' || c == '\u{7f}'))
            };
            if host.chars().any(forbidden) {
                return None;
            }
        }
        Some(format!("{scheme}:"))
    }

    /// `LinkNode.sanitizeUrl(url)`: `about:blank` for a URL whose scheme
    /// isn't supported, else the URL with `formatUrl`'s protocol.
    pub fn sanitize_url(url: &str) -> String {
        let raw_url = url;
        let url = format_url(url);
        match whatwg_url_protocol(&format_url(&url)) {
            Some(protocol) => {
                if !SUPPORTED_URL_PROTOCOLS.contains(&protocol.as_str()) {
                    return "about:blank".to_owned();
                }
            }
            None => {
                // `new URL()` threw: an explicit scheme in the input, once
                // C0 controls, DEL and whitespace are dropped, that isn't
                // supported is neutralized.
                let normalized: String = raw_url
                    .chars()
                    .filter(|&c| !(c <= '\u{1f}' || c == '\u{7f}' || is_js_whitespace(c)))
                    .collect();
                if let Some(scheme) = leading_scheme(&normalized) {
                    let protocol = format!("{}:", scheme.to_ascii_lowercase());
                    if !SUPPORTED_URL_PROTOCOLS.contains(&protocol.as_str()) {
                        return "about:blank".to_owned();
                    }
                }
            }
        }
        url
    }

    /// `LinkNode.createDOM(config)`: an `<a>` with the sanitized URL, the
    /// target, rel and title it has (`updateLinkDOM(null, ...)`), and the
    /// theme's `link` class names.
    pub(crate) fn create_dom(link: &LinkNode, config: &EditorConfig) -> DomNode {
        let element = get_document().create_element("a");
        element.set_attribute("href", &sanitize_url(&link.url));
        for (attr, value) in [
            ("target", &link.target),
            ("rel", &link.rel),
            ("title", &link.title),
        ] {
            match value.as_deref().filter(|v| !v.is_empty()) {
                Some(value) => element.set_attribute(attr, value),
                None => element.remove_attribute(attr),
            }
        }
        add_theme_class_names(&element, &config.theme, "link");
        element
    }

    /// `LinkNode`'s `importDOM`: `<a>`.
    pub(crate) fn import_dom() -> DomConversionMap {
        vec![("a".to_owned(), importer(1, convert_anchor_element))]
    }

    /// `$convertAnchorElement(domNode)`: a link for an `<a>` with content.
    fn convert_anchor_element(
        dom_node: &DomNode,
    ) -> Result<Option<DomConversionOutput>, LexicalError> {
        let mut node = None;
        if dom_node.is_element()
            && dom_node.local_name() == "a"
            && (!dom_node.text_content().is_empty() || !dom_node.children().is_empty())
        {
            node = Some(create_link_node(
                &dom_node.get_attribute("href").unwrap_or_default(),
                &super::LinkAttributes {
                    rel: Some(dom_node.get_attribute("rel")),
                    target: Some(dom_node.get_attribute("target")),
                    title: Some(dom_node.get_attribute("title")),
                },
            ));
        }
        Ok(Some(DomConversionOutput::node(node)))
    }
}

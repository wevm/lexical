//! `LexicalListNode.ts`: `ListNode`, `$createListNode` and `$isListNode`.

use super::format_list::{
    merge_nested_lists_at_seam, merge_next_sibling_list_if_same_type,
    update_children_list_item_value,
};
use super::generated_json::{export_compact_list_node, export_list_node, update_list_node};
use super::list_item_node::{create_list_item_node, is_list_item_node};
use crate::LexicalError;
use crate::json::Object;
use crate::lexical_utils::{set_node_key, with_writable};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::{self, ElementData};

/// `ListType`: `'number'`, `'bullet'` or `'check'`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListType {
    #[default]
    Number,
    Bullet,
    Check,
}

impl ListType {
    pub fn as_str(self) -> &'static str {
        match self {
            ListType::Number => "number",
            ListType::Bullet => "bullet",
            ListType::Check => "check",
        }
    }

    /// The list type a string names (`enumValue(['number', 'bullet',
    /// 'check'])`).
    pub fn parse(list_type: &str) -> Option<Self> {
        Some(match list_type {
            "number" => ListType::Number,
            "bullet" => ListType::Bullet,
            "check" => ListType::Check,
            _ => return None,
        })
    }

    /// `ListNodeTagType`: the tag a list of this type has, `ol` for
    /// `number` and `ul` otherwise (`setListType`).
    pub fn tag(self) -> &'static str {
        match self {
            ListType::Number => "ol",
            _ => "ul",
        }
    }
}

/// `ListNode`: an ordered (`number`), unordered (`bullet`) or check list,
/// whose children are [`ListItemNode`](super::ListItemNode)s.
///
/// JS stores the tag (`__tag`) beside the list type, and sets both together
/// wherever either changes; here the tag is read off the list type.
#[derive(Clone, Debug)]
pub struct ListNode {
    base: NodeBase,
    pub(super) element: ElementData,
    pub(super) list_type: ListType,
    pub(super) start: f64,
}

impl Default for ListNode {
    /// `new ListNode()`: a `number` list starting at 1.
    fn default() -> Self {
        ListNode {
            base: NodeBase::default(),
            element: ElementData::default(),
            list_type: ListType::Number,
            start: 1.0,
        }
    }
}

/// `$createListNode(listType = 'number', start = 1)`.
pub fn create_list_node(list_type: ListType, start: f64) -> NodeRef {
    set_node_key(Box::new(ListNode {
        list_type,
        start,
        ..ListNode::default()
    }))
}

/// `$isListNode(node)`.
pub fn is_list_node(node: &NodeRef) -> bool {
    node.get_latest().is::<ListNode>()
}

impl NodeRef {
    /// `ListNode.getListType()`; `None` for any other node.
    pub fn get_list_type(&self) -> Option<ListType> {
        self.get_latest()
            .downcast_ref::<ListNode>()
            .map(|l| l.list_type)
    }

    /// `ListNode.setListType(type)`. Does nothing to any other node.
    pub fn set_list_type(&self, list_type: ListType) {
        if is_list_node(self) {
            with_writable(self.get_key(), |n| {
                n.downcast_mut::<ListNode>().expect("a list").list_type = list_type;
            });
        }
    }

    /// `ListNode.getTag()`; `None` for any other node.
    pub fn get_list_tag(&self) -> Option<&'static str> {
        self.get_list_type().map(ListType::tag)
    }

    /// `ListNode.getStart()`; `None` for any other node.
    pub fn get_list_start(&self) -> Option<f64> {
        self.get_latest()
            .downcast_ref::<ListNode>()
            .map(|l| l.start)
    }

    /// `ListNode.setStart(start)`. Does nothing to any other node.
    pub fn set_list_start(&self, start: f64) {
        if is_list_node(self) {
            with_writable(self.get_key(), |n| {
                n.downcast_mut::<ListNode>().expect("a list").start = start;
            });
        }
    }
}

impl ListNode {
    pub fn list_type(&self) -> ListType {
        self.list_type
    }

    pub fn start(&self) -> f64 {
        self.start
    }
}

impl LexicalNode for ListNode {
    /// The `$config()` of `ListNode`: `children: [ListItemNode]` and
    /// `mergeAdjacent: {by: ['listType']}`, what
    /// `mergeNextSiblingListIfSameType` compares (`start` is not).
    fn static_node_config_chain(&self) -> Vec<crate::structure::NodeConfigLink> {
        use crate::structure::{
            MergeAdjacentDeclaration, MergeAdjacentEntry, NodeChildEntry, NodeChildrenConfig,
            NodeClassRef, NodeConfigLink, StaticNodeConfig,
        };
        vec![NodeConfigLink::new(
            "list",
            StaticNodeConfig {
                children: Some(NodeChildrenConfig::List(vec![NodeChildEntry::Class(
                    NodeClassRef::of::<super::ListItemNode>(),
                )])),
                merge_adjacent: Some(MergeAdjacentDeclaration::By(vec![
                    MergeAdjacentEntry::Field("listType"),
                ])),
                ..StaticNodeConfig::default()
            },
        )]
    }
    /// `ListNode.updateDOM(prevNode)`: a list of another tag or type is
    /// made again (the tag follows the type here).
    fn update_dom(&self, prev_node: &dyn LexicalNode) -> bool {
        prev_node
            .downcast_ref::<ListNode>()
            .is_some_and(|prev| prev.list_type != self.list_type)
    }
    fn get_type(&self) -> &'static str {
        "list"
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

    /// `ListNode.canBeEmpty()`.
    fn can_be_empty(&self) -> bool {
        false
    }

    /// `ListNode.extractWithChild(child)`: a list goes with a list item
    /// that is copied.
    fn extract_with_child(
        &self,
        _: &NodeRef,
        child: &NodeRef,
        _: Option<&crate::lexical_selection::SelectionRef>,
        _: &str,
    ) -> Result<bool, LexicalError> {
        Ok(is_list_item_node(child))
    }

    /// `ListNode.canIndent()`.
    fn can_indent(&self) -> bool {
        false
    }

    /// `exportListNode` and `exportCompactListNode`.
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            export_compact_list_node(self)
        } else {
            export_list_node(self)
        }
    }

    /// `updateListNode`.
    fn update_from_json(&mut self, json: &Object) {
        update_list_node(self, json);
    }

    /// `ListNode.splice(start, deleteCount, nodesToInsert)`: what isn't a
    /// list item is wrapped in one (`createListItemNode()`), a block by its
    /// children.
    fn splice(
        &self,
        this: &NodeRef,
        start: usize,
        delete_count: usize,
        nodes_to_insert: &[NodeRef],
    ) -> Result<(), LexicalError> {
        let mut list_item_nodes_to_insert = Vec::with_capacity(nodes_to_insert.len());
        for node in nodes_to_insert {
            if is_list_item_node(node) {
                list_item_nodes_to_insert.push(node.clone());
                continue;
            }
            let list_item = create_list_item_node(None);
            if node.is_element() && !(is_list_node(node) || node.is_inline()) {
                // A block can't stay a block inside a list item, so it is
                // unwrapped into its own children.
                list_item.append(&node.get_children())?;
            } else {
                list_item.append(std::slice::from_ref(node))?;
            }
            list_item_nodes_to_insert.push(list_item);
        }
        element::splice(this, start, delete_count, &list_item_nodes_to_insert)
    }

    /// The `$transform` of `ListNode.$config()`: merges the next list of the
    /// same type into this one, then renumbers the items. Where
    /// `NodeStructureExtension` enforces declared structure, it merges
    /// adjacent lists itself (by `mergeAdjacent`, joining nested lists at the
    /// seam as `mergeLists` does), the same way whichever list is dirty.
    fn transform(&self, this: &NodeRef) -> Result<(), LexicalError> {
        let structure_merges = this
            .get_next_sibling()
            .is_some_and(|next| crate::structure::structure_merges(this, &next));
        if !structure_merges {
            merge_next_sibling_list_if_same_type(this)?;
        }
        update_children_list_item_value(this);
        Ok(())
    }

    /// `ListNode.INTERNAL_$afterStructureMerge(last, first)`: joins nested
    /// lists at the seam, as `mergeLists` does, but only items whose only
    /// child is the nested list, so that nothing else in them is removed or
    /// moved.
    fn internal_after_structure_merge(
        &self,
        _: &NodeRef,
        last: &NodeRef,
        first: &NodeRef,
    ) -> Result<bool, LexicalError> {
        merge_nested_lists_at_seam(Some(last.clone()), Some(first.clone()), true)?;
        Ok(true)
    }

    /// `ListNode.INTERNAL_$afterStructureSplit(original, preceding)`: an
    /// ordered list part continues the numbering of the items that precede
    /// it, so it starts at the next number.
    fn internal_after_structure_split(
        &self,
        this: &NodeRef,
        original: &NodeRef,
        preceding: &[NodeRef],
    ) -> Result<(), LexicalError> {
        if self.list_type == ListType::Number {
            let mut start = original.get_list_start().unwrap_or(1.0);
            for node in preceding {
                // As updateChildrenListItemValue counts: an item that only
                // wraps a nested list has no number of its own.
                if is_list_item_node(node)
                    && !node.get_first_child().is_some_and(|c| is_list_node(&c))
                {
                    start += 1.0;
                }
            }
            this.set_list_start(start);
        }
        Ok(())
    }
}

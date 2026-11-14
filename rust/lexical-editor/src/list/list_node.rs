//! `LexicalListNode.ts`: `ListNode`, `$createListNode` and `$isListNode`.

use super::format_list::{merge_next_sibling_list_if_same_type, update_children_list_item_value};
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
    fn get_type(&self) -> &'static str {
        "list"
    }
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        list_dom::create_dom(self, this, config)
    }
    #[cfg(feature = "html")]
    fn export_dom(
        &self,
        this: &NodeRef,
        editor: &crate::Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, LexicalError> {
        list_dom::export_dom(self, this, editor)
    }
    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        Some(list_dom::import_dom())
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
    /// same type into this one, then renumbers the items.
    /// (`NodeStructureExtension`, which merges lists itself where it
    /// enforces structure, isn't ported.)
    fn transform(&self, this: &NodeRef) -> Result<(), LexicalError> {
        merge_next_sibling_list_if_same_type(this)?;
        update_children_list_item_value(this);
        Ok(())
    }
}

// The DOM side of `ListNode` (the `html` feature).

#[cfg(feature = "html")]
pub(super) mod list_dom {
    use super::super::list_item_node::{create_list_item_node, is_list_item_node};
    use super::super::utils::get_list_depth;
    use super::{ListNode, ListType, create_list_node, is_list_node};
    use crate::dom::DomNode;
    use crate::json::{Object, Value};
    use crate::lexical_dom::{
        DomConversionMap, DomConversionNode, DomConversionOutput, DomExportOutput, get_document,
        importer, normalize_class_names, set_direction_from_dom,
    };
    use crate::node::NodeRef;
    use crate::{EditorConfig, LexicalError};

    /// `ListNode.createDOM(config)`: an `<ol>` or `<ul>`, with `start`
    /// when it isn't 1 and the theme's class names.
    pub(crate) fn create_dom(
        node: &ListNode,
        this: &NodeRef,
        config: &EditorConfig,
    ) -> Result<DomNode, LexicalError> {
        let dom = get_document().create_element(node.list_type.tag());
        if node.start != 1.0 {
            dom.set_attribute("start", &crate::json::number_to_string(node.start));
        }
        set_list_theme_class_names(&dom, &config.theme, node, this)?;
        Ok(dom)
    }

    /// `ListNode.exportDOM(editor)`: `createDOM`'s element, with
    /// `__lexicalListType` for a check list and the direction.
    pub(crate) fn export_dom(
        node: &ListNode,
        this: &NodeRef,
        editor: &crate::Editor,
    ) -> Result<DomExportOutput, LexicalError> {
        let element = create_dom(node, this, editor.config())?;
        if node.start != 1.0 {
            element.set_attribute("start", &crate::json::number_to_string(node.start));
        }
        if node.list_type == ListType::Check {
            element.set_attribute("__lexicalListType", "check");
        }
        if let Some(direction) = node.element.get_direction() {
            element.set_dir(direction.as_str());
        }
        Ok(DomExportOutput::element(element))
    }

    /// The strings `normalizeClassNames(value)` takes from a theme value:
    /// none unless it is a string.
    fn class_names_of(value: Option<&Value>) -> Vec<String> {
        value
            .and_then(Value::as_str)
            .map(|v| normalize_class_names(&[v]))
            .unwrap_or_default()
    }

    /// `$setListThemeClassNames(dom, editorThemeClasses, node)`.
    fn set_list_theme_class_names(
        dom: &DomNode,
        theme: &Object,
        node: &ListNode,
        this: &NodeRef,
    ) -> Result<(), LexicalError> {
        let mut classes_to_add: Vec<String> = Vec::new();
        let mut classes_to_remove: Vec<String> = Vec::new();
        if let Some(list_theme) = theme.get("list").filter(|v| !v.is_undefined()) {
            let tag = node.list_type.tag();
            let empty = Vec::new();
            let list_levels_class_names = list_theme
                .get(&format!("{tag}Depth"))
                .and_then(Value::as_array)
                .unwrap_or(&empty);
            // `$getListDepth(node) - 1`, and `%` by the number of level
            // classes (`NaN`, matching none, when there are none).
            let list_depth = get_list_depth(this)? as i64 - 1;
            let normalized_list_depth = (!list_levels_class_names.is_empty())
                .then(|| list_depth.rem_euclid(list_levels_class_names.len() as i64) as usize);
            let list_level_class_name =
                normalized_list_depth.and_then(|i| list_levels_class_names.get(i));
            if let Some(list_class_name) = list_theme.get(tag) {
                classes_to_add.extend(class_names_of(Some(list_class_name)));
            }
            if node.list_type == ListType::Check {
                classes_to_add.extend(class_names_of(list_theme.get("checklist")));
            }
            if let Some(list_level_class_name) = list_level_class_name {
                classes_to_add.extend(class_names_of(Some(list_level_class_name)));
                for (i, class_name) in list_levels_class_names.iter().enumerate() {
                    if Some(i) != normalized_list_depth {
                        classes_to_remove.extend(class_names_of(Some(class_name)));
                    }
                }
            }
            let nested_list_class_name = list_theme
                .get("nested")
                .and_then(|n| n.get("list"))
                .filter(|l| l.as_str().is_some_and(|s| !s.is_empty()));
            if nested_list_class_name.is_some() {
                let nested_list_item_classes = class_names_of(nested_list_class_name);
                if list_depth > 1 {
                    classes_to_add.extend(nested_list_item_classes);
                } else {
                    classes_to_remove.extend(nested_list_item_classes);
                }
            }
        }
        if !classes_to_remove.is_empty() {
            dom.class_list().remove(&classes_to_remove);
        }
        if !classes_to_add.is_empty() {
            dom.class_list().add(&classes_to_add);
        }
        Ok(())
    }

    /// `ListNode`'s `importDOM`: `<ol>` and `<ul>`.
    pub(crate) fn import_dom() -> DomConversionMap {
        vec![
            ("ol".to_owned(), importer(0, convert_list_node)),
            ("ul".to_owned(), importer(0, convert_list_node)),
        ]
    }

    /// `$normalizeChildren(nodes, listNode)`: the children of a list made
    /// from HTML, each a list item holding one nested list or inline
    /// content.
    fn normalize_children(nodes: Vec<NodeRef>) -> Result<Vec<NodeRef>, LexicalError> {
        let mut normalized_list_items = Vec::new();
        for node in nodes {
            if is_list_item_node(&node) {
                normalized_list_items.push(node.clone());
                let children = node.get_children();
                if children.len() > 1 {
                    for child in children {
                        if is_list_node(&child) {
                            let wrapper = create_list_item_node(None);
                            wrapper.append(&[child])?;
                            normalized_list_items.push(wrapper);
                        }
                    }
                }
            } else {
                let wrapper = create_list_item_node(None);
                wrapper.append(&[node])?;
                normalized_list_items.push(wrapper);
            }
        }
        Ok(normalized_list_items)
    }

    /// `isDomChecklist(domNode)`: a Lexical, GitHub, Joplin or Google Docs
    /// check list.
    fn is_dom_checklist(dom_node: &DomNode) -> bool {
        if dom_node.get_attribute("__lexicallisttype").as_deref() == Some("check")
            || dom_node.class_list().contains("contains-task-list")
            || dom_node.get_attribute("data-is-checklist").as_deref() == Some("1")
        {
            return true;
        }
        dom_node
            .child_nodes()
            .iter()
            .any(|child| child.is_element() && child.has_attribute("aria-checked"))
    }

    /// `HTMLOListElement.start`: the `start` attribute by the HTML rules
    /// for parsing integers, or 1.
    pub(crate) fn ol_start(element: &DomNode) -> f64 {
        let Some(value) = element.get_attribute("start") else {
            return 1.0;
        };
        let rest = value.trim_start_matches([' ', '\t', '\n', '\u{c}', '\r']);
        let (sign, rest) = match rest.as_bytes().first() {
            Some(b'-') => (-1, &rest[1..]),
            Some(b'+') => (1, &rest[1..]),
            _ => (1, rest),
        };
        let digits = &rest[..rest
            .bytes()
            .position(|b| !b.is_ascii_digit())
            .unwrap_or(rest.len())];
        match digits.parse::<i64>() {
            Ok(n) if (-2_147_483_648..=2_147_483_647).contains(&(sign * n)) => (sign * n) as f64,
            _ => 1.0,
        }
    }

    /// `$convertListNode(domNode)`: a numbered list from `<ol>`, a check
    /// list or a bullet list from `<ul>`, whose children become list items.
    fn convert_list_node(dom_node: &DomNode) -> Result<Option<DomConversionOutput>, LexicalError> {
        let node = if dom_node.is_element() && dom_node.local_name() == "ol" {
            create_list_node(ListType::Number, ol_start(dom_node))
        } else if is_dom_checklist(dom_node) {
            create_list_node(ListType::Check, 1.0)
        } else {
            create_list_node(ListType::Bullet, 1.0)
        };
        set_direction_from_dom(&node, dom_node);
        Ok(Some(DomConversionOutput {
            node: DomConversionNode::One(node),
            after: Some(Box::new(normalize_children)),
            ..DomConversionOutput::default()
        }))
    }
}

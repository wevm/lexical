//! `LexicalListItemNode.ts`: `ListItemNode`, `$createListItemNode` and
//! `$isListItemNode`.

use super::format_list::{handle_indent, handle_outdent, merge_lists};
use super::generated_json::{
    export_compact_list_item_node, export_list_item_node, update_list_item_node,
};
use super::list_node::{ListType, create_list_node, is_list_node};
use super::utils::{get_new_list_start, is_nested_list_node};
use crate::LexicalError;
use crate::caret::{CaretDirection, get_sibling_caret};
use crate::caret_utils::{
    SplitAtPointCaretNextOptions, insert_node_to_nearest_root_at_caret, rewind_sibling_caret,
};
use crate::error::invariant;
use crate::json::Object;
use crate::lexical_selection::{PointType, SelectionRef, Which, get_selection};
use crate::lexical_utils::{copy_node, set_node_key, with_writable};
use crate::node::{self, LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::{ElementData, is_root_or_shadow_root};
use crate::nodes::paragraph::{ParagraphNode, create_paragraph_node};

/// `ListItemNode`: an item of a [`ListNode`](super::ListNode). It holds
/// inline content, or, to nest a list, only that list.
#[derive(Clone, Debug)]
pub struct ListItemNode {
    base: NodeBase,
    pub(super) element: ElementData,
    pub(super) value: f64,
    pub(super) checked: Option<bool>,
}

impl Default for ListItemNode {
    /// `new ListItemNode()`: value 1, not a checkbox.
    fn default() -> Self {
        ListItemNode {
            base: NodeBase::default(),
            element: ElementData::default(),
            value: 1.0,
            checked: None,
        }
    }
}

/// `$createListItemNode(checked)`: `Some` makes it a checkbox, checked or
/// not.
pub fn create_list_item_node(checked: Option<bool>) -> NodeRef {
    set_node_key(Box::new(ListItemNode {
        checked,
        ..ListItemNode::default()
    }))
}

/// `$isListItemNode(node)`.
pub fn is_list_item_node(node: &NodeRef) -> bool {
    node.get_latest().is::<ListItemNode>()
}

fn with_item<R>(node: &NodeRef, f: impl FnOnce(&ListItemNode) -> R) -> Option<R> {
    node.get_latest().downcast_ref::<ListItemNode>().map(f)
}

fn set_item(node: &NodeRef, f: impl FnOnce(&mut ListItemNode)) {
    if is_list_item_node(node) {
        with_writable(node.get_key(), |n| {
            f(n.downcast_mut::<ListItemNode>().expect("a list item"))
        });
    }
}

impl NodeRef {
    /// `ListItemNode.getValue()`; `None` for any other node.
    pub fn get_list_item_value(&self) -> Option<f64> {
        with_item(self, |i| i.value)
    }

    /// `ListItemNode.setValue(value)`. Does nothing to any other node.
    pub fn set_list_item_value(&self, value: f64) {
        set_item(self, |i| i.value = value);
    }

    /// `ListItemNode.getChecked()`: whether the item is checked when its
    /// list is a check list, `None` otherwise (and for any other node).
    pub fn get_checked(&self) -> Option<bool> {
        let checked = with_item(self, |i| i.checked)?;
        let list_type = self.get_parent().and_then(|p| p.get_list_type());
        (list_type == Some(ListType::Check)).then_some(checked == Some(true))
    }

    /// `ListItemNode.setChecked(checked)`. Does nothing to any other node.
    pub fn set_checked(&self, checked: Option<bool>) {
        set_item(self, |i| i.checked = checked);
    }

    /// `ListItemNode.toggleChecked()`. Does nothing to any other node.
    pub fn toggle_checked(&self) {
        set_item(self, |i| i.checked = Some(i.checked != Some(true)));
    }
}

/// `ListItemNode.getIndent()`: how deep the item is nested in lists, or its
/// own `__indent` when it isn't attached.
fn get_list_item_indent(this: &NodeRef, own: f64) -> Result<f64, LexicalError> {
    // If we don't have a parent, we are likely serializing
    let Some(parent) = this.get_parent() else {
        return Ok(own);
    };
    if !this.is_attached() {
        return Ok(own);
    }
    // ListItemNode should always have a ListNode for a parent.
    let mut list_node_parent = parent.get_parent_or_throw()?;
    let mut indent_level = 0.0;
    while is_list_item_node(&list_node_parent) {
        list_node_parent = list_node_parent
            .get_parent_or_throw()?
            .get_parent_or_throw()?;
        indent_level += 1.0;
    }
    Ok(indent_level)
}

impl ListItemNode {
    pub fn value(&self) -> f64 {
        self.value
    }

    /// `__checked`, whatever the list's type.
    pub fn checked(&self) -> Option<bool> {
        self.checked
    }

    /// `getChecked()` and `getIndent()` for the export, which JS reads off
    /// the tree. An item whose parent is the root can't have an indent
    /// (`getParentOrThrow()` throws on the root), which JS's export throws
    /// on too.
    fn exported_checked_and_indent(&self) -> (Option<bool>, f64) {
        let this = NodeRef::new(self.base.key.clone());
        let checked = this.get_checked();
        let indent =
            get_list_item_indent(&this, self.element.indent).unwrap_or_else(|e| panic!("{e}"));
        (checked, indent)
    }

    /// `ListItemNode.canMergeWith(node)`.
    fn can_merge_with(node: &NodeRef) -> bool {
        is_list_item_node(node) || node.get_latest().is::<ParagraphNode>()
    }
}

impl LexicalNode for ListItemNode {
    fn get_type(&self) -> &'static str {
        "listitem"
    }
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        list_item_dom::create_dom(self, this, config)
    }
    #[cfg(feature = "html")]
    fn export_dom(
        &self,
        this: &NodeRef,
        editor: &crate::Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, LexicalError> {
        list_item_dom::export_dom(self, this, editor)
    }
    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        Some(list_item_dom::import_dom())
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

    /// `ListItemNode.isParentRequired()`.
    fn is_parent_required(&self) -> bool {
        true
    }

    /// `ListItemNode.createParentElementNode()`: a bullet list.
    fn create_parent_element_node(&self) -> NodeRef {
        create_list_node(ListType::Bullet, 1.0)
    }

    /// `ListItemNode.canMergeWhenEmpty()`.
    fn can_merge_when_empty(&self) -> bool {
        true
    }

    /// `exportListItemNode` and `exportCompactListItemNode`.
    fn export_json(&self, compact: bool) -> Object {
        let (checked, indent) = self.exported_checked_and_indent();
        if compact {
            export_compact_list_item_node(self, checked, indent)
        } else {
            export_list_item_node(self, checked, indent)
        }
    }

    /// `updateListItemNode`.
    fn update_from_json(&mut self, json: &Object) {
        update_list_item_node(self, json);
    }

    /// `ListItemNode.append(...nodes)`: a list item or paragraph appended
    /// gives its children instead (`canMergeWith`), and is removed.
    fn append(&self, this: &NodeRef, nodes: &[NodeRef]) -> Result<(), LexicalError> {
        for node in nodes {
            if node.is_element() && Self::can_merge_with(node) {
                let children = node.get_children();
                this.append(&children)?;
                node.remove(false)?;
            } else {
                // `super.append(node)`.
                this.splice(this.get_children_size(), 0, std::slice::from_ref(node))?;
            }
        }
        Ok(())
    }

    /// `ListItemNode.replace(replaceWithNode, includeChildren)`: replaced by
    /// anything but a list item, the item is outdented and the node goes
    /// beside its list, splitting it if the item is in the middle.
    fn replace(
        &self,
        this: &NodeRef,
        replace_with: &NodeRef,
        include_children: bool,
    ) -> Result<NodeRef, LexicalError> {
        if is_list_item_node(replace_with) {
            return node::replace(this, replace_with, false);
        }
        this.set_indent(0.0)?;
        let list = this.get_parent_or_throw()?;
        if !is_list_node(&list) {
            return Ok(replace_with.clone());
        }
        if list.get_first_child().as_ref() == Some(this) {
            list.insert_before(replace_with, true)?;
        } else if list.get_last_child().as_ref() == Some(this) {
            list.insert_after(replace_with, true)?;
        } else {
            // Split the list
            let new_list = copy_node(&list);
            let mut next_sibling = this.get_next_sibling();
            while let Some(node_to_append) = next_sibling {
                next_sibling = node_to_append.get_next_sibling();
                new_list.append(std::slice::from_ref(&node_to_append))?;
            }
            list.insert_after(replace_with, true)?;
            replace_with.insert_after(&new_list, true)?;
        }
        let to_replace_key = this.get_key().to_owned();
        let mut prev_size_before_children_transfer = 0;
        if include_children {
            invariant!(
                replace_with.is_element(),
                "includeChildren should only be true for ElementNodes"
            );
            prev_size_before_children_transfer = replace_with.get_children_size();
            replace_with.splice(prev_size_before_children_transfer, 0, &this.get_children())?;
        }
        // The base LexicalNode.replace remaps element-anchored selection
        // points from the replaced node to the replacement, but this
        // override skips super and the trailing this.remove() would
        // otherwise drop selection onto a sibling list item via
        // moveSelectionPointToSibling. Mirror the base behavior here for
        // the element-anchored case.
        if include_children
            && replace_with.is_element()
            && let Some(selection) = get_selection().filter(SelectionRef::is_range)
        {
            for which in [Which::Anchor, Which::Focus] {
                let point = selection.point(which).expect("a range selection");
                if point.key == to_replace_key && point.point_type == PointType::Element {
                    selection.point_set(
                        which,
                        replace_with.get_key(),
                        prev_size_before_children_transfer + point.offset,
                        PointType::Element,
                    )?;
                }
            }
        }
        this.remove(false)?;
        if list.get_children_size() == 0 {
            list.remove(false)?;
        }
        Ok(replace_with.clone())
    }

    /// `ListItemNode.insertAfter(node, restoreSelection)`: anything but a
    /// list item goes after the list, which splits around it.
    fn insert_after(
        &self,
        this: &NodeRef,
        node: &NodeRef,
        restore_selection: bool,
    ) -> Result<NodeRef, LexicalError> {
        let list_node = this.get_parent_or_throw()?;
        invariant!(
            is_list_node(&list_node),
            "insertAfter: list node is not parent of list item node"
        );
        if is_list_item_node(node) {
            return node::insert_after(this, node, restore_selection);
        }
        let siblings = this.get_next_siblings();
        // Split the lists and insert the node in between them
        list_node.insert_after(node, restore_selection)?;
        if let Some(first_sibling) = siblings.first() {
            let new_list_node = copy_node(&list_node);
            // $copyNode carries the original list's start, which would
            // restart the numbering at the split. The items moving into the
            // new list keep the numbers they were already rendered with, so
            // the new list has to start from the first of them (see issue
            // #7032).
            if new_list_node.get_list_type() == Some(ListType::Number)
                && is_list_item_node(first_sibling)
            {
                new_list_node.set_list_start(get_new_list_start(&list_node, first_sibling));
            }
            for sibling in &siblings {
                new_list_node.append(std::slice::from_ref(sibling))?;
            }
            node.insert_after(&new_list_node, restore_selection)?;
        }
        Ok(node.clone())
    }

    /// `ListItemNode.remove(preserveEmptyParent)`: the nested lists on
    /// either side, if of the same type, are joined.
    fn remove(&self, this: &NodeRef, preserve_empty_parent: bool) -> Result<(), LexicalError> {
        let prev_sibling = this.get_previous_sibling();
        let next_sibling = this.get_next_sibling();
        node::remove(this, preserve_empty_parent)?;
        if let (Some(prev), Some(next)) = (prev_sibling, next_sibling)
            && is_nested_list_node(&prev)
            && is_nested_list_node(&next)
        {
            let prev_list = prev.get_first_child().expect("a nested list");
            let next_list = next.get_first_child().expect("a nested list");
            // Only join the surrounding sublists when they are the same kind
            // of list, otherwise the second one loses its listType.
            if prev_list.get_list_type() == next_list.get_list_type() {
                merge_lists(&prev_list, &next_list)?;
                next.remove(false)?;
            }
        }
        Ok(())
    }

    /// `ListItemNode.resetOnCopyNodeFrom(original)`: a copy of a checked
    /// item is unchecked.
    fn reset_on_copy_node_from(&self, this: &NodeRef, original: &NodeRef) {
        if original.get_checked() == Some(true) {
            this.set_checked(Some(false));
        }
    }

    /// `ListItemNode.insertNewAfter(selection, restoreSelection)`: a copy of
    /// the item after it.
    fn insert_new_after(
        &self,
        this: &NodeRef,
        _: &SelectionRef,
        restore_selection: bool,
    ) -> Result<Option<NodeRef>, LexicalError> {
        let new_element = copy_node(this);
        this.insert_after(&new_element, restore_selection)?;
        Ok(Some(new_element))
    }

    /// `ListItemNode.collapseAtStart(selection)`: a nested item outdents;
    /// otherwise the item becomes a paragraph after its list, which splits.
    fn collapse_at_start(&self, this: &NodeRef, _: &SelectionRef) -> Result<bool, LexicalError> {
        if is_nested_list_node(this) {
            return Ok(false);
        }
        let list_node = this.get_parent_or_throw()?;
        let list_node_parent = list_node.get_parent_or_throw()?;
        if is_list_item_node(&list_node_parent) {
            handle_outdent(this)?;
            return Ok(true);
        }
        let paragraph = create_paragraph_node();
        paragraph.append(&this.get_children())?;
        let next_siblings = this.get_next_siblings();
        if !next_siblings.is_empty() {
            let new_list = copy_node(&list_node);
            new_list.append(&next_siblings)?;
            list_node.insert_after(&new_list, true)?;
        }
        list_node.insert_after(&paragraph, true)?;
        this.remove(false)?;
        if list_node.get_children_size() == 0 {
            list_node.remove(false)?;
        }
        paragraph.select_start()?;
        Ok(true)
    }

    /// `ListItemNode.getIndent()`.
    fn get_indent(&self, this: &NodeRef) -> Result<f64, LexicalError> {
        get_list_item_indent(this, self.element.indent)
    }

    /// `ListItemNode.setIndent(indent)`: nests or unwraps the item in lists
    /// one level at a time until it is `indent` deep.
    fn set_indent(&self, this: &NodeRef, indent: f64) -> Result<(), LexicalError> {
        let indent = indent.floor();
        // `indent >= 0`, which NaN fails.
        invariant!(
            indent.partial_cmp(&0.0).is_some_and(|o| o.is_ge()),
            "Indent value must be non-negative."
        );
        let target = indent;
        let mut current_indent = this.get_indent()?;
        while current_indent != target {
            if current_indent < target {
                handle_indent(this)?;
                current_indent += 1.0;
            } else {
                handle_outdent(this)?;
                current_indent -= 1.0;
            }
        }
        Ok(())
    }

    /// The `$transform` of `ListItemNode.$config()`: an item outside a list
    /// is wrapped, with the items beside it, in the list
    /// `createParentElementNode` makes, lifted out of blocks to the nearest
    /// root. (`NodeStructureExtension`, which lifts it itself where it
    /// enforces structure, isn't ported.)
    fn transform(&self, this: &NodeRef) -> Result<(), LexicalError> {
        let Some(parent) = this.get_parent() else {
            return Ok(());
        };
        if is_list_node(&parent) {
            if parent.get_list_type() != Some(ListType::Check) && this.get_checked().is_some() {
                this.set_checked(None);
            }
            return Ok(());
        }
        let new_parent = this.create_parent_element_node();
        invariant!(
            is_list_node(&new_parent),
            "ListItemNode.createParentElementNode() must return a ListNode"
        );
        // Insert an empty ListNode at the orphan's position, splitting any
        // enclosing non-shadow-root blocks so the ListNode lifts to a valid
        // container before we move the orphan in. The ListNode $transform
        // merges adjacent same-type lists, so neighbouring orphans will
        // coalesce once their own transforms run.
        let mut children = vec![this.clone()];
        for next in [false, true] {
            children.reverse();
            let mut sibling = if next {
                this.get_next_sibling()
            } else {
                this.get_previous_sibling()
            };
            while let Some(origin) = sibling.filter(is_list_item_node) {
                sibling = if next {
                    origin.get_next_sibling()
                } else {
                    origin.get_previous_sibling()
                };
                children.push(origin);
            }
        }
        this.insert_before(&new_parent, true)?;
        new_parent.splice(0, 0, &children)?;
        if !is_root_or_shadow_root(&*parent.get_latest()) {
            insert_node_to_nearest_root_at_caret(
                &new_parent,
                &rewind_sibling_caret(&get_sibling_caret(&new_parent, CaretDirection::Next))?,
                &SplitAtPointCaretNextOptions {
                    should_split: |_, _| false,
                    remove_empty_destination: true,
                    ..SplitAtPointCaretNextOptions::default()
                },
            )?;
            if parent.is_empty() && parent.is_attached() {
                parent.remove(false)?;
            }
        }
        Ok(())
    }
}

// The DOM side of `ListItemNode` (the `html` feature).

#[cfg(feature = "html")]
pub(super) mod list_item_dom {
    use super::super::list_node::{ListType, is_list_node};
    use super::super::utils::is_nested_list_node;
    use super::{ListItemNode, create_list_item_node};
    use crate::dom::DomNode;
    use crate::json::{Object, Value};
    use crate::lexical_dom::{
        DomConversionNode, DomConversionOutput, DomExportOutput, get_document,
        normalize_class_names, set_direction_from_dom, set_format_from_dom,
    };
    use crate::node::NodeRef;
    use crate::nodes::paragraph::ParagraphNode;
    use crate::set_dom_style::{get_style_object_from_css, set_dom_style_from_css};
    use crate::{EditorConfig, LexicalError};

    /// `ListItemNode.createDOM(config)`: an `<li>`, through
    /// `updateListItemDOM(null, element, config)`.
    pub(crate) fn create_dom(
        node: &ListItemNode,
        this: &NodeRef,
        config: &EditorConfig,
    ) -> Result<DomNode, LexicalError> {
        let dom = get_document().create_element("li");
        update_list_item_checked(&dom, node, this);
        // `dom.value = this.__value`: a `long`, written as its attribute.
        dom.set_attribute(
            "value",
            &crate::nodes::text::to_int32(node.value).to_string(),
        );
        set_list_item_theme_class_names(&dom, &config.theme, node, this);
        let style = node.element.get_style();
        if !style.is_empty() {
            set_dom_style_from_css(&dom.style(), style, "");
        }
        apply_marker_styles(&dom, node);
        Ok(dom)
    }

    /// `applyMarkerStyles(dom, node, null)`: the text style as
    /// `--listitem-marker-*` properties.
    fn apply_marker_styles(dom: &DomNode, node: &ListItemNode) {
        let style = dom.style();
        for (k, v) in get_style_object_from_css(node.element.get_text_style()).iter() {
            style.set_property(&format!("--listitem-marker-{k}"), v, "");
        }
    }

    /// `ListItemNode.exportDOM(editor)`: `createDOM`'s element with the
    /// alignment and the direction. A nested list's item moves its
    /// children into the item before it.
    pub(crate) fn export_dom(
        node: &ListItemNode,
        this: &NodeRef,
        editor: &crate::Editor,
    ) -> Result<DomExportOutput, LexicalError> {
        let element = create_dom(node, this, editor.config())?;
        let format_type = node.element.get_format_type();
        if !format_type.is_empty() {
            element.style().set_property("text-align", format_type, "");
        }
        if let Some(direction) = node.element.get_direction() {
            element.set_dir(direction.as_str());
        }
        if is_nested_list_node(this) {
            return Ok(DomExportOutput {
                element: Some(element),
                after: Some(Box::new(|container: &DomNode| {
                    if container.is_element()
                        && let Some(prev_sibling) = container.previous_element_sibling()
                        && prev_sibling.node_name() == "LI"
                    {
                        while let Some(child) = container.first_child() {
                            prev_sibling.append_child(&child);
                        }
                        container.remove();
                    }
                    Ok(Some(container.clone()))
                })),
                ..DomExportOutput::default()
            });
        }
        Ok(DomExportOutput::element(element))
    }

    fn class_names_of(value: Option<&Value>) -> Vec<String> {
        value
            .and_then(Value::as_str)
            .map(|v| normalize_class_names(&[v]))
            .unwrap_or_default()
    }

    /// `$setListItemThemeClassNames(dom, editorThemeClasses, node)`.
    fn set_list_item_theme_class_names(
        dom: &DomNode,
        theme: &Object,
        node: &ListItemNode,
        this: &NodeRef,
    ) {
        let Some(list_theme) = theme.get("list").filter(|t| {
            !matches!(t, Value::Undefined | Value::Null | Value::Bool(false))
                && t.as_str() != Some("")
        }) else {
            return;
        };
        let list_item_class_name = list_theme.get("listitem");
        let nested_list_item_class_name = list_theme.get("nested").and_then(|n| n.get("listitem"));
        let parent = this.get_parent();
        let is_check_list = parent
            .as_ref()
            .is_some_and(|p| p.get_list_type() == Some(ListType::Check));
        let checked = node.checked == Some(true);
        let is_nested = this.get_children().iter().any(is_list_node);
        let mut classes_to_remove = class_names_of(list_theme.get("listitemChecked"));
        classes_to_remove.extend(class_names_of(list_theme.get("listitemUnchecked")));
        classes_to_remove.extend(class_names_of(nested_list_item_class_name));
        if !classes_to_remove.is_empty() {
            dom.class_list().remove(&classes_to_remove);
        }
        let mut classes_to_add = class_names_of(list_item_class_name);
        if is_check_list {
            let key = if checked {
                "listitemChecked"
            } else {
                "listitemUnchecked"
            };
            classes_to_add.extend(class_names_of(list_theme.get(key)));
        }
        if is_nested {
            classes_to_add.extend(class_names_of(nested_list_item_class_name));
        }
        if !classes_to_add.is_empty() {
            dom.class_list().add(&classes_to_add);
        }
    }

    /// `updateListItemChecked(dom, listItemNode, null)`: a leaf item of a
    /// check list is a checkbox.
    fn update_list_item_checked(dom: &DomNode, node: &ListItemNode, this: &NodeRef) {
        let is_checkbox = this
            .get_parent()
            .is_some_and(|p| p.get_list_type() == Some(ListType::Check))
            && !this.get_first_child().is_some_and(|c| is_list_node(&c));
        if !is_checkbox {
            dom.remove_attribute("role");
            dom.remove_attribute("tabIndex");
            dom.remove_attribute("aria-checked");
        } else {
            dom.set_attribute("role", "checkbox");
            dom.set_attribute("tabIndex", "-1");
            dom.set_attribute(
                "aria-checked",
                if node.checked == Some(true) {
                    "true"
                } else {
                    "false"
                },
            );
        }
    }

    /// `ListItemNode`'s `importDOM`: `<li>`.
    pub(crate) fn import_dom() -> crate::lexical_dom::DomConversionMap {
        vec![(
            "li".to_owned(),
            crate::lexical_dom::importer(0, convert_list_item_element),
        )]
    }

    /// `$convertListItemElement(domNode)`: a list item, checked or not for
    /// a GitHub or Joplin task item or an `aria-checked` one.
    fn convert_list_item_element(
        dom_node: &DomNode,
    ) -> Result<Option<DomConversionOutput>, LexicalError> {
        if dom_node.class_list().contains("task-list-item") {
            for child in dom_node.children() {
                if child.tag_name() == "INPUT" {
                    return convert_checkbox_input(&child);
                }
            }
        }
        if dom_node.class_list().contains("joplin-checkbox") {
            for child in dom_node.children() {
                let grandchildren = child.children();
                if child.class_list().contains("checkbox-wrapper")
                    && grandchildren
                        .first()
                        .is_some_and(|c| c.tag_name() == "INPUT")
                {
                    return convert_checkbox_input(&grandchildren[0]);
                }
            }
        }
        let checked = match dom_node.get_attribute("aria-checked").as_deref() {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        };
        let node = create_list_item_node(checked);
        set_format_from_dom(&node, dom_node);
        set_direction_from_dom(&node, dom_node);
        Ok(Some(with_format_from_children(node)))
    }

    /// `$convertCheckboxInput(domNode)`.
    fn convert_checkbox_input(
        dom_node: &DomNode,
    ) -> Result<Option<DomConversionOutput>, LexicalError> {
        if dom_node.get_attribute("type").as_deref() != Some("checkbox") {
            return Ok(Some(DomConversionOutput::node(None)));
        }
        let node = create_list_item_node(Some(dom_node.has_attribute("checked")));
        Ok(Some(with_format_from_children(node)))
    }

    /// `{after: setFormatFromChildren.bind(null, node), node}`: an item
    /// whose one child is an aligned paragraph (as Google Docs writes)
    /// takes the paragraph's alignment and its children.
    fn with_format_from_children(node: NodeRef) -> DomConversionOutput {
        let list_item_node = node.clone();
        DomConversionOutput {
            node: DomConversionNode::One(node),
            after: Some(Box::new(move |children: Vec<NodeRef>| {
                if let [first_child] = children.as_slice()
                    && first_child.get_latest().is::<ParagraphNode>()
                    && list_item_node.element_data().get_format_type().is_empty()
                {
                    let format_type = first_child.element_data().get_format_type();
                    if !format_type.is_empty() {
                        list_item_node.set_element_format(format_type);
                        return Ok(first_child.get_children());
                    }
                }
                Ok(children)
            })),
            ..DomConversionOutput::default()
        }
    }
}

//! `MarkNode.ts`: `MarkNode`, `$createMarkNode` and `$isMarkNode`.

use super::generated_json::{export_compact_mark_node, export_mark_node, update_mark_node};
use crate::json::Object;
use crate::lexical_selection::SelectionRef;
use crate::lexical_utils::{set_node_key, with_writable};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::ElementData;
use crate::schema::{NodeSchema, array_value, node_schema, string_value, with_field};
use crate::{LexicalError, utf16};

/// `MarkNode`: an inline element over the content that one or more ids
/// (comment threads, for example) annotate.
#[derive(Clone, Debug, Default)]
pub struct MarkNode {
    base: NodeBase,
    pub(super) element: ElementData,
    pub(super) ids: Vec<String>,
}

impl MarkNode {
    /// `__ids`.
    pub fn ids(&self) -> &[String] {
        &self.ids
    }
}

/// `$createMarkNode(ids)`.
pub fn create_mark_node(ids: &[String]) -> NodeRef {
    set_node_key(Box::new(MarkNode {
        ids: ids.to_vec(),
        ..MarkNode::default()
    }))
}

/// `$isMarkNode(node)`.
pub fn is_mark_node(node: &NodeRef) -> bool {
    node.get_latest().is::<MarkNode>()
}

/// Changes a mark's ids (`getWritable()`); does nothing to any other node.
fn with_ids(node: &NodeRef, f: impl FnOnce(&mut Vec<String>)) {
    if is_mark_node(node) {
        with_writable(node.get_key(), |n| {
            f(&mut n.downcast_mut::<MarkNode>().expect("a mark").ids)
        });
    }
}

impl NodeRef {
    /// `MarkNode.hasID(id)`; false for any other node.
    pub fn has_id(&self, id: &str) -> bool {
        self.get_ids()
            .is_some_and(|ids| ids.iter().any(|i| i == id))
    }

    /// `MarkNode.getIDs()`: a copy of the ids; `None` for any other node.
    pub fn get_ids(&self) -> Option<Vec<String>> {
        self.get_latest()
            .downcast_ref::<MarkNode>()
            .map(|m| m.ids.clone())
    }

    /// `MarkNode.setIDs(ids)`, as given (duplicates too). Does nothing to
    /// any other node.
    pub fn set_ids(&self, ids: &[String]) {
        with_ids(self, |i| *i = ids.to_vec());
    }

    /// `MarkNode.addID(id)`: appends `id` unless the mark has it. The mark
    /// is written either way. Does nothing to any other node.
    pub fn add_id(&self, id: &str) {
        with_ids(self, |ids| {
            if !ids.iter().any(|i| i == id) {
                ids.push(id.to_owned());
            }
        });
    }

    /// `MarkNode.deleteID(id)`: removes the first `id`, if any. The mark is
    /// written either way, and stays when it has no ids left. Does nothing
    /// to any other node.
    pub fn delete_id(&self, id: &str) {
        with_ids(self, |ids| {
            if let Some(idx) = ids.iter().position(|i| i == id) {
                ids.remove(idx);
            }
        });
    }
}

thread_local! {
    static MARK_NODE_SCHEMA: NodeSchema = node_schema(vec![(
        "ids",
        with_field(
            array_value(string_value("")).expect("an array of strings"),
            |n: &MarkNode| &n.ids,
            |n: &mut MarkNode| &mut n.ids,
        )
        .expect("a field"),
    )])
    .expect("MarkNode's schema");
}

/// `markNodeSchema`: `ids`, read with `getIDs` (a copy of the field) and
/// written to the field.
pub fn mark_node_schema() -> NodeSchema {
    MARK_NODE_SCHEMA.with(Clone::clone)
}

impl LexicalNode for MarkNode {
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![
            mark_node_schema(),
            crate::nodes::element::element_node_schema(),
        ]
    }
    /// The `$config()` of `MarkNode`: `inline: true`, `children: 'inline'`.
    fn static_node_config_chain(&self) -> Vec<crate::structure::NodeConfigLink> {
        use crate::structure::{NodeChildrenConfig, NodeConfigLink, StaticNodeConfig};
        vec![NodeConfigLink::new(
            "mark",
            StaticNodeConfig {
                inline: Some(true),
                children: Some(NodeChildrenConfig::Inline),
                ..StaticNodeConfig::default()
            },
        )]
    }

    fn get_type(&self) -> &'static str {
        "mark"
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

    /// `MarkNode.createDOM(config)`: a `<mark>` with the theme's `mark`
    /// class names, and its `markOverlap` ones when the mark has more than
    /// one id. (`updateDOM` is the reconciler's.)
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, crate::LexicalError> {
        use crate::lexical_dom::{add_theme_class_names, get_document};
        let element = get_document().create_element("mark");
        add_theme_class_names(&element, &config.theme, "mark");
        if self.ids.len() > 1 {
            add_theme_class_names(&element, &config.theme, "markOverlap");
        }
        Ok(element)
    }

    /// `MarkNode.isInline()`.
    fn is_inline(&self) -> bool {
        true
    }

    /// `MarkNode.canBeEmpty()`: a mark left with no children is removed.
    fn can_be_empty(&self) -> bool {
        false
    }

    /// `MarkNode.canInsertTextBefore()`: text typed at the mark's start
    /// goes before it.
    fn can_insert_text_before(&self) -> bool {
        false
    }

    /// `MarkNode.canInsertTextAfter()`: text typed at the mark's end goes
    /// after it.
    fn can_insert_text_after(&self) -> bool {
        false
    }

    /// `MarkNode.insertNewAfter(selection, restoreSelection)`: a new mark
    /// with the same ids, after it.
    fn insert_new_after(
        &self,
        this: &NodeRef,
        _: &SelectionRef,
        restore_selection: bool,
    ) -> Result<Option<NodeRef>, LexicalError> {
        let mark_node = create_mark_node(&self.ids);
        this.insert_after(&mark_node, restore_selection)?;
        Ok(Some(mark_node))
    }

    /// `MarkNode.extractWithChild(child, selection, destination)`: a copy
    /// (not to HTML) of a range selection whose points are both in the mark
    /// takes the mark with it when the difference of their offsets is the
    /// mark's text length (offsets in different nodes are compared as they
    /// are, as in JS).
    fn extract_with_child(
        &self,
        this: &NodeRef,
        _: &NodeRef,
        selection: Option<&SelectionRef>,
        destination: &str,
    ) -> Result<bool, LexicalError> {
        let Some(selection) = selection.filter(|s| s.is_range()) else {
            return Ok(false);
        };
        if destination == "html" {
            return Ok(false);
        }
        let range = selection.range().expect("a range selection");
        let anchor_node = range.anchor.get_node()?;
        let focus_node = range.focus.get_node()?;
        let (anchor, focus) = (range.anchor.offset as i64, range.focus.offset as i64);
        let selection_length = if selection.is_backward()? {
            anchor - focus
        } else {
            focus - anchor
        };
        Ok(this.is_parent_of(&anchor_node)
            && this.is_parent_of(&focus_node)
            && utf16::len(&this.get_text_content()) as i64 == selection_length)
    }

    /// `MarkNode.excludeFromCopy(destination)`: left out of every copy but
    /// a clone.
    fn exclude_from_copy(&self, destination: &str) -> bool {
        destination != "clone"
    }

    /// `exportMarkNode` and `exportCompactMarkNode`.
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        Some(if compact {
            export_compact_mark_node(self)
        } else {
            export_mark_node(self)
        })
    }

    /// `updateMarkNode`.
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        update_mark_node(self, json);
        true
    }
}

#[cfg(all(test, feature = "html"))]
mod tests {
    use crate::json::Value;
    use crate::{CreateEditorArgs, create_editor};

    /// `createDOM`: the theme's `mark` class names, and its `markOverlap`
    /// ones only with more than one id (as Swift's port; the HTML export
    /// leaves marks out, so `html/export-mark` can't see them).
    #[test]
    fn create_dom_adds_the_theme_classes() {
        let mut theme = crate::json::Object::new();
        theme.insert("mark", Value::from("m"));
        theme.insert("markOverlap", Value::from("o"));
        let editor = create_editor(CreateEditorArgs {
            nodes: super::super::mark_nodes(),
            theme,
            ..CreateEditorArgs::default()
        });
        let html = |ids: &[&str]| {
            let ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
            let mark = super::MarkNode {
                ids,
                ..super::MarkNode::default()
            };
            crate::node::LexicalNode::create_dom(
                &mark,
                &crate::lexical_utils::get_root(),
                editor.config(),
                &editor,
            )
            .map(|dom| dom.outer_html())
        };
        let read_editor = editor.clone();
        let classes = read_editor
            .read(|| [html(&[]), html(&["a"]), html(&["a", "b"])])
            .unwrap()
            .map(Result::unwrap);
        assert_eq!(
            classes,
            [
                r#"<mark class="m"></mark>"#,
                r#"<mark class="m"></mark>"#,
                r#"<mark class="m o"></mark>"#,
            ]
        );
    }
}

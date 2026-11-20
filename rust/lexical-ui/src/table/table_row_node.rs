//! `LexicalTableRowNode.ts`: `TableRowNode`, `$createTableRowNode` and
//! `$isTableRowNode`.

use super::generated_json::{
    export_compact_table_row_node, export_table_row_node, update_table_row_node,
};
use crate::LexicalError;
use crate::json::Object;
use crate::lexical_selection::SelectionRef;
use crate::lexical_utils::{set_node_key, with_writable};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::{self, ElementData};
use crate::schema::{
    NodeSchema, NumberValueOptions, node_schema, number_value, optional, with_field,
};

/// `TableRowNode`: a row of a [`TableNode`](super::TableNode), holding
/// [`TableCellNode`](super::TableCellNode)s.
#[derive(Clone, Debug, Default)]
pub struct TableRowNode {
    base: NodeBase,
    pub(super) element: ElementData,
    /// `__height`.
    pub(super) height: Option<f64>,
}

impl TableRowNode {
    /// `__height`.
    pub fn height(&self) -> Option<f64> {
        self.height
    }
}

/// `$createTableRowNode(height)`.
pub fn create_table_row_node(height: Option<f64>) -> NodeRef {
    crate::lexical_utils::replaced(set_node_key(Box::new(TableRowNode {
        height,
        ..TableRowNode::default()
    })))
}

/// `$isTableRowNode(node)`.
pub fn is_table_row_node(node: &NodeRef) -> bool {
    node.get_latest().is::<TableRowNode>()
}

impl NodeRef {
    /// `TableRowNode.setHeight(height)`; does nothing to any other node.
    pub fn set_row_height(&self, height: Option<f64>) {
        if is_table_row_node(self) {
            with_writable(self.get_key(), |n| {
                n.downcast_mut::<TableRowNode>()
                    .expect("a table row")
                    .height = height;
            });
        }
    }

    /// `TableRowNode.getHeight()`.
    pub fn get_row_height(&self) -> Option<f64> {
        self.get_latest()
            .downcast_ref::<TableRowNode>()
            .and_then(|r| r.height)
    }
}

thread_local! {
    static TABLE_ROW_NODE_SCHEMA: NodeSchema =
        build_table_row_node_schema().expect("TableRowNode's schema");
}

/// `tableRowNodeSchema`: `height`, optional.
pub fn table_row_node_schema() -> NodeSchema {
    TABLE_ROW_NODE_SCHEMA.with(Clone::clone)
}

fn build_table_row_node_schema() -> Result<NodeSchema, LexicalError> {
    node_schema(vec![(
        "height",
        with_field(
            optional(number_value(0.0, NumberValueOptions::default())?, false)?,
            |n: &TableRowNode| &n.height,
            |n: &mut TableRowNode| &mut n.height,
        )?,
    )])
}

impl LexicalNode for TableRowNode {
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![table_row_node_schema(), element::element_node_schema()]
    }

    fn get_type(&self) -> &'static str {
        "tablerow"
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

    /// `TableRowNode.extractWithChild(child, selection, destination)`: a
    /// copy to HTML takes the row with its cells.
    fn extract_with_child(
        &self,
        _: &NodeRef,
        _: &NodeRef,
        _: Option<&SelectionRef>,
        destination: &str,
    ) -> Result<bool, LexicalError> {
        Ok(destination == "html")
    }

    /// `TableRowNode.isShadowRoot()`.
    fn is_shadow_root(&self) -> bool {
        true
    }

    /// `TableRowNode.canBeEmpty()`.
    fn can_be_empty(&self) -> bool {
        false
    }

    /// `TableRowNode.canIndent()`.
    fn can_indent(&self) -> bool {
        false
    }

    /// `TableRowNode.updateDOM(prevNode)`.
    fn update_dom(&self, prev_node: &dyn LexicalNode) -> bool {
        prev_node
            .as_any()
            .downcast_ref::<TableRowNode>()
            .is_none_or(|prev| prev.height != self.height)
    }

    /// `TableRowNode.createDOM(config)`: a `<tr>` with the height and the
    /// theme's `tableRow` class names.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        use crate::lexical_dom::{add_theme_class_names, get_document};
        let element = get_document().create_element("tr");
        if let Some(height) = self.height.filter(|h| *h != 0.0 && !h.is_nan()) {
            element.style().set_property(
                "height",
                &format!("{}px", crate::json::number_to_string(height)),
                "",
            );
        }
        add_theme_class_names(&element, &config.theme, "tableRow");
        Ok(element)
    }

    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        use crate::lexical_dom::importer;
        Some(vec![(
            "tr".to_owned(),
            importer(0, convert_table_row_element),
        )])
    }

    /// `exportTableRowNode` and `exportCompactTableRowNode`.
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        Some(if compact {
            export_compact_table_row_node(self)
        } else {
            export_table_row_node(self)
        })
    }

    /// `updateTableRowNode`.
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        update_table_row_node(self, json);
        true
    }
}

/// `$convertTableRowElement(domNode)`: a row with the element's pixel
/// height and direction, keeping only the cells among its descendants.
#[cfg(feature = "html")]
fn convert_table_row_element(
    dom_node: &crate::dom::DomNode,
) -> Result<Option<crate::lexical_dom::DomConversionOutput>, LexicalError> {
    use super::constants::parse_pixel_value;
    use super::table_cell_node::is_table_cell_node;
    use crate::lexical_dom::{DomConversionOutput, set_direction_from_dom};
    let height = parse_pixel_value(&dom_node.style().get_property_value("height"));
    let node = create_table_row_node(height);
    set_direction_from_dom(&node, dom_node);
    Ok(Some(DomConversionOutput {
        after: Some(Box::new(|children: Vec<NodeRef>| {
            Ok(crate::utils::descendants_matching(
                &children,
                is_table_cell_node,
            ))
        })),
        ..DomConversionOutput::node(Some(node))
    }))
}

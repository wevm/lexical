//! `LexicalTableCellNode.ts`: `TableCellNode`, `TableCellHeaderStates`,
//! `$createTableCellNode` and `$isTableCellNode`.

use super::generated_json::{
    export_compact_table_cell_node, export_table_cell_node, update_table_cell_node,
};
use crate::LexicalError;
use crate::json::{Object, Value};
use crate::lexical_selection::SelectionRef;
use crate::lexical_utils::{set_node_key, with_writable};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::{self, ElementData};
use crate::schema::{
    NodeSchema, NumberValueOptions, enum_value, node_schema, nullable, number_value, optional,
    schema_getter, schema_setter, string_value, with_accessors, with_field,
};

/// `TableCellHeaderStates`: the bits of a cell's header state. A cell can
/// head its row, its column, both, or neither.
#[derive(Clone, Copy, Debug)]
pub struct TableCellHeaderStates;

impl TableCellHeaderStates {
    pub const NO_STATUS: i32 = 0;
    pub const ROW: i32 = 1;
    pub const COLUMN: i32 = 2;
    pub const BOTH: i32 = 3;
}

/// `TableCellNode`: a cell of a [`TableRowNode`](super::TableRowNode). It is a
/// shadow root that holds blocks.
#[derive(Clone, Debug)]
pub struct TableCellNode {
    base: NodeBase,
    pub(super) element: ElementData,
    /// `__colSpan`: a number a document sets, as JS keeps it.
    pub(super) col_span: f64,
    /// `__rowSpan`.
    pub(super) row_span: f64,
    /// `__headerState`: [`TableCellHeaderStates`] bits.
    pub(super) header_state: i32,
    /// `__width`.
    pub(super) width: Option<f64>,
    /// `__backgroundColor`.
    pub(super) background_color: Option<String>,
    /// `__verticalAlign`: any string `setVerticalAlign` was given; only
    /// `middle` and `bottom` are written out.
    pub(super) vertical_align: Option<String>,
}

impl Default for TableCellNode {
    /// `new TableCellNode()`: no header, spans of 1, no width.
    fn default() -> Self {
        TableCellNode {
            base: NodeBase::default(),
            element: ElementData::default(),
            col_span: 1.0,
            row_span: 1.0,
            header_state: TableCellHeaderStates::NO_STATUS,
            width: None,
            background_color: None,
            vertical_align: None,
        }
    }
}

impl TableCellNode {
    /// `getSerializedVerticalAlign()`: the vertical alignment when it is
    /// `middle` or `bottom`, otherwise `None` (left out of the JSON).
    pub fn serialized_vertical_align(&self) -> Option<&str> {
        self.vertical_align
            .as_deref()
            .filter(|v| is_valid_vertical_align(Some(v)))
    }

    /// `hasHeader()` of this value.
    pub fn has_header(&self) -> bool {
        self.header_state != TableCellHeaderStates::NO_STATUS
    }

    /// `getTag()`: `th` for a header cell, `td` otherwise.
    pub fn get_tag(&self) -> &'static str {
        if self.has_header() { "th" } else { "td" }
    }

    /// `__colSpan`.
    pub fn col_span(&self) -> f64 {
        self.col_span
    }

    /// `__rowSpan`.
    pub fn row_span(&self) -> f64 {
        self.row_span
    }

    /// `__headerState`.
    pub fn header_state(&self) -> i32 {
        self.header_state
    }

    /// `__width`.
    pub fn width(&self) -> Option<f64> {
        self.width
    }

    /// `__backgroundColor`.
    pub fn background_color(&self) -> Option<&str> {
        self.background_color.as_deref()
    }

    /// `__verticalAlign`.
    pub fn vertical_align(&self) -> Option<&str> {
        self.vertical_align.as_deref()
    }
}

/// `isValidVerticalAlign(verticalAlign)`.
pub(super) fn is_valid_vertical_align(vertical_align: Option<&str>) -> bool {
    matches!(vertical_align, Some("middle" | "bottom"))
}

/// `$createTableCellNode(headerState, colSpan, width)`.
pub fn create_table_cell_node(header_state: i32, col_span: f64, width: Option<f64>) -> NodeRef {
    crate::lexical_utils::replaced(set_node_key(Box::new(TableCellNode {
        header_state,
        col_span,
        width,
        ..TableCellNode::default()
    })))
}

/// `$createTableCellNode()` with its defaults: no header, a span of 1.
pub fn create_default_table_cell_node() -> NodeRef {
    create_table_cell_node(TableCellHeaderStates::NO_STATUS, 1.0, None)
}

/// `$isTableCellNode(node)`.
pub fn is_table_cell_node(node: &NodeRef) -> bool {
    node.get_latest().is::<TableCellNode>()
}

fn read<R>(node: &NodeRef, f: impl FnOnce(&TableCellNode) -> R) -> Option<R> {
    node.get_latest().downcast_ref::<TableCellNode>().map(f)
}

/// Changes a cell (`getWritable()`); does nothing to any other node.
pub(super) fn with_cell(node: &NodeRef, f: impl FnOnce(&mut TableCellNode)) {
    if is_table_cell_node(node) {
        with_writable(node.get_key(), |n| {
            f(n.downcast_mut::<TableCellNode>().expect("a table cell"))
        });
    }
}

impl NodeRef {
    /// `TableCellNode.getColSpan()`; 1 for any other node.
    pub fn get_col_span(&self) -> f64 {
        read(self, |c| c.col_span).unwrap_or(1.0)
    }

    /// `TableCellNode.setColSpan(colSpan)`.
    pub fn set_col_span(&self, col_span: f64) {
        with_cell(self, |c| c.col_span = col_span);
    }

    /// `TableCellNode.getRowSpan()`; 1 for any other node.
    pub fn get_row_span(&self) -> f64 {
        read(self, |c| c.row_span).unwrap_or(1.0)
    }

    /// `TableCellNode.setRowSpan(rowSpan)`.
    pub fn set_row_span(&self, row_span: f64) {
        with_cell(self, |c| c.row_span = row_span);
    }

    /// `TableCellNode.getTag()`; `None` for any other node.
    pub fn get_table_cell_tag(&self) -> Option<&'static str> {
        read(self, TableCellNode::get_tag)
    }

    /// `TableCellNode.setHeaderStyles(headerState, mask)`: the bits of
    /// `mask` take those of `header_state`, the others stay.
    pub fn set_header_styles(&self, header_state: i32, mask: i32) {
        with_cell(self, |c| {
            c.header_state = (header_state & mask) | (c.header_state & !mask);
        });
    }

    /// `TableCellNode.getHeaderStyles()`; `NO_STATUS` for any other node.
    pub fn get_header_styles(&self) -> i32 {
        read(self, |c| c.header_state).unwrap_or(TableCellHeaderStates::NO_STATUS)
    }

    /// `TableCellNode.setWidth(width)`.
    pub fn set_cell_width(&self, width: Option<f64>) {
        with_cell(self, |c| c.width = width);
    }

    /// `TableCellNode.getWidth()`.
    pub fn get_cell_width(&self) -> Option<f64> {
        read(self, |c| c.width).flatten()
    }

    /// `TableCellNode.getBackgroundColor()`.
    pub fn get_background_color(&self) -> Option<String> {
        read(self, |c| c.background_color.clone()).flatten()
    }

    /// `TableCellNode.setBackgroundColor(newBackgroundColor)`.
    pub fn set_background_color(&self, background_color: Option<&str>) {
        with_cell(self, |c| {
            c.background_color = background_color.map(str::to_owned)
        });
    }

    /// `TableCellNode.getVerticalAlign()`.
    pub fn get_vertical_align(&self) -> Option<String> {
        read(self, |c| c.vertical_align.clone()).flatten()
    }

    /// `TableCellNode.setVerticalAlign(newVerticalAlign)`: an empty string is
    /// `undefined`.
    pub fn set_vertical_align(&self, vertical_align: Option<&str>) {
        with_cell(self, |c| {
            c.vertical_align = vertical_align.filter(|v| !v.is_empty()).map(str::to_owned);
        });
    }

    /// `TableCellNode.toggleHeaderStyle(headerStateToToggle)`: clears the
    /// bits when the cell has all of them, sets them otherwise.
    pub fn toggle_header_style(&self, header_state_to_toggle: i32) {
        with_cell(self, |c| {
            if (c.header_state & header_state_to_toggle) == header_state_to_toggle {
                c.header_state &= !header_state_to_toggle;
            } else {
                c.header_state |= header_state_to_toggle;
            }
        });
    }

    /// `TableCellNode.hasHeaderState(headerState)`.
    pub fn has_header_state(&self, header_state: i32) -> bool {
        (self.get_header_styles() & header_state) == header_state
    }

    /// `TableCellNode.hasHeader()`.
    pub fn has_header(&self) -> bool {
        read(self, TableCellNode::has_header).unwrap_or(false)
    }
}

thread_local! {
    static TABLE_CELL_NODE_SCHEMA: NodeSchema =
        build_table_cell_node_schema().expect("TableCellNode's schema");
}

/// `tableCellNodeSchema`: `backgroundColor` (an empty string is `null`),
/// `colSpan` and `rowSpan` (positive integers, else 1), `headerState`
/// (applied with `setHeaderStyles`), `verticalAlign` (`middle` or `bottom`,
/// else absent) and `width` (0 is absent).
pub fn table_cell_node_schema() -> NodeSchema {
    TABLE_CELL_NODE_SCHEMA.with(Clone::clone)
}

fn build_table_cell_node_schema() -> Result<NodeSchema, LexicalError> {
    let span = || {
        number_value(
            1.0,
            NumberValueOptions {
                integer: true,
                min: Some(1.0),
                ..NumberValueOptions::default()
            },
        )
    };
    node_schema(vec![
        (
            "backgroundColor",
            with_field(
                nullable(string_value(""), true)?,
                |n: &TableCellNode| &n.background_color,
                |n: &mut TableCellNode| &mut n.background_color,
            )?,
        ),
        (
            "colSpan",
            with_field(
                span()?,
                |n: &TableCellNode| &n.col_span,
                |n: &mut TableCellNode| &mut n.col_span,
            )?,
        ),
        (
            "headerState",
            with_accessors(
                number_value(0.0, NumberValueOptions::default())?,
                Some(schema_getter(|n: &TableCellNode| {
                    f64::from(n.header_state).into()
                })),
                Some(schema_setter(|n: &mut TableCellNode, value| {
                    let header_state = crate::nodes::text::to_int32(value.as_f64().unwrap_or(0.0));
                    let mask = TableCellHeaderStates::BOTH;
                    n.header_state = (header_state & mask) | (n.header_state & !mask);
                })),
            )?,
        ),
        (
            "rowSpan",
            with_field(
                span()?,
                |n: &TableCellNode| &n.row_span,
                |n: &mut TableCellNode| &mut n.row_span,
            )?,
        ),
        (
            "verticalAlign",
            with_accessors(
                enum_value(
                    vec![Value::Undefined, "middle".into(), "bottom".into()],
                    None,
                )?,
                Some(schema_getter(|n: &TableCellNode| {
                    n.serialized_vertical_align()
                        .map_or(Value::Undefined, Value::from)
                })),
                Some(schema_setter(|n: &mut TableCellNode, value| {
                    n.vertical_align = value.as_str().map(str::to_owned);
                })),
            )?,
        ),
        (
            "width",
            with_field(
                optional(number_value(0.0, NumberValueOptions::default())?, true)?,
                |n: &TableCellNode| &n.width,
                |n: &mut TableCellNode| &mut n.width,
            )?,
        ),
    ])
}

impl LexicalNode for TableCellNode {
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![table_cell_node_schema(), element::element_node_schema()]
    }

    /// The `$config()` of `TableCellNode`: `children: 'block'`.
    fn static_node_config_chain(&self) -> Vec<crate::structure::NodeConfigLink> {
        use crate::structure::{NodeChildrenConfig, NodeConfigLink, StaticNodeConfig};
        vec![NodeConfigLink::new(
            "tablecell",
            StaticNodeConfig {
                children: Some(NodeChildrenConfig::Block),
                ..StaticNodeConfig::default()
            },
        )]
    }

    fn get_type(&self) -> &'static str {
        "tablecell"
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

    /// `TableCellNode.isShadowRoot()`.
    fn is_shadow_root(&self) -> bool {
        true
    }

    /// `TableCellNode.collapseAtStart()`: a backward deletion at the cell's
    /// start does nothing.
    fn collapse_at_start(&self, _: &NodeRef, _: &SelectionRef) -> Result<bool, LexicalError> {
        Ok(true)
    }

    /// `TableCellNode.canBeEmpty()`.
    fn can_be_empty(&self) -> bool {
        false
    }

    /// `TableCellNode.canIndent()`.
    fn can_indent(&self) -> bool {
        false
    }

    /// `TableCellNode.updateDOM(prevNode)`: whether a field the element
    /// shows changed.
    fn update_dom(&self, prev_node: &dyn LexicalNode) -> bool {
        let Some(prev) = prev_node.as_any().downcast_ref::<TableCellNode>() else {
            return true;
        };
        prev.header_state != self.header_state
            || prev.width != self.width
            || prev.col_span != self.col_span
            || prev.row_span != self.row_span
            || prev.background_color != self.background_color
            || prev.vertical_align != self.vertical_align
    }

    /// `TableCellNode.createDOM(config)`.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(cell_dom::create_dom(self, &config.theme))
    }

    /// `TableCellNode.exportDOM(editor)`.
    #[cfg(feature = "html")]
    fn export_dom(
        &self,
        this: &NodeRef,
        editor: &crate::Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, LexicalError> {
        cell_dom::export_dom(self, this, editor)
    }

    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        use crate::lexical_dom::importer;
        Some(vec![
            (
                "td".to_owned(),
                importer(0, cell_dom::convert_table_cell_node_element),
            ),
            (
                "th".to_owned(),
                importer(0, cell_dom::convert_table_cell_node_element),
            ),
        ])
    }

    /// `exportTableCellNode` and `exportCompactTableCellNode`.
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        Some(if compact {
            export_compact_table_cell_node(self)
        } else {
            export_table_cell_node(self)
        })
    }

    /// `updateTableCellNode`.
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        update_table_cell_node(self, json);
        true
    }
}

#[cfg(feature = "html")]
pub(super) mod cell_dom {
    use super::{
        TableCellHeaderStates, TableCellNode, create_table_cell_node, is_valid_vertical_align,
    };
    use crate::LexicalError;
    use crate::dom::DomNode;
    use crate::json::{Object, number_to_string};
    use crate::lexical_dom::{
        DomConversionOutput, DomExportOutput, add_class_names_to_element, set_direction_from_dom,
        theme_string,
    };
    use crate::node::NodeRef;
    use crate::nodes::paragraph::create_paragraph_node;
    use crate::table::constants::{COLUMN_WIDTH, parse_pixel_value};
    use crate::table::dom_helpers::{cell_index, col_span, row_index, row_span, unsigned_long};

    /// `TableCellNode.createDOM(config)`: a `<td>` or `<th>` with the
    /// width, spans, background color, vertical alignment and the theme's
    /// `tableCell` (and for a header `tableCellHeader`) class names.
    pub(crate) fn create_dom(cell: &TableCellNode, theme: &Object) -> DomNode {
        let element = crate::lexical_dom::get_document().create_element(cell.get_tag());
        if let Some(width) = cell.width.filter(|w| *w != 0.0 && !w.is_nan()) {
            element
                .style()
                .set_property("width", &format!("{}px", number_to_string(width)), "");
        }
        if cell.col_span > 1.0 {
            element.set_attribute("colspan", &unsigned_long(cell.col_span).to_string());
        }
        if cell.row_span > 1.0 {
            element.set_attribute("rowspan", &unsigned_long(cell.row_span).to_string());
        }
        if let Some(background_color) = &cell.background_color {
            element
                .style()
                .set_property("background-color", background_color, "");
        }
        if is_valid_vertical_align(cell.vertical_align.as_deref()) {
            element.style().set_property(
                "vertical-align",
                cell.vertical_align.as_deref().unwrap_or_default(),
                "",
            );
        }
        let mut classes: Vec<&str> = Vec::new();
        if let Some(c) = theme_string(theme, "tableCell") {
            classes.push(c);
        }
        if cell.has_header()
            && let Some(c) = theme_string(theme, "tableCellHeader")
        {
            classes.push(c);
        }
        add_class_names_to_element(&element, &classes);
        element
    }

    /// `TableCellNode.exportDOM(editor)`: `ElementNode`'s, with the node's
    /// key (for `TableNode`'s export to find the cell's columns), a border,
    /// the spans, a width (75px by default), the vertical alignment (`top`
    /// by default), `text-align: start` and a header's background.
    pub(crate) fn export_dom(
        cell: &TableCellNode,
        this: &NodeRef,
        editor: &crate::Editor,
    ) -> Result<DomExportOutput, LexicalError> {
        let output = crate::nodes::element::export_dom(cell, this, editor)?;
        if let Some(element) = output.element.as_ref().filter(|e| e.is_element()) {
            element.set_attribute("data-temporary-table-cell-lexical-key", this.get_key());
            let style = element.style();
            style.set_property("border", "1px solid black", "");
            if cell.col_span > 1.0 {
                element.set_attribute("colspan", &unsigned_long(cell.col_span).to_string());
            }
            if cell.row_span > 1.0 {
                element.set_attribute("rowspan", &unsigned_long(cell.row_span).to_string());
            }
            let width = cell
                .width
                .filter(|w| *w != 0.0 && !w.is_nan())
                .unwrap_or(COLUMN_WIDTH);
            style.set_property("width", &format!("{}px", number_to_string(width)), "");
            style.set_property(
                "vertical-align",
                cell.vertical_align
                    .as_deref()
                    .filter(|v| !v.is_empty())
                    .unwrap_or("top"),
                "",
            );
            style.set_property("text-align", "start", "");
            if cell.background_color.is_none() && cell.has_header() {
                style.set_property("background-color", "#f2f3f5", "");
            }
        }
        Ok(output)
    }

    /// `$convertTableCellNodeElement(domNode)`: a cell with the element's
    /// header state (from `th`, its `scope`, or its place in the table),
    /// spans, pixel width, background color, vertical alignment and
    /// direction. Its inline children are wrapped in paragraphs and take the
    /// cell's bold, italic, underline, strikethrough and color.
    pub(crate) fn convert_table_cell_node_element(
        dom_node: &DomNode,
    ) -> Result<Option<DomConversionOutput>, LexicalError> {
        let node_name = dom_node.node_name().to_ascii_lowercase();
        let style = dom_node.style();
        let width = parse_pixel_value(&style.get_property_value("width"));
        let mut header_state = TableCellHeaderStates::NO_STATUS;
        if node_name == "th" {
            match dom_node.get_attribute("scope").as_deref() {
                Some("col") => header_state = TableCellHeaderStates::COLUMN,
                Some("row") => header_state = TableCellHeaderStates::ROW,
                _ => {
                    let parent_row = dom_node.parent_element();
                    let is_in_header_row = parent_row.as_ref().is_some_and(|row| {
                        row.is_element()
                            && row.node_name().eq_ignore_ascii_case("tr")
                            && row.parent_element().is_some_and(|section| {
                                section.is_element()
                                    && (section.node_name().eq_ignore_ascii_case("thead")
                                        || row_index(row) == 0)
                            })
                    });
                    let is_first_column = cell_index(dom_node) == 0;
                    if is_in_header_row {
                        header_state |= TableCellHeaderStates::ROW;
                    }
                    if is_first_column {
                        header_state |= TableCellHeaderStates::COLUMN;
                    }
                    if header_state == TableCellHeaderStates::NO_STATUS {
                        header_state = TableCellHeaderStates::ROW;
                    }
                }
            }
        }
        let table_cell_node = create_table_cell_node(header_state, col_span(dom_node), width);
        let row_span = row_span(dom_node);
        super::with_cell(&table_cell_node, |c| c.row_span = row_span);
        let background_color = style.get_property_value("background-color");
        if !background_color.is_empty() {
            super::with_cell(&table_cell_node, |c| {
                c.background_color = Some(background_color.clone());
            });
        }
        let vertical_align = style.get_property_value("vertical-align");
        if is_valid_vertical_align(Some(&vertical_align)) {
            super::with_cell(&table_cell_node, |c| {
                c.vertical_align = Some(vertical_align.clone());
            });
        }
        set_direction_from_dom(&table_cell_node, dom_node);

        let text_decoration = style.get_property_value("text-decoration");
        let text_decoration: Vec<&str> = text_decoration.split(' ').collect();
        let font_weight = style.get_property_value("font-weight");
        let has_bold_font_weight = font_weight == "700" || font_weight == "bold";
        let has_linethrough_text_decoration = text_decoration.contains(&"line-through");
        let has_italic_font_style = style.get_property_value("font-style") == "italic";
        let has_underline_text_decoration = text_decoration.contains(&"underline");
        let color = style.get_property_value("color");
        Ok(Some(DomConversionOutput {
            after: Some(Box::new(move |child_lexical_nodes: Vec<NodeRef>| {
                let mut result: Vec<NodeRef> = Vec::new();
                let mut paragraph_node: Option<NodeRef> = None;
                let remove_single_line_break_node =
                    |paragraph_node: &Option<NodeRef>| -> Result<(), LexicalError> {
                        if let Some(paragraph_node) = paragraph_node {
                            let first_child = paragraph_node.get_first_child();
                            if let Some(first_child) = first_child
                                && first_child
                                    .get_latest()
                                    .is::<crate::nodes::line_break::LineBreakNode>()
                                && paragraph_node.get_children_size() == 1
                            {
                                first_child.remove(false)?;
                            }
                        }
                        Ok(())
                    };
                for child in child_lexical_nodes {
                    let is_inline_or_decorator =
                        (child.is_element() || child.is_decorator()) && child.is_inline();
                    if is_inline_or_decorator
                        || child.is_text()
                        || child
                            .get_latest()
                            .is::<crate::nodes::line_break::LineBreakNode>()
                    {
                        if child.is_text() {
                            if has_bold_font_weight {
                                child.toggle_format("bold");
                            }
                            if has_linethrough_text_decoration {
                                child.toggle_format("strikethrough");
                            }
                            if has_italic_font_style {
                                child.toggle_format("italic");
                            }
                            if has_underline_text_decoration {
                                child.toggle_format("underline");
                            }
                            if !color.is_empty() {
                                let existing_style = child.text_data().get_style().to_owned();
                                if !existing_style.contains("color:") {
                                    child.set_style(&format!("{existing_style}color: {color};"));
                                }
                            }
                        }
                        match &paragraph_node {
                            Some(paragraph) => {
                                paragraph.append(&[child])?;
                            }
                            None => {
                                let paragraph = create_paragraph_node();
                                paragraph.append(&[child])?;
                                result.push(paragraph.clone());
                                paragraph_node = Some(paragraph);
                            }
                        }
                    } else {
                        result.push(child);
                        remove_single_line_break_node(&paragraph_node)?;
                        paragraph_node = None;
                    }
                }
                remove_single_line_break_node(&paragraph_node)?;
                if result.is_empty() {
                    result.push(create_paragraph_node());
                }
                Ok(result)
            })),
            ..DomConversionOutput::node(Some(table_cell_node))
        }))
    }
}

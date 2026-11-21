//! `LexicalTableNode.ts`: `TableNode`, `$createTableNode`, `$isTableNode`,
//! and the editor's scrollable tables flag (`$isScrollableTablesActive`,
//! `setScrollableTablesActive`).
//!
//! Not ported: the sticky scrollbar's listeners and measurements, the DOM
//! coordinates (`getCordsFromCellNode`, `getDOMCellFromCords`,
//! `getCellNodeFromCords`), which read a rendered table, and
//! `scaleDOMColWidths`. The model navigates the table map instead
//! (`table_utils`, rust/PARITY.md).

use super::generated_json::{export_compact_table_node, export_table_node, update_table_node};
use super::table_cell_node::is_table_cell_node;
use super::table_row_node::is_table_row_node;
use crate::json::{Object, Value};
use crate::lexical_selection::SelectionRef;
use crate::lexical_utils::{set_node_key, with_writable};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::{self, ElementData};
use crate::schema::{
    NodeSchema, NumberValueOptions, array_value, boolean_value, node_schema, number_value,
    optional, schema_getter, schema_setter, with_accessors,
};
use crate::{Editor, LexicalError};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// `TableNode`: a table, holding [`TableRowNode`](super::TableRowNode)s.
#[derive(Clone, Debug, Default)]
pub struct TableNode {
    base: NodeBase,
    pub(super) element: ElementData,
    /// `__rowStriping`.
    pub(super) row_striping: bool,
    /// `__frozenColumnCount`.
    pub(super) frozen_column_count: f64,
    /// `__frozenRowCount`.
    pub(super) frozen_row_count: f64,
    /// `__colWidths`.
    pub(super) col_widths: Option<Vec<f64>>,
}

/// JS's `x || undefined` for a number: `None` for 0 and NaN.
fn truthy(x: f64) -> Option<f64> {
    (x != 0.0 && !x.is_nan()).then_some(x)
}

impl TableNode {
    /// `getSerializedFrozenColumnCount()`.
    pub fn serialized_frozen_column_count(&self) -> Option<f64> {
        truthy(self.frozen_column_count)
    }

    /// `getSerializedFrozenRowCount()`.
    pub fn serialized_frozen_row_count(&self) -> Option<f64> {
        truthy(self.frozen_row_count)
    }

    /// `getSerializedRowStriping()`.
    pub fn serialized_row_striping(&self) -> Option<bool> {
        self.row_striping.then_some(true)
    }

    /// `__colWidths`.
    pub fn col_widths(&self) -> Option<&[f64]> {
        self.col_widths.as_deref()
    }
}

/// `$createTableNode()`.
pub fn create_table_node() -> NodeRef {
    crate::lexical_utils::replaced(set_node_key(Box::<TableNode>::default()))
}

/// `$isTableNode(node)`.
pub fn is_table_node(node: &NodeRef) -> bool {
    node.get_latest().is::<TableNode>()
}

fn read<R>(node: &NodeRef, f: impl FnOnce(&TableNode) -> R) -> Option<R> {
    node.get_latest().downcast_ref::<TableNode>().map(f)
}

fn with_table(node: &NodeRef, f: impl FnOnce(&mut TableNode)) {
    if is_table_node(node) {
        with_writable(node.get_key(), |n| {
            f(n.downcast_mut::<TableNode>().expect("a table"))
        });
    }
}

impl NodeRef {
    /// `TableNode.getColWidths()`.
    pub fn get_col_widths(&self) -> Option<Vec<f64>> {
        read(self, |t| t.col_widths.clone()).flatten()
    }

    /// `TableNode.setColWidths(colWidths)`.
    pub fn set_col_widths(&self, col_widths: Option<Vec<f64>>) {
        with_table(self, |t| t.col_widths = col_widths);
    }

    /// `TableNode.getRowStriping()`.
    pub fn get_row_striping(&self) -> bool {
        read(self, |t| t.row_striping).unwrap_or(false)
    }

    /// `TableNode.setRowStriping(newRowStriping)`.
    pub fn set_row_striping(&self, row_striping: bool) {
        with_table(self, |t| t.row_striping = row_striping);
    }

    /// `TableNode.setFrozenColumns(columnCount)`.
    pub fn set_frozen_columns(&self, column_count: f64) {
        with_table(self, |t| t.frozen_column_count = column_count);
    }

    /// `TableNode.getFrozenColumns()`.
    pub fn get_frozen_columns(&self) -> f64 {
        read(self, |t| t.frozen_column_count).unwrap_or(0.0)
    }

    /// `TableNode.setFrozenRows(rowCount)`.
    pub fn set_frozen_rows(&self, row_count: f64) {
        with_table(self, |t| t.frozen_row_count = row_count);
    }

    /// `TableNode.getFrozenRows()`.
    pub fn get_frozen_rows(&self) -> f64 {
        read(self, |t| t.frozen_row_count).unwrap_or(0.0)
    }

    /// `TableNode.getColumnCount()`: the spans of the first row's cells.
    pub fn get_column_count(&self) -> usize {
        let Some(first_row) = self.get_first_child().filter(is_table_row_node) else {
            return 0;
        };
        first_row
            .get_children()
            .iter()
            .filter(|cell| is_table_cell_node(cell))
            .map(|cell| cell.get_col_span())
            .sum::<f64>() as usize
    }
}

thread_local! {
    static TABLE_NODE_SCHEMA: NodeSchema = build_table_node_schema().expect("TableNode's schema");
    /// `scrollableEditors`: a `WeakSet` of editors.
    static SCROLLABLE_EDITORS: RefCell<Vec<Weak<crate::editor::EditorInner>>> =
        const { RefCell::new(Vec::new()) };
    /// The editors whose `TableExtension` has a sticky scrollbar, which JS
    /// reads from the extension's output (`$isStickyScrollbarActive`).
    static STICKY_EDITORS: RefCell<Vec<Weak<crate::editor::EditorInner>>> =
        const { RefCell::new(Vec::new()) };
}

fn set_in(
    set: &'static std::thread::LocalKey<RefCell<Vec<Weak<crate::editor::EditorInner>>>>,
    editor: &Editor,
    active: bool,
) {
    set.with(|s| {
        let mut s = s.borrow_mut();
        s.retain(|e| e.strong_count() > 0 && !std::ptr::eq(e.as_ptr(), Rc::as_ptr(&editor.0)));
        if active {
            s.push(Rc::downgrade(&editor.0));
        }
    });
}

fn has_in(
    set: &'static std::thread::LocalKey<RefCell<Vec<Weak<crate::editor::EditorInner>>>>,
    editor: &Editor,
) -> bool {
    set.with(|s| {
        s.borrow()
            .iter()
            .any(|e| std::ptr::eq(e.as_ptr(), Rc::as_ptr(&editor.0)))
    })
}

/// `$isScrollableTablesActive(editor)`: whether the editor's tables render
/// in a horizontally scrolling wrapper.
pub fn is_scrollable_tables_active(editor: &Editor) -> bool {
    has_in(&SCROLLABLE_EDITORS, editor)
}

/// `setScrollableTablesActive(editor, active)`.
pub fn set_scrollable_tables_active(editor: &Editor, active: bool) {
    set_in(&SCROLLABLE_EDITORS, editor, active);
}

/// `$isStickyScrollbarActive(editor)`: the table extension's
/// `hasStickyScrollbar` and `hasHorizontalScroll`.
pub fn is_sticky_scrollbar_active(editor: &Editor) -> bool {
    has_in(&STICKY_EDITORS, editor)
}

#[cfg(feature = "extension")]
pub(super) fn set_sticky_scrollbar_active(editor: &Editor, active: bool) {
    set_in(&STICKY_EDITORS, editor, active);
}

/// `tableNodeSchema`: `colWidths` (read off the field, applied with
/// `setColWidths`), `frozenColumnCount`, `frozenRowCount` and
/// `rowStriping`, each read through a getter that leaves out a falsy value.
pub fn table_node_schema() -> NodeSchema {
    TABLE_NODE_SCHEMA.with(Clone::clone)
}

fn build_table_node_schema() -> Result<NodeSchema, LexicalError> {
    let number = || number_value(0.0, NumberValueOptions::default());
    node_schema(vec![
        (
            "colWidths",
            with_accessors(
                optional(array_value(number()?)?, false)?,
                Some(schema_getter(|n: &TableNode| {
                    n.col_widths.as_ref().map_or(Value::Undefined, |w| {
                        Value::Array(w.iter().map(|x| Value::Number(*x)).collect())
                    })
                })),
                Some(schema_setter(|n: &mut TableNode, value| {
                    n.col_widths = value
                        .as_array()
                        .map(|w| w.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect());
                })),
            )?,
        ),
        (
            "frozenColumnCount",
            with_accessors(
                number()?,
                Some(schema_getter(|n: &TableNode| {
                    n.serialized_frozen_column_count()
                        .map_or(Value::Undefined, Value::from)
                })),
                Some(schema_setter(|n: &mut TableNode, value| {
                    n.frozen_column_count = value.as_f64().unwrap_or(0.0);
                })),
            )?,
        ),
        (
            "frozenRowCount",
            with_accessors(
                number()?,
                Some(schema_getter(|n: &TableNode| {
                    n.serialized_frozen_row_count()
                        .map_or(Value::Undefined, Value::from)
                })),
                Some(schema_setter(|n: &mut TableNode, value| {
                    n.frozen_row_count = value.as_f64().unwrap_or(0.0);
                })),
            )?,
        ),
        (
            "rowStriping",
            with_accessors(
                boolean_value(false),
                Some(schema_getter(|n: &TableNode| {
                    n.serialized_row_striping()
                        .map_or(Value::Undefined, Value::from)
                })),
                Some(schema_setter(|n: &mut TableNode, value| {
                    n.row_striping = value.as_bool().unwrap_or(false);
                })),
            )?,
        ),
    ])
}

impl LexicalNode for TableNode {
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![table_node_schema(), element::element_node_schema()]
    }

    fn get_type(&self) -> &'static str {
        "table"
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

    /// `TableNode.extractWithChild(child, selection, destination)`.
    fn extract_with_child(
        &self,
        _: &NodeRef,
        _: &NodeRef,
        _: Option<&SelectionRef>,
        destination: &str,
    ) -> Result<bool, LexicalError> {
        Ok(destination == "html")
    }

    /// `TableNode.canBeEmpty()`.
    fn can_be_empty(&self) -> bool {
        false
    }

    /// `TableNode.isShadowRoot()`.
    fn is_shadow_root(&self) -> bool {
        true
    }

    /// `TableNode.canIndent()`.
    fn can_indent(&self) -> bool {
        false
    }

    /// `TableNode.createDOM(config, editor)`.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        this: &NodeRef,
        config: &crate::EditorConfig,
        editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(table_dom::create_dom(self, this, &config.theme, editor))
    }

    /// `TableNode.exportDOM(editor)`.
    #[cfg(feature = "html")]
    fn export_dom(
        &self,
        this: &NodeRef,
        editor: &crate::Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, LexicalError> {
        table_dom::export_dom(self, this, editor)
    }

    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        use crate::lexical_dom::importer;
        Some(vec![(
            "table".to_owned(),
            importer(1, table_dom::convert_table_element),
        )])
    }

    /// `exportTableNode` and `exportCompactTableNode`.
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        Some(if compact {
            export_compact_table_node(self)
        } else {
            export_table_node(self)
        })
    }

    /// `updateTableNode`.
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        update_table_node(self, json);
        true
    }
}

#[cfg(feature = "html")]
pub(super) mod table_dom {
    use super::{TableNode, create_table_node, is_scrollable_tables_active};
    use crate::LexicalError;
    use crate::dom::DomNode;
    use crate::json::{Object, Value, number_to_string};
    use crate::lexical_dom::{
        DomConversionOutput, DomExportOutput, add_class_names_to_element, get_document,
        set_direction_from_dom, theme_string,
    };
    use crate::node::NodeRef;
    use crate::set_dom_style::set_dom_style_from_css;
    use crate::table::constants::parse_pixel_value;
    use crate::table::table_row_node::is_table_row_node;
    use std::collections::{HashMap, HashSet};

    fn remove_class_names(element: &DomNode, class_names: &[&str]) {
        for names in class_names {
            let tokens: Vec<&str> = names.split_whitespace().collect();
            if !tokens.is_empty() {
                element.class_list().remove(&tokens);
            }
        }
    }

    fn first_child_named(node: &DomNode, name: &str) -> Option<DomNode> {
        node.children()
            .into_iter()
            .find(|c| c.local_name().eq_ignore_ascii_case(name))
    }

    fn children_named(node: &DomNode, name: &str) -> Vec<DomNode> {
        node.children()
            .into_iter()
            .filter(|c| c.local_name().eq_ignore_ascii_case(name))
            .collect()
    }

    fn is_table(node: &DomNode) -> bool {
        node.is_element() && node.local_name() == "table"
    }

    /// `$updateColgroup(dom, colCount, colWidths)`.
    fn update_colgroup(dom: &DomNode, col_count: usize, col_widths: Option<&[f64]>) {
        let col_group = first_child_named(dom, "colgroup");
        let Some(col_widths) = col_widths else {
            if let Some(col_group) = col_group {
                col_group.remove();
            }
            return;
        };
        let col_group = col_group.unwrap_or_else(|| {
            let col_group = get_document().create_element("colgroup");
            dom.insert_before(&col_group, dom.first_child().as_ref());
            col_group
        });
        let cols: Vec<DomNode> = (0..col_count)
            .map(|i| {
                let col = get_document().create_element("col");
                if let Some(width) = col_widths.get(i).copied().and_then(super::truthy) {
                    col.style().set_property(
                        "width",
                        &format!("{}px", number_to_string(width)),
                        "",
                    );
                }
                col
            })
            .collect();
        col_group.replace_children(&cols);
    }

    /// `alignTableElement(dom, config, formatType)`: the theme's
    /// `tableAlignment` class for `center` or `right`.
    fn align_table_element(dom: &DomNode, theme: &Object, format_type: &str) {
        let Some(Value::Object(alignment)) = theme.get("tableAlignment") else {
            return;
        };
        let mut remove = Vec::new();
        let mut add = Vec::new();
        for format in ["center", "right"] {
            let Some(classes) = theme_string(alignment, format) else {
                continue;
            };
            if format == format_type {
                add.push(classes);
            } else {
                remove.push(classes);
            }
        }
        remove_class_names(dom, &remove);
        add_class_names_to_element(dom, &add);
    }

    /// `updateTableElement(null, tableElement, config)`, for a new element.
    fn update_table_element(table: &TableNode, this: &NodeRef, element: &DomNode, theme: &Object) {
        let style = table.element.get_style();
        if !style.is_empty() {
            set_dom_style_from_css(&element.style(), style, "");
        }
        if table.row_striping {
            if let Some(classes) = theme_string(theme, "tableRowStriping") {
                add_class_names_to_element(element, &[classes]);
            }
            element.set_attribute("data-lexical-row-striping", "true");
        }
        let column_count = this.get_column_count();
        if column_count != 0 || table.col_widths.is_some() {
            update_colgroup(element, column_count, table.col_widths.as_deref());
        }
        align_table_element(element, theme, table.element.get_format_type());
    }

    /// `TableNode.createDOM(config, editor)`: a `<table>` (with a
    /// `<colgroup>` for its column widths), in a scrolling wrapper when the
    /// editor's tables scroll, and with a sticky scrollbar after it when
    /// the extension has one.
    pub(crate) fn create_dom(
        table: &TableNode,
        this: &NodeRef,
        theme: &Object,
        editor: &crate::Editor,
    ) -> DomNode {
        let table_element = get_document().create_element("table");
        let style = table.element.get_style();
        if !style.is_empty() {
            set_dom_style_from_css(&table_element.style(), style, "");
        }
        if table.col_widths.is_some() {
            table_element.append_child(&get_document().create_element("colgroup"));
        }
        if let Some(classes) = theme_string(theme, "table") {
            add_class_names_to_element(&table_element, &[classes]);
        }
        update_table_element(table, this, &table_element, theme);
        if !is_scrollable_tables_active(editor) {
            return table_element;
        }
        let has_sticky_scrollbar = super::is_sticky_scrollbar_active(editor);
        let wrapper = get_document().create_element("div");
        match theme_string(theme, "tableScrollableWrapper") {
            Some(classes) => add_class_names_to_element(&wrapper, &[classes]),
            None => {
                wrapper.style().set_property("overflow-x", "auto", "");
            }
        }
        if has_sticky_scrollbar {
            wrapper.style().set_property("scrollbar-width", "none", "");
        }
        wrapper.append_child(&table_element);
        // `updateTableWrapper(null, wrapper, tableElement, config)`.
        let frozen = |count: f64, key: &str, attribute: &str| {
            if count > 0.0 {
                if let Some(classes) = theme_string(theme, key) {
                    add_class_names_to_element(&wrapper, &[classes]);
                }
                table_element.set_attribute(attribute, "true");
            }
        };
        if table.frozen_column_count != 0.0 {
            frozen(
                table.frozen_column_count,
                "tableFrozenColumn",
                "data-lexical-frozen-column",
            );
        }
        if table.frozen_row_count != 0.0 {
            frozen(
                table.frozen_row_count,
                "tableFrozenRow",
                "data-lexical-frozen-row",
            );
        }
        if !has_sticky_scrollbar {
            return wrapper;
        }
        let scrollbar = get_document().create_element("div");
        match theme_string(theme, "tableStickyScrollbar") {
            Some(classes) => add_class_names_to_element(&scrollbar, &[classes]),
            None => {
                let style = scrollbar.style();
                style.set_property("position", "sticky", "");
                style.set_property("bottom", "0", "");
                style.set_property("overflow-x", "scroll", "");
                style.set_property("overflow-y", "hidden", "");
            }
        }
        scrollbar.style().set_property("display", "none", "");
        scrollbar.set_attribute("aria-hidden", "true");
        scrollbar.set_attribute("tabindex", "-1");
        let spacer = get_document().create_element("div");
        spacer.style().set_property("height", "1px", "");
        spacer.style().set_property("width", "0px", "");
        scrollbar.append_child(&spacer);
        let outer = get_document().create_element("div");
        outer.set_attribute("data-lexical-sticky-scrollbar", "true");
        outer.append_child(&wrapper);
        outer.append_child(&scrollbar);
        outer
    }

    /// The `<table>` an element is or holds (`querySelector('table')`).
    fn find_table(element: &DomNode) -> Option<DomNode> {
        if is_table(element) {
            return Some(element.clone());
        }
        element
            .descendant_elements()
            .into_iter()
            .find(|e| e.local_name() == "table")
    }

    /// `TableNode.exportDOM(editor)`: `ElementNode`'s export of the
    /// `<table>` (its direction and indent moved off a scroll wrapper), its
    /// alignment, the `<col>`s of the columns the exported cells are in, and
    /// its rows in a `<tbody>`.
    pub(crate) fn export_dom(
        table: &TableNode,
        this: &NodeRef,
        editor: &crate::Editor,
    ) -> Result<DomExportOutput, LexicalError> {
        let super_export = crate::nodes::element::export_dom(table, this, editor)?;
        let element = super_export.element.clone();
        let exported_element = match &element {
            Some(e) if e.is_element() && !is_table(e) => find_table(e),
            other => other.clone(),
        };
        if let (Some(element), Some(exported)) = (&element, &exported_element)
            && element.is_element()
            && is_table(exported)
            && exported != element
        {
            if let Some(dir) = element.get_attribute("dir").filter(|d| !d.is_empty()) {
                exported.set_attribute("dir", &dir);
            }
            let padding = element.style().get_property_value("padding-inline-start");
            if !padding.is_empty() {
                exported
                    .style()
                    .set_property("padding-inline-start", &padding, "");
            }
            if let Some(indent) = element.get_attribute("data-lexical-indent") {
                exported.set_attribute("data-lexical-indent", &indent);
            }
        }
        let super_after = super_export.after;
        let this = this.clone();
        let theme = editor.config().theme.clone();
        let format_type = table.element.get_format_type();
        Ok(DomExportOutput {
            element: exported_element,
            after: Some(Box::new(move |table_element: &DomNode| {
                let mut table_element = table_element.clone();
                if let Some(after) = super_after {
                    match after(&table_element)? {
                        Some(e) => table_element = e,
                        None => return Ok(None),
                    }
                }
                let Some(table_element) = find_table(&table_element) else {
                    return Ok(None);
                };
                align_table_element(&table_element, &theme, format_type);

                // The columns each cell of the table map starts at.
                let (table_map, _, _) =
                    crate::table::table_utils::compute_table_map_skip_cell_check(
                        &this, None, None,
                    )?;
                let mut cell_values: HashMap<String, (usize, usize)> = HashMap::new();
                for map_row in &table_map {
                    for value in map_row.iter().flatten() {
                        let key = value.cell.get_key().to_owned();
                        cell_values
                            .entry(key)
                            .or_insert((value.cell.get_col_span() as usize, value.start_column));
                    }
                }
                // The columns of the cells that were exported.
                let mut known_columns: HashSet<usize> = HashSet::new();
                for row in children_named(&table_element, "tr") {
                    for cell_dom in row.children() {
                        let Some(key) =
                            cell_dom.get_attribute("data-temporary-table-cell-lexical-key")
                        else {
                            continue;
                        };
                        cell_dom.remove_attribute("data-temporary-table-cell-lexical-key");
                        if let Some((col_span, start_column)) = cell_values.remove(&key) {
                            for i in 0..col_span {
                                known_columns.insert(i + start_column);
                            }
                        }
                    }
                }
                if let Some(col_group) = first_child_named(&table_element, "colgroup") {
                    let cols: Vec<DomNode> = children_named(&col_group, "col")
                        .into_iter()
                        .enumerate()
                        .filter(|(i, _)| known_columns.contains(i))
                        .map(|(_, col)| col)
                        .collect();
                    col_group.replace_children(&cols);
                }
                let rows = children_named(&table_element, "tr");
                if !rows.is_empty() {
                    let t_body = get_document().create_element("tbody");
                    for row in rows {
                        t_body.append_child(&row);
                    }
                    table_element.append_child(&t_body);
                }
                Ok(Some(table_element))
            })),
            append: super_export.append,
            get_child_nodes: super_export.get_child_nodes,
        })
    }

    /// Whether `value` is all ASCII digits (`/^\d+$/`).
    fn is_digits(value: &str) -> bool {
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
    }

    /// `$convertTableElement(domNode)`: a table with the element's row
    /// striping, frozen first column and row, column widths (from the
    /// `<col>`s' pixel widths or their `width` attributes) and direction,
    /// keeping only the rows among its descendants.
    pub(crate) fn convert_table_element(
        dom_node: &DomNode,
    ) -> Result<Option<DomConversionOutput>, LexicalError> {
        let table_node = create_table_node();
        if dom_node.has_attribute("data-lexical-row-striping") {
            table_node.set_row_striping(true);
        }
        if dom_node.has_attribute("data-lexical-frozen-column") {
            table_node.set_frozen_columns(1.0);
        }
        if dom_node.has_attribute("data-lexical-frozen-row") {
            table_node.set_frozen_rows(1.0);
        }
        if let Some(col_group) = first_child_named(dom_node, "colgroup") {
            let mut columns = Some(Vec::new());
            for col in children_named(&col_group, "col") {
                let width = col.style().get_property_value("width");
                let width = match parse_pixel_value(&width) {
                    Some(width) => width,
                    None => match col.get_attribute("width").filter(|w| is_digits(w)) {
                        Some(width) => width.parse().unwrap_or(0.0),
                        None => {
                            columns = None;
                            break;
                        }
                    },
                };
                if let Some(columns) = columns.as_mut() {
                    columns.push(width);
                }
            }
            if let Some(columns) = columns {
                table_node.set_col_widths(Some(columns));
            }
        }
        set_direction_from_dom(&table_node, dom_node);
        Ok(Some(DomConversionOutput {
            after: Some(Box::new(|children: Vec<NodeRef>| {
                Ok(crate::utils::descendants_matching(
                    &children,
                    is_table_row_node,
                ))
            })),
            ..DomConversionOutput::node(Some(table_node))
        }))
    }
}

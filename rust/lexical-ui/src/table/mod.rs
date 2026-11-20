//! `@lexical/table` (the `table` feature), from `packages/lexical-table/src`:
//! `TableNode`, `TableRowNode` and `TableCellNode` with their generated JSON,
//! `TableSelection`, the table map utilities of `LexicalTableUtils.ts`,
//! `INSERT_TABLE_COMMAND`, and the model parts of the table handlers.
//!
//! Not ported: the DOM highlight of a table selection, pointer selection
//! and the sticky scrollbar, which read or drive a rendered table. Up and
//! down navigation reads the table map and the app's caret movement in
//! place of the DOM's layout (rust/PARITY.md).

mod constants;
#[cfg(feature = "html")]
mod dom_helpers;
mod generated_json;
mod table_cell_node;
mod table_commands;
#[cfg(feature = "extension")]
mod table_extension;
#[cfg(all(feature = "html", feature = "extension"))]
mod table_import_extension;
mod table_node;
mod table_observer;
mod table_plugin_helpers;
mod table_row_node;
pub(crate) mod table_selection;
mod table_selection_helpers;
mod table_utils;

pub use constants::{COLUMN_WIDTH, parse_pixel_value};
pub use table_cell_node::{
    TableCellHeaderStates, TableCellNode, create_default_table_cell_node, create_table_cell_node,
    is_table_cell_node,
};
pub use table_commands::{
    INSERT_TABLE_COMMAND, InsertTableCommandPayload, InsertTableCommandPayloadHeaders,
};
#[cfg(feature = "extension")]
pub use table_extension::{
    TableConfig, TableConfigOverrides, TableExtensionType, TableOutput, table_extension,
};
#[cfg(all(feature = "html", feature = "extension"))]
pub use table_import_extension::{table_import_rules, table_row_schema, table_schema};
pub use table_node::{
    TableNode, create_table_node, is_scrollable_tables_active, is_sticky_scrollbar_active,
    is_table_node, set_scrollable_tables_active,
};
pub use table_plugin_helpers::{register_table_cell_unmerge_transform, register_table_plugin};
pub use table_row_node::{TableRowNode, create_table_row_node, is_table_row_node};
pub use table_selection::{
    TableSelection, TableSelectionShape, create_table_selection, create_table_selection_from,
    is_table_selection,
};
pub use table_selection_helpers::{
    find_cell_node, find_table_node, register_table_selection_observer,
};
pub use table_utils::*;

use crate::NodeClass;

/// The node classes this package registers (`TableExtension`'s `nodes`):
/// `TableNode`, `TableRowNode` and `TableCellNode`.
pub fn table_nodes() -> Vec<NodeClass> {
    vec![
        NodeClass::of::<TableNode>(),
        NodeClass::of::<TableRowNode>(),
        NodeClass::of::<TableCellNode>(),
    ]
}

/// `TableExtension`'s dependencies: `CoreImportExtension` and
/// `DOMImportExtension` with `TableImportRules`, with `html`.
#[cfg(feature = "extension")]
fn import_dependencies() -> Vec<crate::extension_core::AnyLexicalExtensionArgument> {
    #[cfg(feature = "html")]
    return crate::html::import::package_import_dependencies(table_import_rules(), Vec::new());
    #[cfg(not(feature = "html"))]
    Vec::new()
}

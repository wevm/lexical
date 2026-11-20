//! `LexicalTableExtension.ts`: `TableExtension`.

use super::import_dependencies;
use super::table_node::{
    is_scrollable_tables_active, set_scrollable_tables_active, set_sticky_scrollbar_active,
};
use super::table_nodes;
use super::table_plugin_helpers::{register_table_cell_unmerge_transform, register_table_plugin};
use super::table_selection_helpers::register_table_selection_observer;
use crate::editor::DirtyType;
use crate::extension::named_signals_config;
use crate::extension::signals::effect;
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::merge_register::merge_register;
use crate::updates::get_active_editor;
use crate::{Editor, UpdateOptions};
use std::cell::Cell;
use std::rc::Rc;

named_signals_config! {
    /// `TableConfig`.
    pub struct TableConfig;
    /// `Partial<TableConfig>`.
    pub struct TableConfigOverrides;
    /// `namedSignals(TableConfig)`: `TableExtension`'s output.
    pub struct TableOutput {
        /// `hasCellMerge`: cells may span rows and columns. Without it,
        /// [`register_table_cell_unmerge_transform`] splits merged cells.
        /// `true` by default.
        has_cell_merge: bool,
        /// `hasCellBackgroundColor`: cells may have a background color.
        /// Without it a transform clears them. `true` by default.
        has_cell_background_color: bool,
        /// `hasTabHandler`: Tab and Shift+Tab move between cells. `true` by
        /// default.
        has_tab_handler: bool,
        /// `hasHorizontalScroll`: tables render in a horizontally
        /// scrolling wrapper. `true` by default.
        has_horizontal_scroll: bool,
        /// `hasStickyScrollbar`: with `has_horizontal_scroll`, the wrapper
        /// is marked for a sticky scrollbar. The scrollbar's listeners read
        /// a rendered table and aren't ported. `false` by default.
        has_sticky_scrollbar: bool,
        /// `hasNestedTables`: a table may be inserted or pasted in a cell.
        /// `false` by default.
        has_nested_tables: bool,
    }
}

/// `TableExtension`.
pub type TableExtensionType = LexicalExtension<TableConfig, TableOutput, ()>;

/// `$fullReconcile()`, in an update of its own: re-renders every table
/// through the new scroll wrapper config without marking nodes dirty.
fn full_reconcile(editor: &Editor) {
    let _ = editor.update(
        || {
            if let Some(editor) = get_active_editor() {
                editor.0.dirty.borrow_mut().dirty_type = DirtyType::FullReconcile;
            }
            Ok(())
        },
        UpdateOptions::default(),
    );
}

fn make_table_extension() -> Rc<TableExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &TableConfig, _: &ExtensionBuildState<()>| {
                Ok(TableOutput::of(config))
            },
        )),
        // `CoreImportExtension` and `DOMImportExtension` with
        // `TableImportRules`: the HTML import, with `html`.
        dependencies: import_dependencies(),
        nodes: table_nodes(),
        register: Some(Rc::new(
            |editor: &Editor, _: &TableConfig, state: &ExtensionRegisterState<(), TableOutput>| {
                let stores = state.get_output().expect("TableExtension has a build");
                let prev_sticky = Rc::new(Cell::new(false));
                let (weak, s) = (editor.weak(), stores.clone());
                let scroll = effect(move || {
                    let has_horizontal_scroll = s.has_horizontal_scroll.value();
                    let has_sticky = s.has_sticky_scrollbar.value() && has_horizontal_scroll;
                    let editor = weak.upgrade().map(Editor)?;
                    let had_horizontal_scroll = is_scrollable_tables_active(&editor);
                    if had_horizontal_scroll != has_horizontal_scroll {
                        set_scrollable_tables_active(&editor, has_horizontal_scroll);
                    }
                    set_sticky_scrollbar_active(&editor, has_sticky);
                    if had_horizontal_scroll != has_horizontal_scroll
                        || prev_sticky.get() != has_sticky
                    {
                        full_reconcile(&editor);
                    }
                    prev_sticky.set(has_sticky);
                    None
                });
                let s = stores.clone();
                let plugin = register_table_plugin(editor, move || s.has_nested_tables.peek())?;
                let (weak, s) = (editor.weak(), stores.clone());
                let observer = effect(move || {
                    let has_tab_handler = s.has_tab_handler.value();
                    let editor = weak.upgrade().map(Editor)?;
                    Some(register_table_selection_observer(&editor, has_tab_handler))
                });
                let (weak, s) = (editor.weak(), stores.clone());
                let unmerge = effect(move || {
                    if s.has_cell_merge.value() {
                        return None;
                    }
                    let editor = weak.upgrade().map(Editor)?;
                    Some(register_table_cell_unmerge_transform(&editor))
                });
                let (weak, s) = (editor.weak(), stores);
                let background = effect(move || {
                    if s.has_cell_background_color.value() {
                        return None;
                    }
                    let editor = weak.upgrade().map(Editor)?;
                    Some(editor.register_node_transform("tablecell", |node| {
                        if node.get_background_color().is_some() {
                            node.set_background_color(None);
                        }
                        Ok(())
                    }))
                });
                Ok(merge_register(vec![
                    scroll, plugin, observer, unmerge, background,
                ]))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/table/Table",
            TableConfig {
                has_cell_background_color: true,
                has_cell_merge: true,
                has_horizontal_scroll: true,
                has_nested_tables: false,
                has_sticky_scrollbar: false,
                has_tab_handler: true,
            },
        )
    })
}

thread_local! {
    static TABLE_EXTENSION: Rc<TableExtensionType> = make_table_extension();
}

/// `TableExtension`: the table nodes, [`register_table_plugin`],
/// [`register_table_selection_observer`], and the unmerge and background
/// color transforms when `has_cell_merge` or `has_cell_background_color`
/// is off.
pub fn table_extension() -> Rc<TableExtensionType> {
    TABLE_EXTENSION.with(Rc::clone)
}

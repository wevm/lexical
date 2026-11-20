//! Ports of `packages/lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts`.

use super::support_c::{
    build_mounted_table_editor, build_table_editor, key_event, map_cell, table_with_dimensions,
};
use crate::upstream_tests::support::{mount_in_jsdom, update};
use crate::{
    COMMAND_PRIORITY_EDITOR, COMMAND_PRIORITY_HIGH, DELETE_LINE_COMMAND, Editor,
    KEY_ARROW_DOWN_COMMAND, PointType, SELECTION_CHANGE_COMMAND, Which, create_paragraph_node,
    create_range_selection, create_text_node, get_root, get_selection, set_selection,
};
use std::cell::Cell;
use std::rc::Rc;

/// A range selection from the start of the first cell to the start of the
/// second of a new 1x2 table, then SELECTION_CHANGE_COMMAND.
fn select_across_cells(editor: &Editor) -> Result<(), crate::LexicalError> {
    let table = table_with_dimensions(1, 2, true);
    get_root().clear()?;
    get_root().append(std::slice::from_ref(&table))?;
    let selection = create_range_selection();
    selection.point_set(
        Which::Anchor,
        map_cell(&table, 0, 0).get_key(),
        0,
        PointType::Element,
    )?;
    selection.point_set(
        Which::Focus,
        map_cell(&table, 0, 1).get_key(),
        0,
        PointType::Element,
    )?;
    set_selection(Some(selection));
    editor.dispatch_command(&SELECTION_CHANGE_COMMAND, ())?;
    Ok(())
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts::LexicalTableSelectionHelpers > table normalization keeps its priority order across root attachment
#[test]
fn table_normalization_keeps_its_priority_order_across_root_attachment() {
    let editor = build_table_editor("TableTestThemeExtension", Vec::new());
    let normalized = Rc::new(Cell::new(false));
    let n = normalized.clone();
    let _unregister = editor.register_command(
        &SELECTION_CHANGE_COMMAND,
        move |_, _| {
            n.set(get_selection().is_some_and(|s| s.is_table()));
            Ok(true)
        },
        COMMAND_PRIORITY_HIGH,
    );
    for _ in 0..2 {
        mount_in_jsdom(&editor);
        let (e, n) = ((*editor).clone(), normalized.clone());
        update(&editor, move || {
            select_across_cells(&e)?;
            assert!(n.get());
            Ok(())
        });
        editor.set_root_element(None).unwrap();
        normalized.set(false);
    }
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts::LexicalTableSelectionHelpers > table selection handling follows the root lifecycle
#[test]
fn table_selection_handling_follows_the_root_lifecycle() {
    let editor = build_table_editor("TableTestThemeExtension", Vec::new());
    // Explicit dispatch also exercises the rootless path, where core does
    // not automatically notify range selection changes.
    let select = |mounted: bool| {
        let e = (*editor).clone();
        update(&editor, move || {
            select_across_cells(&e)?;
            let selection = get_selection();
            assert_eq!(selection.as_ref().is_some_and(|s| s.is_table()), mounted);
            assert_eq!(selection.as_ref().is_some_and(|s| s.is_range()), !mounted);
            Ok(())
        });
    };
    // The second root element is a second `TestRootElement`.
    select(false);
    mount_in_jsdom(&editor);
    select(true);
    editor.set_root_element(None).unwrap();
    select(false);
    mount_in_jsdom(&editor);
    select(true);
    mount_in_jsdom(&editor);
    select(true);
    editor.set_root_element(None).unwrap();
    select(false);
    editor.dispose();
}

/// `$createTableBelowParagraph()`.
fn create_table_below_paragraph() -> Result<(), crate::LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node("above")])?;
    get_root().clear()?;
    get_root().append(&[paragraph.clone(), table_with_dimensions(2, 2, false)])?;
    paragraph.select_end()?;
    Ok(())
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts::LexicalTableSelectionHelpers > regression #8670 > selection change ignores the table recorded on ArrowDown when it was removed
#[test]
fn selection_change_ignores_the_table_recorded_on_arrow_down_when_it_was_removed() {
    let editor = build_mounted_table_editor("TableTestThemeExtension");
    update(&editor, create_table_below_paragraph);
    // Pressing ArrowDown above the table, where JS records the table to
    // check on the next selection change.
    editor
        .dispatch_command(&KEY_ARROW_DOWN_COMMAND, key_event("ArrowDown", false))
        .unwrap();
    // The table is removed before the next selection change occurs.
    update(&editor, || {
        let root = get_root();
        root.get_last_child_or_throw()?.remove(false)?;
        root.select_end()?;
        Ok(())
    });
    assert!(
        editor
            .dispatch_command(&SELECTION_CHANGE_COMMAND, ())
            .is_ok()
    );
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts::LexicalTableSelectionHelpers > regression #8670 > selection change self-heals observers for tables removed while the root element was detached
#[test]
fn selection_change_self_heals_observers_for_tables_removed_while_the_root_element_was_detached() {
    let editor = build_mounted_table_editor("TableTestThemeExtension");
    update(&editor, create_table_below_paragraph);
    // Removing the table while the root element is detached skips
    // reconciliation, so no destroyed mutation is reported for it.
    editor.set_root_element(None).unwrap();
    update(&editor, || {
        get_root().get_last_child_or_throw()?.remove(false)
    });
    mount_in_jsdom(&editor);
    assert!(
        editor
            .dispatch_command(&SELECTION_CHANGE_COMMAND, ())
            .is_ok()
    );
    assert!(
        editor
            .dispatch_command(&SELECTION_CHANGE_COMMAND, ())
            .is_ok()
    );
    // New tables still work.
    update(&editor, || {
        get_root().append(&[table_with_dimensions(2, 2, false)])
    });
    assert!(
        editor
            .dispatch_command(&SELECTION_CHANGE_COMMAND, ())
            .is_ok()
    );
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelectionHelpers.test.ts::LexicalTableSelectionHelpers > DELETE_LINE_COMMAND in table cells > DELETE_LINE_COMMAND propagates past the table handler in the first cell paragraph
#[test]
fn delete_line_command_propagates_past_the_table_handler_in_the_first_cell_paragraph() {
    let editor = build_mounted_table_editor("TableTestThemeExtension");
    update(&editor, || {
        let table = table_with_dimensions(2, 2, false);
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&table))?;
        let first_cell = table
            .get_first_child_or_throw()?
            .get_first_child_or_throw()?;
        let paragraph = first_cell.get_first_child_or_throw()?;
        paragraph.clear()?;
        paragraph.append(&[create_text_node("hello world")])?;
        paragraph.select_end()?;
        Ok(())
    });
    let propagated = Rc::new(Cell::new(false));
    let p = propagated.clone();
    let unregister = editor.register_command(
        &DELETE_LINE_COMMAND,
        move |_, _| {
            p.set(true);
            Ok(true)
        },
        COMMAND_PRIORITY_EDITOR,
    );
    editor.dispatch_command(&DELETE_LINE_COMMAND, true).unwrap();
    assert!(propagated.get());
    propagated.set(false);
    editor
        .dispatch_command(&DELETE_LINE_COMMAND, false)
        .unwrap();
    assert!(propagated.get());
    unregister();
    editor.dispose();
}

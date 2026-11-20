//! Ports of `packages/lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts`.

use super::support_d::{
    build_table_test_editor, cell, discrete, read_latest, table_output, text_paragraph,
};
use crate::clipboard::insert_generated_nodes;
use crate::table::{
    INSERT_TABLE_COMMAND, InsertTableCommandPayload, InsertTableCommandPayloadHeaders,
    TableCellHeaderStates, compute_table_map_skip_cell_check, create_table_cell_node,
    create_table_node, create_table_node_with_dimensions, create_table_row_node,
    create_table_selection_from, is_table_cell_node, is_table_node, is_table_row_node, merge_cells,
};
use crate::upstream_tests::support::set_test_root_element;
use crate::{
    CLICK_COMMAND, Editor, FORMAT_ELEMENT_COMMAND, KeyboardEvent, MouseEvent, NodeRef, PointType,
    SELECT_ALL_COMMAND, create_paragraph_node, create_text_node, get_root, get_selection,
    set_selection,
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// `includeHeaders: false`, as `$createTableNodeWithDimensions(r, c, false)`.
const NO_HEADERS: InsertTableCommandPayloadHeaders = InsertTableCommandPayloadHeaders::Both(false);

fn insert_table(columns: &str, rows: &str) -> InsertTableCommandPayload {
    InsertTableCommandPayload {
        columns: columns.to_owned(),
        rows: rows.to_owned(),
        include_headers: None,
    }
}

fn update(editor: &Editor, f: impl FnOnce() -> Result<(), crate::LexicalError> + 'static) {
    editor.update(f, discrete()).unwrap();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > Creates a table with INSERT_TABLE_COMMAND
#[test]
fn creates_a_table_with_insert_table_command() {
    let editor = build_table_test_editor();
    let e = editor.editor().clone();
    update(&editor, move || {
        get_root().select_end()?;
        e.dispatch_command(
            &INSERT_TABLE_COMMAND,
            InsertTableCommandPayload {
                include_headers: Some(InsertTableCommandPayloadHeaders::Both(true)),
                ..insert_table("3", "2")
            },
        )?;
        Ok(())
    });
    editor
        .read(|| {
            let children = get_root().get_children();
            let types: Vec<_> = children.iter().map(NodeRef::get_type).collect();
            assert_eq!(types, ["paragraph", "table", "paragraph"]);
            let table = &children[1];
            assert!(is_table_node(table));
            let rows = table.get_children();
            assert_eq!(rows.len(), 2);
            for row in &rows {
                assert!(is_table_row_node(row));
                let cells = row.get_children();
                assert_eq!(cells.len(), 3);
                assert!(cells.iter().all(is_table_cell_node));
            }
        })
        .unwrap();
    editor.dispose();
}

/// A root holding one table of one row of one cell, with `cell_children`
/// in the cell; returns the cell.
fn one_cell_table(cell_children: &[NodeRef]) -> NodeRef {
    let root = get_root();
    root.clear().unwrap();
    let table = create_table_node();
    let row = create_table_row_node(None);
    let cell = create_table_cell_node(0, 1.0, None);
    cell.append(cell_children).unwrap();
    row.append(std::slice::from_ref(&cell)).unwrap();
    table.append(&[row]).unwrap();
    root.append(&[table]).unwrap();
    cell
}

/// The first cell of the root's first table, checked as upstream asserts.
fn first_cell() -> NodeRef {
    let table = get_root().get_first_child().unwrap();
    assert!(is_table_node(&table), "Expected table node");
    let row = table.get_first_child().unwrap();
    assert!(is_table_row_node(&row), "Expected row node");
    let cell = row.get_first_child().unwrap();
    assert!(is_table_cell_node(&cell), "Expected cell node");
    cell
}

fn nested_table_after_insert(has_nested_tables: bool) -> bool {
    let editor = build_table_test_editor();
    if has_nested_tables {
        table_output(&editor).has_nested_tables.set_value(true);
    }
    update(&editor, || {
        one_cell_table(&[]).select(None, None)?;
        Ok(())
    });
    // Try to insert a table inside the cell
    let e = editor.editor().clone();
    update(&editor, move || {
        e.dispatch_command(&INSERT_TABLE_COMMAND, insert_table("2", "2"))?;
        Ok(())
    });
    let nested = read_latest(&editor, || {
        first_cell().get_children().iter().any(is_table_node)
    });
    editor.dispose();
    nested
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > Prevents nested tables by default
#[test]
fn prevents_nested_tables_by_default() {
    // Verify no nested table was created
    assert!(!nested_table_after_insert(false));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > Allows nested tables when hasNestedTables is true
#[test]
fn allows_nested_tables_when_has_nested_tables_is_true() {
    // Verify nested table was created
    assert!(nested_table_after_insert(true));
}

/// Pastes a whole table (a table and a paragraph: "whole table paste, as
/// opposed to a table merge, is done by having multiple nodes on the
/// clipboard") into a one-cell table's paragraph.
fn nested_table_after_whole_table_paste(has_nested_tables: bool) -> bool {
    let editor = build_table_test_editor();
    if has_nested_tables {
        table_output(&editor).has_nested_tables.set_value(true);
    }
    update(&editor, || {
        let paragraph = create_paragraph_node();
        one_cell_table(std::slice::from_ref(&paragraph));
        paragraph.select(None, None)?;
        Ok(())
    });
    let e = editor.editor().clone();
    update(&editor, move || {
        let table_node = create_table_node();
        let row = create_table_row_node(None);
        row.append(&[cell(0, &[create_paragraph_node()])])?;
        table_node.append(&[row])?;
        let selection = get_selection().filter(|s| s.is_range());
        let selection = selection.expect("Expected range selection");
        insert_generated_nodes(&e, vec![table_node, create_paragraph_node()], &selection)
    });
    let nested = read_latest(&editor, || {
        first_cell().get_children().iter().any(is_table_node)
    });
    editor.dispose();
    nested
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $insertGeneratedNodes > SELECTION_INSERT_CLIPBOARD_NODES_COMMAND handler prevents pasting whole table into cells by default
#[test]
fn selection_insert_clipboard_nodes_command_handler_prevents_pasting_whole_table_into_cells_by_default()
 {
    // Verify no nested table was created
    assert!(!nested_table_after_whole_table_paste(false));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $insertGeneratedNodes > SELECTION_INSERT_CLIPBOARD_NODES_COMMAND handler allows pasting whole table into a single cell when hasNestedTables is true
#[test]
fn selection_insert_clipboard_nodes_command_handler_allows_pasting_whole_table_into_a_single_cell_when_has_nested_tables_is_true()
 {
    // Verify a nested table was created
    assert!(nested_table_after_whole_table_paste(true));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $insertGeneratedNodes > SELECTION_INSERT_CLIPBOARD_NODES_COMMAND handler allows extending table when hasNestedTables is true
#[test]
fn selection_insert_clipboard_nodes_command_handler_allows_extending_table_when_has_nested_tables_is_true()
 {
    let editor = build_table_test_editor();
    table_output(&editor).has_nested_tables.set_value(true);
    update(&editor, || {
        let paragraph = create_paragraph_node();
        one_cell_table(std::slice::from_ref(&paragraph));
        paragraph.select(None, None)?;
        Ok(())
    });
    // Try to paste a wider table inside the cell
    let e = editor.editor().clone();
    update(&editor, move || {
        let table_node = create_table_node();
        let row = create_table_row_node(None);
        row.append(&[
            cell(0, &[text_paragraph("a")]),
            cell(0, &[text_paragraph("b")]),
        ])?;
        table_node.append(&[row])?;
        let selection = get_selection().expect("Expected valid selection");
        insert_generated_nodes(&e, vec![table_node], &selection)
    });
    // Verify the table was extended
    read_latest(&editor, || {
        let table = get_root().get_first_child().unwrap();
        assert!(is_table_node(&table), "Expected table node");
        let row = table.get_first_child().unwrap();
        assert!(row.is_element(), "Expected row node");
        let cells = row.get_children();
        assert_eq!(cells.len(), 2, "Expected 2 children in row");
        for (cell, text) in cells.iter().zip(["a", "b"]) {
            assert!(is_table_cell_node(cell), "Expected cell to be a cell node");
            let child = cell.get_first_child().unwrap();
            assert_eq!(child.get_type(), "paragraph");
            assert_eq!(child.get_text_content(), text);
        }
    });
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $insertGeneratedNodes > SELECTION_INSERT_CLIPBOARD_NODES_COMMAND clips to selection boundary with TableSelection
#[test]
fn selection_insert_clipboard_nodes_command_clips_to_selection_boundary_with_table_selection() {
    let editor = build_table_test_editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let table = create_table_node();
        for r in 0..2 {
            let row = create_table_row_node(None);
            for c in 0..2 {
                row.append(&[cell(0, &[text_paragraph(&format!("{c},{r}"))])])?;
            }
            table.append(&[row])?;
        }
        root.append(std::slice::from_ref(&table))?;
        let (table_map, _, _) = compute_table_map_skip_cell_check(&table, None, None)?;
        let anchor = &table_map[0][0].as_ref().unwrap().cell;
        let focus = &table_map[1][1].as_ref().unwrap().cell;
        set_selection(Some(create_table_selection_from(&table, anchor, focus)?));
        Ok(())
    });
    let e = editor.editor().clone();
    update(&editor, move || {
        let template = create_table_node();
        for r in 0..3u8 {
            let row = create_table_row_node(None);
            for c in 0..3u8 {
                let letter = char::from(65 + r * 3 + c).to_string();
                row.append(&[cell(0, &[text_paragraph(&letter)])])?;
            }
            template.append(&[row])?;
        }
        let selection = get_selection().filter(|s| s.is_table());
        let selection = selection.expect("Expected table selection");
        insert_generated_nodes(&e, vec![template], &selection)
    });
    read_latest(&editor, || {
        let table = get_root().get_first_child().unwrap();
        assert!(is_table_node(&table), "Expected table node");
        let rows: Vec<_> = table
            .get_children()
            .into_iter()
            .filter(is_table_row_node)
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get_children().len(), 2);
        let texts: Vec<Vec<String>> = rows
            .iter()
            .map(|row| {
                row.get_children()
                    .iter()
                    .map(NodeRef::get_text_content)
                    .collect()
            })
            .collect();
        assert_eq!(texts, [["A", "B"], ["D", "E"]]);
    });
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $insertGeneratedNodes > pasting a table carries the cell backgroundColor and verticalAlign
#[test]
fn pasting_a_table_carries_the_cell_background_color_and_vertical_align() {
    let editor = build_table_test_editor();
    update(&editor, || {
        one_cell_table(&[text_paragraph("old")]).select_start()?;
        Ok(())
    });
    let e = editor.editor().clone();
    update(&editor, move || {
        let template = create_table_node();
        let row = create_table_row_node(None);
        let cell = create_table_cell_node(0, 1.0, None);
        cell.set_background_color(Some("rgb(255, 0, 0)"));
        cell.set_vertical_align(Some("middle"));
        cell.append(&[text_paragraph("new")])?;
        row.append(&[cell])?;
        template.append(&[row])?;
        let selection = get_selection().expect("Expected a selection");
        insert_generated_nodes(&e, vec![template], &selection)
    });
    read_latest(&editor, || {
        let cell = first_cell();
        assert_eq!(cell.get_text_content(), "new");
        assert_eq!(
            cell.get_background_color().as_deref(),
            Some("rgb(255, 0, 0)")
        );
        assert_eq!(cell.get_vertical_align().as_deref(), Some("middle"));
    });
    editor.dispose();
}

/// The tags of the root table's cells, by row.
fn cell_tags() -> Vec<Vec<&'static str>> {
    let table = get_root().get_first_child().unwrap();
    assert!(is_table_node(&table));
    table
        .get_children()
        .iter()
        .filter(|row| is_table_row_node(row))
        .map(|row| {
            row.get_children()
                .iter()
                .filter_map(NodeRef::get_table_cell_tag)
                .collect()
        })
        .collect()
}

/// A table of a header row of `header` cells and a body row of `body`
/// cells.
fn padding_table(
    header: &'static [&'static str],
    body: &'static [&'static str],
) -> Vec<Vec<&'static str>> {
    let editor = build_table_test_editor();
    update(&editor, move || {
        let root = get_root();
        root.clear()?;
        let table = create_table_node();
        let header_row = create_table_row_node(None);
        for text in header {
            header_row.append(&[cell(TableCellHeaderStates::ROW, &[text_paragraph(text)])])?;
        }
        let body_row = create_table_row_node(None);
        for text in body {
            body_row.append(&[cell(0, &[text_paragraph(text)])])?;
        }
        table.append(&[header_row, body_row])?;
        root.append(&[table])?;
        Ok(())
    });
    let tags = read_latest(&editor, cell_tags);
    editor.dispose();
    tags
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $tableTransform padding > a short header row is padded with header cells
#[test]
fn a_short_header_row_is_padded_with_header_cells() {
    // First row is a header row but is one cell short.
    assert_eq!(
        padding_table(&["h"], &["a", "b"]),
        [["th", "th"], ["td", "td"]]
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > $tableTransform padding > a short body row is padded with body cells
#[test]
fn a_short_body_row_is_padded_with_body_cells() {
    assert_eq!(
        padding_table(&["h1", "h2"], &["a"]),
        [["th", "th"], ["td", "td"]]
    );
}

/// Inserts a table of `columns` columns and sets its `colWidths`; returns
/// them once the transforms ran.
fn col_widths_after(columns: &'static str, col_widths: Vec<f64>) -> Option<Vec<f64>> {
    let editor = build_table_test_editor();
    let e = editor.editor().clone();
    update(&editor, move || {
        let root = get_root();
        root.clear()?;
        root.select_start()?;
        e.dispatch_command(&INSERT_TABLE_COMMAND, insert_table(columns, "2"))?;
        let table = root.get_first_child().filter(is_table_node).unwrap();
        table.set_col_widths(Some(col_widths));
        Ok(())
    });
    let widths = read_latest(&editor, || {
        let table = get_root().get_first_child().filter(is_table_node).unwrap();
        table.get_col_widths()
    });
    editor.dispose();
    widths
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > colWidths > removes colWidths if it is an empty array
#[test]
fn removes_col_widths_if_it_is_an_empty_array() {
    assert_eq!(col_widths_after("2", vec![]), None);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > colWidths > uses the last column width if the column count is greater than the number of column widths
#[test]
fn uses_the_last_column_width_if_the_column_count_is_greater_than_the_number_of_column_widths() {
    assert_eq!(
        col_widths_after("3", vec![10.0, 20.0]),
        Some(vec![10.0, 20.0, 20.0])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > colWidths > shortens the colWidths if the column count is less than the number of column widths
#[test]
fn shortens_the_col_widths_if_the_column_count_is_less_than_the_number_of_column_widths() {
    assert_eq!(
        col_widths_after("2", vec![10.0, 20.0, 30.0]),
        Some(vec![10.0, 20.0])
    );
}

/// The first paragraph of the first cell of `table`.
fn first_cell_paragraph(table: &NodeRef) -> NodeRef {
    let first_row = table.get_first_child().unwrap();
    assert!(is_table_row_node(&first_row), "Expected first row");
    let first_cell = first_row.get_first_child().unwrap();
    assert!(is_table_cell_node(&first_cell), "Expected first cell");
    let paragraph = first_cell.get_first_child().unwrap();
    assert!(paragraph.is_element(), "Expected paragraph in cell");
    paragraph
}

fn dispatch_select_all(editor: &Editor) {
    let e = editor.clone();
    update(editor, move || {
        e.dispatch_command(&SELECT_ALL_COMMAND, Rc::new(KeyboardEvent::default()))?;
        Ok(())
    });
}

/// After a select all in a table that is the root's only content: the
/// selection is a table selection from the first cell of the map to its
/// last, and its nodes hold every cell of the table once.
fn check_select_all_selects_every_cell(editor: &Editor) {
    read_latest(editor, || {
        let selection = get_selection().filter(|s| s.is_table());
        let selection = selection.expect("Expected TableSelection after Ctrl+A");
        let table = get_root().get_first_child().unwrap();
        assert!(is_table_node(&table), "Expected table node");
        // Verify all cells are selected by checking the selection spans the
        // entire table
        let (table_map, _, _) = compute_table_map_skip_cell_check(&table, None, None).unwrap();
        let first_cell_map = table_map[0][0].as_ref().expect("Expected first cell");
        let last_row = table_map.last().unwrap();
        let last_cell_map = last_row
            .last()
            .unwrap()
            .as_ref()
            .expect("Expected last cell");
        let (anchor, focus) = selection.get_start_end_points().unwrap();
        let anchor_cell = anchor.get_node().unwrap();
        let focus_cell = focus.get_node().unwrap();
        assert!(
            is_table_cell_node(&anchor_cell),
            "Expected anchor to be in a table cell"
        );
        assert!(
            is_table_cell_node(&focus_cell),
            "Expected focus to be in a table cell"
        );
        // Verify selection spans from first to last cell
        assert_eq!(anchor_cell.get_key(), first_cell_map.cell.get_key());
        assert_eq!(focus_cell.get_key(), last_cell_map.cell.get_key());
        // Verify all unique cells are in the selection (merged cells should
        // appear once)
        let selected: Vec<_> = selection
            .get_nodes()
            .unwrap()
            .into_iter()
            .filter(is_table_cell_node)
            .collect();
        let unique: HashSet<_> = selected.iter().map(|n| n.get_key().to_owned()).collect();
        let total: HashSet<_> = table_map
            .iter()
            .flatten()
            .flatten()
            .map(|m| m.cell.get_key().to_owned())
            .collect();
        assert_eq!(selected.len(), total.len());
        assert_eq!(unique.len(), total.len());
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > SELECT_ALL_COMMAND > Selects all cells in table without merged cells when table is only content
#[test]
fn selects_all_cells_in_table_without_merged_cells_when_table_is_only_content() {
    let editor = build_table_test_editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        // Create a 3x3 table
        let table = create_table_node_with_dimensions(3, 3, NO_HEADERS)?;
        root.append(std::slice::from_ref(&table))?;
        // Place cursor in the first cell
        let paragraph = first_cell_paragraph(&table);
        assert_eq!(
            paragraph.get_type(),
            "paragraph",
            "Expected paragraph in cell"
        );
        paragraph.select_start()?;
        Ok(())
    });
    dispatch_select_all(&editor);
    check_select_all_selects_every_cell(&editor);
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > SELECT_ALL_COMMAND > Selects all cells in table with merged cells when table is only content
#[test]
fn selects_all_cells_in_table_with_merged_cells_when_table_is_only_content() {
    let editor = build_table_test_editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        // Create a 3x3 table
        let table = create_table_node_with_dimensions(3, 3, NO_HEADERS)?;
        root.append(std::slice::from_ref(&table))?;
        // Merge two cells in the last column (merge cells in row 0 and row
        // 1, column 2)
        let row0 = table
            .get_child_at_index(0)
            .filter(is_table_row_node)
            .unwrap();
        let row1 = table
            .get_child_at_index(1)
            .filter(is_table_row_node)
            .unwrap();
        let cell0_2 = row0
            .get_child_at_index(2)
            .filter(is_table_cell_node)
            .unwrap();
        let cell1_2 = row1
            .get_child_at_index(2)
            .filter(is_table_cell_node)
            .unwrap();
        // Merge the cells
        merge_cells(&[cell0_2, cell1_2])?;
        // Place cursor in any cell (not the merged one)
        first_cell_paragraph(&table).select_start()?;
        Ok(())
    });
    dispatch_select_all(&editor);
    check_select_all_selects_every_cell(&editor);
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > SELECT_ALL_COMMAND > Does not intercept SELECT_ALL_COMMAND when cursor is outside table
#[test]
fn does_not_intercept_select_all_command_when_cursor_is_outside_table() {
    let editor = build_table_test_editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        let text = create_text_node("Some text");
        paragraph.append(std::slice::from_ref(&text))?;
        root.append(&[paragraph])?;
        // Add a table after the paragraph
        root.append(&[create_table_node_with_dimensions(2, 2, NO_HEADERS)?])?;
        // Place cursor in the paragraph (outside table)
        text.select_start()?;
        Ok(())
    });
    dispatch_select_all(&editor);
    // Verify RangeSelection was created (not TableSelection) since cursor
    // is outside table
    read_latest(&editor, || {
        let selection = get_selection().unwrap();
        assert!(
            selection.is_range(),
            "Expected RangeSelection when cursor is outside table"
        );
        assert!(!selection.is_table());
    });
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > SELECT_ALL_COMMAND > Does not intercept SELECT_ALL_COMMAND when there is paragraph after table
#[test]
fn does_not_intercept_select_all_command_when_there_is_paragraph_after_table() {
    let editor = build_table_test_editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        // Create a table
        let table = create_table_node_with_dimensions(2, 2, NO_HEADERS)?;
        root.append(std::slice::from_ref(&table))?;
        // Add a paragraph after the table (simulating insertTable behavior)
        root.append(&[create_paragraph_node()])?;
        // Place cursor in the table
        first_cell_paragraph(&table).select_start()?;
        Ok(())
    });
    dispatch_select_all(&editor);
    // Verify RangeSelection was created (not TableSelection) since there's
    // paragraph after table
    read_latest(&editor, || {
        let selection = get_selection().unwrap();
        assert!(
            selection.is_range(),
            "Expected RangeSelection when paragraph exists after table"
        );
        assert!(!selection.is_table());
    });
    editor.dispose();
}

// The click is the app's CLICK_COMMAND with the key of the node under the
// pointer (`MouseEvent.target_key`), where upstream dispatches a DOM click
// on the rendered text.
// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > triple click in a cell > selects the paragraph with text points, as a native selection of it resolves
#[test]
fn selects_the_paragraph_with_text_points_as_a_native_selection_of_it_resolves() {
    let editor = build_table_test_editor();
    set_test_root_element(&editor);
    let keys = Rc::new(RefCell::new((String::new(), String::new())));
    let k = keys.clone();
    update(&editor, move || {
        let table = create_table_node_with_dimensions(1, 2, NO_HEADERS)?;
        let root = get_root();
        root.clear()?;
        root.append(std::slice::from_ref(&table))?;
        let row = table.get_first_child().filter(is_table_row_node).unwrap();
        let cell = row.get_last_child().filter(is_table_cell_node).unwrap();
        let paragraph = cell.get_first_child().unwrap();
        assert_eq!(paragraph.get_type(), "paragraph");
        let first = create_text_node("Foo");
        let last = create_text_node("Bar");
        last.toggle_format("bold");
        paragraph.append(&[first.clone(), last.clone()])?;
        *k.borrow_mut() = (first.get_key().to_owned(), last.get_key().to_owned());
        Ok(())
    });
    let (first_key, last_key) = keys.borrow().clone();
    // The third click's click event, with detail 3.
    let mut event = MouseEvent::new("click");
    event.detail = 3;
    event.target_key = Some(last_key.clone());
    editor
        .dispatch_command(&CLICK_COMMAND, Rc::new(event))
        .unwrap();
    editor
        .read(|| {
            let selection = get_selection().and_then(|s| s.range());
            let selection = selection.expect("Expected a RangeSelection");
            let (anchor, focus) = (&selection.anchor, &selection.focus);
            assert_eq!(
                (anchor.key.as_str(), anchor.offset, anchor.point_type),
                (first_key.as_str(), 0, PointType::Text)
            );
            assert_eq!(
                (focus.key.as_str(), focus.offset, focus.point_type),
                (last_key.as_str(), 3, PointType::Text)
            );
        })
        .unwrap();
    editor.dispose();
}

/// `$getCell(table, row, column)`.
fn get_cell(table: &NodeRef, row: usize, column: usize) -> NodeRef {
    let row_node = table
        .get_child_at_index(row)
        .filter(is_table_row_node)
        .unwrap();
    row_node
        .get_child_at_index(column)
        .filter(is_table_cell_node)
        .unwrap()
}

/// `createTable()`: the root holds a 3x3 table only, on an editor with a
/// root element.
fn create_table(editor: &Editor) -> NodeRef {
    update(editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[create_table_node_with_dimensions(3, 3, NO_HEADERS)?])?;
        Ok(())
    });
    editor
        .read(|| get_root().get_first_child().filter(is_table_node).unwrap())
        .unwrap()
}

fn select_and_format(
    editor: &Editor,
    table: &NodeRef,
    anchor: (usize, usize),
    focus: (usize, usize),
    format: &str,
) {
    let (e, table, format) = (editor.clone(), table.clone(), format.to_owned());
    update(editor, move || {
        let anchor_cell = get_cell(&table, anchor.0, anchor.1);
        let focus_cell = get_cell(&table, focus.0, focus.1);
        set_selection(Some(create_table_selection_from(
            &table,
            &anchor_cell,
            &focus_cell,
        )?));
        e.dispatch_command(&FORMAT_ELEMENT_COMMAND, format)?;
        Ok(())
    });
}

fn paragraph_format(table: &NodeRef, row: usize, column: usize) -> &'static str {
    let paragraph = get_cell(table, row, column).get_first_child().unwrap();
    assert_eq!(paragraph.get_type(), "paragraph");
    paragraph.element_data().get_format_type()
}

fn aligns_the_table_when_selected(anchor: (usize, usize), focus: (usize, usize)) {
    let editor = build_table_test_editor();
    set_test_root_element(&editor);
    let table = create_table(&editor);
    select_and_format(&editor, &table, anchor, focus, "center");
    editor
        .read(|| {
            assert_eq!(table.element_data().get_format_type(), "center");
            for row in 0..3 {
                for column in 0..3 {
                    assert_eq!(paragraph_format(&table, row, column), "");
                }
            }
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > aligns the table when selected top-left -> bottom-right
#[test]
fn aligns_the_table_when_selected_top_left_to_bottom_right() {
    aligns_the_table_when_selected((0, 0), (2, 2));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > aligns the table when selected bottom-right -> top-left
#[test]
fn aligns_the_table_when_selected_bottom_right_to_top_left() {
    aligns_the_table_when_selected((2, 2), (0, 0));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > aligns the table when selected top-right -> bottom-left
#[test]
fn aligns_the_table_when_selected_top_right_to_bottom_left() {
    aligns_the_table_when_selected((0, 2), (2, 0));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > aligns the table when selected bottom-left -> top-right
#[test]
fn aligns_the_table_when_selected_bottom_left_to_top_right() {
    aligns_the_table_when_selected((2, 0), (0, 2));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > aligns the table when the full selection includes a merged cell, selected in reverse
#[test]
fn aligns_the_table_when_the_full_selection_includes_a_merged_cell_selected_in_reverse() {
    let editor = build_table_test_editor();
    set_test_root_element(&editor);
    let table = create_table(&editor);
    let t = table.clone();
    update(&editor, move || {
        merge_cells(&[get_cell(&t, 0, 2), get_cell(&t, 1, 2)])?;
        Ok(())
    });
    select_and_format(&editor, &table, (2, 2), (0, 0), "justify");
    editor
        .read(|| assert_eq!(table.element_data().get_format_type(), "justify"))
        .unwrap();
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableExtension.test.ts::TableExtension > FORMAT_ELEMENT_COMMAND on a full table selection (#8880) > applies per-cell alignment (not table alignment) when only part of the table is selected, even in reverse direction
#[test]
fn applies_per_cell_alignment_not_table_alignment_when_only_part_of_the_table_is_selected_even_in_reverse_direction()
 {
    let editor = build_table_test_editor();
    set_test_root_element(&editor);
    let table = create_table(&editor);
    select_and_format(&editor, &table, (1, 1), (0, 0), "right");
    editor
        .read(|| {
            assert_eq!(table.element_data().get_format_type(), "");
            for row in 0..3 {
                for column in 0..3 {
                    let in_selected_subset = row <= 1 && column <= 1;
                    assert_eq!(
                        paragraph_format(&table, row, column),
                        if in_selected_subset { "right" } else { "" }
                    );
                }
            }
        })
        .unwrap();
    editor.dispose();
}

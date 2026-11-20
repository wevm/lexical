//! Ports of `packages/lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts`.

use super::support_a::{
    build_table_editor, cell_texts, cells_of, empty_cell, first_table, grid, read, read_latest,
    row, table, table_of, text_cell, text_paragraph, update,
};
use crate::extension::LexicalEditorWithDispose;
use crate::table::{
    TableCellHeaderStates, create_default_table_cell_node, insert_table_column_at_node,
    merge_cells, move_table_column, move_table_row, set_table_column_is_header,
    set_table_row_is_header, unmerge_cell_node,
};
use crate::{NodeRef, create_paragraph_node, get_root};

const NO_STATUS: i32 = TableCellHeaderStates::NO_STATUS;
const ROW: i32 = TableCellHeaderStates::ROW;
const COLUMN: i32 = TableCellHeaderStates::COLUMN;
const BOTH: i32 = TableCellHeaderStates::BOTH;

/// The `beforeEach` editor: `TableExtension`, an empty root.
fn build_editor() -> LexicalEditorWithDispose {
    let editor = build_table_editor("LexicalTableUtils-test");
    update(&editor, || get_root().clear());
    editor
}

/// `$createTestTable(rows, columns)`: cells holding `r{r}c{c}`.
fn create_test_table(rows: usize, columns: usize) -> NodeRef {
    table_of(
        (0..rows)
            .map(|r| {
                (0..columns)
                    .map(|c| text_cell(&format!("r{r}c{c}")))
                    .collect()
            })
            .collect(),
    )
}

fn strings(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|cells| cells.iter().map(|s| (*s).to_owned()).collect())
        .collect()
}

/// `$getHeaderStates(table, flag)`.
fn header_states(table: &NodeRef, flag: i32) -> Vec<Vec<bool>> {
    cells_of(table)
        .iter()
        .map(|cells| cells.iter().map(|c| c.has_header_state(flag)).collect())
        .collect()
}

/// A cell of `text` with the given spans.
fn span_cell(text: &str, row_span: f64, col_span: f64) -> NodeRef {
    let cell = text_cell(text);
    cell.set_row_span(row_span);
    cell.set_col_span(col_span);
    cell
}

/// Appends `table`, then runs `f` on the root's table in a second update,
/// and returns the cell texts it leaves, read with `'latest'`.
fn after(
    make: impl FnOnce() -> NodeRef + 'static,
    f: impl FnOnce(&NodeRef) -> Result<(), crate::LexicalError> + 'static,
) -> (LexicalEditorWithDispose, Vec<Vec<String>>) {
    let editor = build_editor();
    update(&editor, || get_root().append(&[make()]));
    update(&editor, move || f(&first_table()));
    let texts = read_latest(&editor, || cell_texts(&first_table()));
    (editor, texts)
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$unmergeCellNode > restores cells to the correct rows below the first row (#4876)
#[test]
fn restores_cells_to_the_correct_rows_below_the_first_row_4876() {
    let editor = build_editor();
    update(&editor, || get_root().append(&[create_test_table(4, 4)]));
    update(&editor, || {
        let cells: Vec<NodeRef> = cells_of(&first_table())
            .into_iter()
            .skip(1)
            .flat_map(|cells| cells.into_iter().take(2))
            .collect();
        merge_cells(&cells).map(|_| ())
    });
    update(&editor, || {
        let merged = first_table()
            .get_child_at_index(1)
            .unwrap()
            .get_first_child()
            .unwrap();
        assert_eq!(merged.get_col_span(), 2.0);
        assert_eq!(merged.get_row_span(), 3.0);
        unmerge_cell_node(&merged)
    });
    read(&editor, || {
        let table = first_table();
        assert_eq!(
            cell_texts(&table),
            strings(&[
                &["r0c0", "r0c1", "r0c2", "r0c3"],
                &[
                    "r1c0\n\nr1c1\n\nr2c0\n\nr2c1\n\nr3c0\n\nr3c1",
                    "",
                    "r1c2",
                    "r1c3"
                ],
                &["", "", "r2c2", "r2c3"],
                &["", "", "r3c2", "r3c3"],
            ])
        );
        for cell in cells_of(&table).iter().flatten() {
            assert_eq!(cell.get_col_span(), 1.0);
            assert_eq!(cell.get_row_span(), 1.0);
        }
    });
    editor.dispose();
}

fn moved_column(rows: usize, columns: usize, origin: usize, target: usize) -> Vec<Vec<String>> {
    let (editor, texts) = after(
        move || create_test_table(rows, columns),
        move |t| move_table_column(t, origin, target),
    );
    editor.dispose();
    texts
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > moves a column forward
#[test]
fn moves_a_column_forward() {
    assert_eq!(
        moved_column(2, 4, 0, 2),
        strings(&[
            &["r0c1", "r0c2", "r0c0", "r0c3"],
            &["r1c1", "r1c2", "r1c0", "r1c3"],
        ])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > moves a column backward
#[test]
fn moves_a_column_backward() {
    assert_eq!(
        moved_column(2, 4, 3, 1),
        strings(&[
            &["r0c0", "r0c3", "r0c1", "r0c2"],
            &["r1c0", "r1c3", "r1c1", "r1c2"],
        ])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > moves a column to the first position
#[test]
fn moves_a_column_to_the_first_position() {
    assert_eq!(
        moved_column(2, 3, 2, 0),
        strings(&[&["r0c2", "r0c0", "r0c1"], &["r1c2", "r1c0", "r1c1"]])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > moves a column to the last position
#[test]
fn moves_a_column_to_the_last_position() {
    assert_eq!(
        moved_column(2, 3, 0, 2),
        strings(&[&["r0c1", "r0c2", "r0c0"], &["r1c1", "r1c2", "r1c0"]])
    );
}

const UNMOVED_2X3: &[&[&str]] = &[&["r0c0", "r0c1", "r0c2"], &["r1c0", "r1c1", "r1c2"]];

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > is a no-op when origin equals target
#[test]
fn move_table_column_is_a_no_op_when_origin_equals_target() {
    assert_eq!(moved_column(2, 3, 1, 1), strings(UNMOVED_2X3));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > is a no-op when origin is out of bounds
#[test]
fn move_table_column_is_a_no_op_when_origin_is_out_of_bounds() {
    assert_eq!(moved_column(2, 3, 5, 0), strings(UNMOVED_2X3));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > is a no-op when target is out of bounds
#[test]
fn move_table_column_is_a_no_op_when_target_is_out_of_bounds() {
    assert_eq!(moved_column(2, 3, 0, 10), strings(UNMOVED_2X3));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > reorders colWidths when present
#[test]
fn reorders_col_widths_when_present() {
    let editor = build_editor();
    update(&editor, || {
        let table = create_test_table(2, 4);
        table.set_col_widths(Some(vec![100.0, 200.0, 300.0, 400.0]));
        get_root().append(&[table])
    });
    update(&editor, || move_table_column(&first_table(), 0, 2));
    read_latest(&editor, || {
        assert_eq!(
            first_table().get_col_widths(),
            Some(vec![200.0, 300.0, 100.0, 400.0])
        );
    });
    editor.dispose();
}

/// Row 0: a cell spanning 2 columns, then a normal cell; row 1: 3 normal
/// cells.
fn merged_table() -> NodeRef {
    let merged = text_cell("merged");
    merged.set_col_span(2.0);
    table_of(vec![
        vec![merged, text_cell("r0c2")],
        (0..3).map(|c| text_cell(&format!("r1c{c}"))).collect(),
    ])
}

/// Should be unchanged because table has merged cells.
fn assert_merged_table_unchanged(editor: &LexicalEditorWithDispose) {
    read_latest(editor, || {
        let first_row_cells = first_table().get_first_child().unwrap().get_children();
        assert_eq!(first_row_cells.len(), 2); // merged cell + normal cell
        assert_eq!(first_row_cells[0].get_col_span(), 2.0);
        assert_eq!(first_row_cells[0].get_text_content(), "merged");
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > does not modify table with merged cells
#[test]
fn move_table_column_does_not_modify_table_with_merged_cells() {
    let (editor, _) = after(merged_table, |t| move_table_column(t, 0, 1));
    assert_merged_table_unchanged(&editor);
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > swaps adjacent columns
#[test]
fn swaps_adjacent_columns() {
    assert_eq!(
        moved_column(3, 2, 0, 1),
        strings(&[&["r0c1", "r0c0"], &["r1c1", "r1c0"], &["r2c1", "r2c0"]])
    );
}

/// The number of rows and the children of each.
fn row_sizes(editor: &LexicalEditorWithDispose) -> Vec<usize> {
    read_latest(editor, || {
        first_table()
            .get_children()
            .iter()
            .map(NodeRef::get_children_size)
            .collect()
    })
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableColumn > preserves table structure after move
#[test]
fn move_table_column_preserves_table_structure_after_move() {
    let (editor, _) = after(|| create_test_table(3, 4), |t| move_table_column(t, 1, 3));
    // Verify row and column count is preserved
    assert_eq!(row_sizes(&editor), [4, 4, 4]);
    editor.dispose();
}

fn moved_row(rows: usize, columns: usize, origin: usize, target: usize) -> Vec<Vec<String>> {
    let (editor, texts) = after(
        move || create_test_table(rows, columns),
        move |t| move_table_row(t, origin, target),
    );
    editor.dispose();
    texts
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > moves a row forward
#[test]
fn moves_a_row_forward() {
    assert_eq!(
        moved_row(4, 2, 0, 2),
        strings(&[
            &["r1c0", "r1c1"],
            &["r2c0", "r2c1"],
            &["r0c0", "r0c1"],
            &["r3c0", "r3c1"],
        ])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > moves a row backward
#[test]
fn moves_a_row_backward() {
    assert_eq!(
        moved_row(4, 2, 3, 1),
        strings(&[
            &["r0c0", "r0c1"],
            &["r3c0", "r3c1"],
            &["r1c0", "r1c1"],
            &["r2c0", "r2c1"],
        ])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > moves a row to the first position
#[test]
fn moves_a_row_to_the_first_position() {
    assert_eq!(
        moved_row(4, 2, 2, 0),
        strings(&[
            &["r2c0", "r2c1"],
            &["r0c0", "r0c1"],
            &["r1c0", "r1c1"],
            &["r3c0", "r3c1"],
        ])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > moves a row to the last position
#[test]
fn moves_a_row_to_the_last_position() {
    assert_eq!(
        moved_row(4, 2, 0, 3),
        strings(&[
            &["r1c0", "r1c1"],
            &["r2c0", "r2c1"],
            &["r3c0", "r3c1"],
            &["r0c0", "r0c1"],
        ])
    );
}

const UNMOVED_3X2: &[&[&str]] = &[&["r0c0", "r0c1"], &["r1c0", "r1c1"], &["r2c0", "r2c1"]];

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > is a no-op when origin equals target
#[test]
fn move_table_row_is_a_no_op_when_origin_equals_target() {
    assert_eq!(moved_row(3, 2, 1, 1), strings(UNMOVED_3X2));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > is a no-op when origin is out of bounds
#[test]
fn move_table_row_is_a_no_op_when_origin_is_out_of_bounds() {
    assert_eq!(moved_row(3, 2, 3, 0), strings(UNMOVED_3X2));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > is a no-op when target is out of bounds
#[test]
fn move_table_row_is_a_no_op_when_target_is_out_of_bounds() {
    assert_eq!(moved_row(3, 2, 0, 3), strings(UNMOVED_3X2));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > does not modify table with merged cells
#[test]
fn move_table_row_does_not_modify_table_with_merged_cells() {
    let (editor, _) = after(merged_table, |t| move_table_row(t, 0, 1));
    assert_merged_table_unchanged(&editor);
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > swaps adjacent rows
#[test]
fn swaps_adjacent_rows() {
    assert_eq!(
        moved_row(2, 3, 0, 1),
        strings(&[&["r1c0", "r1c1", "r1c2"], &["r0c0", "r0c1", "r0c2"]])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > moves header cells along with the row
#[test]
fn moves_header_cells_along_with_the_row() {
    let (editor, texts) = after(
        || {
            table_of(
                (0..3)
                    .map(|r| {
                        (0..2)
                            .map(|c| {
                                let header = if r == 0 { ROW } else { NO_STATUS };
                                super::support_a::cell_with(
                                    header,
                                    &[text_paragraph(&format!("r{r}c{c}"))],
                                )
                            })
                            .collect()
                    })
                    .collect(),
            )
        },
        |t| move_table_row(t, 0, 2),
    );
    assert_eq!(
        texts,
        strings(&[&["r1c0", "r1c1"], &["r2c0", "r2c1"], &["r0c0", "r0c1"]])
    );
    read_latest(&editor, || {
        for cell in &cells_of(&first_table())[2] {
            assert_eq!(cell.get_header_styles(), ROW);
        }
    });
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$moveTableRow > preserves table structure after move
#[test]
fn move_table_row_preserves_table_structure_after_move() {
    let (editor, _) = after(|| create_test_table(4, 3), |t| move_table_row(t, 1, 3));
    // Verify row and column count is preserved
    assert_eq!(row_sizes(&editor), [3, 3, 3, 3]);
    editor.dispose();
}

/// Appends `make()`, runs `f` on the table, and returns the header states
/// for each flag.
fn headers_after(
    make: impl FnOnce() -> NodeRef + 'static,
    f: impl FnOnce(&NodeRef) -> Result<(), crate::LexicalError> + 'static,
) -> (Vec<Vec<bool>>, Vec<Vec<bool>>) {
    let (editor, _) = after(make, f);
    let states = read_latest(&editor, || {
        let table = first_table();
        (header_states(&table, ROW), header_states(&table, COLUMN))
    });
    editor.dispose();
    states
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > sets a row as header
#[test]
fn sets_a_row_as_header() {
    let (rows, _) = headers_after(
        || create_test_table(3, 3),
        |t| set_table_row_is_header(t, 0, true),
    );
    assert_eq!(
        rows,
        [
            [true, true, true],
            [false, false, false],
            [false, false, false]
        ]
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > clears a header row
#[test]
fn clears_a_header_row() {
    let (rows, _) = headers_after(
        || {
            table_of(vec![
                (0..3)
                    .map(|c| super::support_a::cell_with(ROW, &[text_paragraph(&format!("h{c}"))]))
                    .collect(),
                (0..3).map(|c| text_cell(&format!("b{c}"))).collect(),
            ])
        },
        |t| set_table_row_is_header(t, 0, false),
    );
    assert_eq!(rows, [[false, false, false], [false, false, false]]);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > preserves COLUMN bits when setting ROW header
#[test]
fn preserves_column_bits_when_setting_row_header() {
    let (rows, columns) = headers_after(
        || table_of(vec![vec![empty_cell(COLUMN), empty_cell(NO_STATUS)]]),
        |t| set_table_row_is_header(t, 0, true),
    );
    assert_eq!(rows, [[true, true]]);
    assert_eq!(columns, [[true, false]]);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > handles colSpan cells
#[test]
fn set_table_row_is_header_handles_col_span_cells() {
    let (rows, _) = headers_after(
        || {
            let span_cell = empty_cell(NO_STATUS);
            span_cell.set_col_span(2.0);
            table_of(vec![vec![span_cell, empty_cell(NO_STATUS)]])
        },
        |t| set_table_row_is_header(t, 0, true),
    );
    assert_eq!(rows, [[true, true]]);
}

/// Row 0: a cell spanning 2 rows, then a normal cell; row 1: one cell.
fn row_span_table() -> NodeRef {
    let span_cell = empty_cell(NO_STATUS);
    span_cell.set_row_span(2.0);
    table(&[
        row(&[span_cell, empty_cell(NO_STATUS)]),
        row(&[empty_cell(NO_STATUS)]),
    ])
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > handles rowSpan cells
#[test]
fn set_table_row_is_header_handles_row_span_cells() {
    let (rows, _) = headers_after(row_span_table, |t| set_table_row_is_header(t, 0, true));
    assert_eq!(rows, [vec![true, true], vec![false]]);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > sets a middle row as header
#[test]
fn sets_a_middle_row_as_header() {
    let (rows, _) = headers_after(
        || create_test_table(3, 3),
        |t| set_table_row_is_header(t, 1, true),
    );
    assert_eq!(
        rows,
        [
            [false, false, false],
            [true, true, true],
            [false, false, false]
        ]
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > throws on out-of-range row index
#[test]
fn throws_on_out_of_range_row_index() {
    let editor = build_editor();
    update(&editor, || get_root().append(&[create_test_table(3, 3)]));
    let result = editor.update(
        || set_table_row_is_header(&first_table(), 5, true),
        super::support_a::discrete(),
    );
    assert!(result.is_err());
    editor.dispose();
}

/// A table of one cell whose header state is `BOTH`.
fn both_table() -> NodeRef {
    table_of(vec![vec![empty_cell(BOTH)]])
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableRowIsHeader > clears ROW from BOTH header state, preserving COLUMN
#[test]
fn clears_row_from_both_header_state_preserving_column() {
    let (rows, columns) = headers_after(both_table, |t| set_table_row_is_header(t, 0, false));
    assert_eq!(rows, [[false]]);
    assert_eq!(columns, [[true]]);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > sets a column as header
#[test]
fn sets_a_column_as_header() {
    let (_, columns) = headers_after(
        || create_test_table(3, 3),
        |t| set_table_column_is_header(t, 0, true),
    );
    assert_eq!(
        columns,
        [
            [true, false, false],
            [true, false, false],
            [true, false, false]
        ]
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > clears a header column
#[test]
fn clears_a_header_column() {
    let (_, columns) = headers_after(
        || {
            table_of(
                (0..3)
                    .map(|_| vec![empty_cell(COLUMN), empty_cell(NO_STATUS)])
                    .collect(),
            )
        },
        |t| set_table_column_is_header(t, 0, false),
    );
    assert_eq!(columns, [[false, false], [false, false], [false, false]]);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > preserves ROW bits when setting COLUMN header
#[test]
fn preserves_row_bits_when_setting_column_header() {
    let (rows, columns) = headers_after(
        || table_of(vec![vec![empty_cell(ROW), empty_cell(ROW)]]),
        |t| set_table_column_is_header(t, 0, true),
    );
    assert_eq!(columns, [[true, false]]);
    assert_eq!(rows, [[true, true]]);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > handles rowSpan cells
#[test]
fn set_table_column_is_header_handles_row_span_cells() {
    let (_, columns) = headers_after(row_span_table, |t| set_table_column_is_header(t, 0, true));
    assert_eq!(columns, [vec![true, false], vec![false]]);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > throws on out-of-range column index
#[test]
fn throws_on_out_of_range_column_index() {
    let editor = build_editor();
    update(&editor, || get_root().append(&[create_test_table(3, 3)]));
    let result = editor.update(
        || set_table_column_is_header(&first_table(), 5, true),
        super::support_a::discrete(),
    );
    assert!(result.is_err());
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$setTableColumnIsHeader > clears COLUMN from BOTH header state, preserving ROW
#[test]
fn clears_column_from_both_header_state_preserving_row() {
    let (rows, columns) = headers_after(both_table, |t| set_table_column_is_header(t, 0, false));
    assert_eq!(columns, [[false]]);
    assert_eq!(rows, [[true]]);
}

/// `$getGridTexts(table)`: the text at each grid coordinate, spans
/// resolved, so column alignment across rows is asserted directly.
fn grid_texts(table: &NodeRef) -> Vec<Vec<String>> {
    grid(table)
        .iter()
        .map(|row| {
            row.iter()
                .map(|v| v.as_ref().unwrap().cell.get_text_content())
                .collect()
        })
        .collect()
}

/// Appends a table of `rows`, inserts a column after the cell at grid
/// `(row, column)`, and returns the grid texts.
fn grid_after_insert(
    rows: impl FnOnce() -> Vec<Vec<NodeRef>> + 'static,
    row: usize,
    column: usize,
) -> Vec<Vec<String>> {
    let (editor, _) = after(
        move || table_of(rows()),
        move |t| {
            let cell = grid(t)[row][column].as_ref().unwrap().cell.clone();
            insert_table_column_at_node(&cell, true, false).map(|_| ())
        },
    );
    let texts = read_latest(&editor, || grid_texts(&first_table()));
    editor.dispose();
    texts
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$insertTableColumnAtNode > walks left by each visited cell colSpan when a row is spanned
#[test]
fn walks_left_by_each_visited_cell_col_span_when_a_row_is_spanned() {
    // Grid:
    //   row0: [A0][X(rowSpan=2)][C(colSpan=2,rowSpan=2)][D0]
    //   row1: [A1]                                      [D1]
    // C occupies grid columns 2-3 of row 0; insert a column after it.
    let texts = grid_after_insert(
        || {
            vec![
                vec![
                    text_cell("A0"),
                    span_cell("X", 2.0, 1.0),
                    span_cell("C", 2.0, 2.0),
                    text_cell("D0"),
                ],
                vec![text_cell("A1"), text_cell("D1")],
            ]
        },
        0,
        2,
    );
    assert_eq!(
        texts,
        strings(&[
            &["A0", "X", "C", "C", "", "D0"],
            &["A1", "X", "C", "C", "", "D1"],
        ])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$insertTableColumnAtNode > inserts the new cell in the correct column for rows spanned by a rowSpan cell
#[test]
fn inserts_the_new_cell_in_the_correct_column_for_rows_spanned_by_a_row_span_cell() {
    // Grid:
    //   row0: [A(rowSpan=2), B]
    //   row1: [C]              (grid col 0 is covered by A's rowSpan)
    //   row2: [D, E]
    let texts = grid_after_insert(
        || {
            vec![
                vec![span_cell("A", 2.0, 1.0), text_cell("B")],
                vec![text_cell("C")],
                vec![text_cell("D"), text_cell("E")],
            ]
        },
        0,
        0,
    );
    // The inserted (empty) column must line up at grid column 1 in every row.
    // Row 1 is entirely covered at column 0 by A's rowSpan, so the new cell
    // has to be prepended before C rather than appended after it.
    assert_eq!(
        texts,
        strings(&[&["A", "", "B"], &["A", "", "C"], &["D", "", "E"]])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$insertTableColumnAtNode > does not prepend when a spanned row still owns a cell left of a colSpan > 1 anchor
#[test]
fn does_not_prepend_when_a_spanned_row_still_owns_a_cell_left_of_a_col_span_anchor() {
    // Grid:
    //   row0: [P, A(rowSpan=2), C(rowSpan=2, colSpan=2)]  cols P=0 A=1 C=2-3
    //   row1: [B]                                         cols 1-3 are covered
    // Insert after C, i.e. after grid column 3.
    let texts = grid_after_insert(
        || {
            vec![
                vec![
                    text_cell("P"),
                    span_cell("A", 2.0, 1.0),
                    span_cell("C", 2.0, 2.0),
                ],
                vec![text_cell("B")],
            ]
        },
        0,
        3,
    );
    // Row 1 owns B at column 0, so the new cell belongs after B, not before.
    assert_eq!(
        texts,
        strings(&[&["P", "A", "C", "C", ""], &["B", "A", "C", "C", ""]])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$insertTableColumnAtNode > inserts after the last owned cell of a spanned row, not an earlier one
#[test]
fn inserts_after_the_last_owned_cell_of_a_spanned_row_not_an_earlier_one() {
    // Grid:
    //   row0: [A, B, V(rowSpan=2), X(rowSpan=2, colSpan=2)]  cols V=2 X=3-4
    //   row1: [C0, C1]                                       cols 2-4 covered
    // Insert after X, i.e. after grid column 4.
    let texts = grid_after_insert(
        || {
            vec![
                vec![
                    text_cell("A"),
                    text_cell("B"),
                    span_cell("V", 2.0, 1.0),
                    span_cell("X", 2.0, 2.0),
                ],
                vec![text_cell("C0"), text_cell("C1")],
            ]
        },
        0,
        4,
    );
    // C1 is the last cell row 1 owns before the insertion column, so the new
    // cell goes after C1 rather than after C0.
    assert_eq!(
        texts,
        strings(&[
            &["A", "B", "V", "X", "X", ""],
            &["C0", "C1", "V", "X", "X", ""],
        ])
    );
}

/// `$mergeCells`'s `$cell(text)`: an empty string gives the cell a single
/// empty paragraph, which `$mergeCells` treats as having no content.
fn merge_cell(text: &str) -> NodeRef {
    if text.is_empty() {
        let cell = create_default_table_cell_node();
        cell.append(&[create_paragraph_node()]).unwrap();
        cell
    } else {
        text_cell(text)
    }
}

fn merge_table(rows: &'static [&'static [&'static str]]) -> NodeRef {
    table_of(
        rows.iter()
            .map(|cells| cells.iter().map(|t| merge_cell(t)).collect())
            .collect(),
    )
}

/// `$getMergedCell()`: the first cell of the first row.
fn merged_cell() -> NodeRef {
    first_table()
        .get_first_child()
        .unwrap()
        .get_first_child()
        .unwrap()
}

/// `$getBlockTexts(cell)`.
fn block_texts(cell: &NodeRef) -> Vec<String> {
    cell.get_children()
        .iter()
        .map(NodeRef::get_text_content)
        .collect()
}

/// Merges every cell of `rows` and returns the merged cell's block texts
/// and spans.
fn merged(rows: &'static [&'static [&'static str]]) -> (Vec<String>, f64, f64) {
    let (editor, _) = after(
        move || merge_table(rows),
        |t| {
            let all: Vec<NodeRef> = cells_of(t).into_iter().flatten().collect();
            merge_cells(&all).map(|_| ())
        },
    );
    let result = read_latest(&editor, || {
        let merged = merged_cell();
        (
            block_texts(&merged),
            merged.get_col_span(),
            merged.get_row_span(),
        )
    });
    editor.dispose();
    result
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$mergeCells > drops the empty paragraph of the target cell
#[test]
fn drops_the_empty_paragraph_of_the_target_cell() {
    assert_eq!(
        merged(&[&["", "hello"]]),
        (vec!["hello".to_owned()], 2.0, 1.0)
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$mergeCells > keeps every cell's content in order, and the spans
#[test]
fn keeps_every_cells_content_in_order_and_the_spans() {
    assert_eq!(
        merged(&[&["", "b"], &["c", "d"]]),
        (
            vec!["b".to_owned(), "c".to_owned(), "d".to_owned()],
            2.0,
            2.0
        )
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$mergeCells > keeps the same empty paragraph when every cell is empty
#[test]
fn keeps_the_same_empty_paragraph_when_every_cell_is_empty() {
    let editor = build_editor();
    update(&editor, || {
        get_root().append(&[merge_table(&[&["", "", ""]])])
    });
    let paragraph_key = read_latest(&editor, || {
        merged_cell()
            .get_first_child_or_throw()
            .unwrap()
            .get_key()
            .to_owned()
    });
    update(&editor, || {
        let all: Vec<NodeRef> = cells_of(&first_table()).into_iter().flatten().collect();
        merge_cells(&all).map(|_| ())
    });
    read_latest(&editor, || {
        let merged = merged_cell();
        assert_eq!(merged.get_children_size(), 1);
        let first_child = merged.get_first_child_or_throw().unwrap();
        assert!(first_child.get_type() == "paragraph" && first_child.is_empty());
        // Nothing clears the target unless content is about to land, so this is
        // the paragraph the cell started with rather than a replacement.
        assert_eq!(first_child.get_key(), paragraph_key);
        assert_eq!(merged.get_col_span(), 3.0);
    });
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$mergeCells > leaves the target alone when only it has content
#[test]
fn leaves_the_target_alone_when_only_it_has_content() {
    let (texts, col_span, _) = merged(&[&["keep", "", ""]]);
    assert_eq!(texts, ["keep"]);
    assert_eq!(col_span, 3.0);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableUtils.test.ts::$mergeCells > keeps the content of a non empty target ahead of the rest
#[test]
fn keeps_the_content_of_a_non_empty_target_ahead_of_the_rest() {
    let (texts, col_span, _) = merged(&[&["one", "two", "three"]]);
    assert_eq!(texts, ["one", "two", "three"]);
    assert_eq!(col_span, 3.0);
}

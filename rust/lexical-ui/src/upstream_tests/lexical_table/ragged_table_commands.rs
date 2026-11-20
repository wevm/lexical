//! Ports of `packages/lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts`.

use super::support_a::{
    build_with_table_nodes, cell_texts, first_table, grid, read, table_of, text_paragraph, update,
};
use crate::extension::LexicalEditorWithDispose;
use crate::table::{
    TableCellHeaderStates, create_table_cell_node, delete_table_column_at_selection,
    delete_table_row_at_selection, insert_table_column_at_selection, insert_table_into_grid,
    insert_table_row_at_selection, merge_cells, pad_table_rows, unmerge_cell,
};
use crate::{LexicalError, NodeRef, create_paragraph_node, get_root, get_selection};

/// `CellSpec`: a cell's text, spans and header state.
#[derive(Clone, Copy)]
struct CellSpec {
    text: &'static str,
    col_span: f64,
    row_span: f64,
    header_state: i32,
}

/// A spec of only `text`.
const fn t(text: &'static str) -> CellSpec {
    CellSpec {
        text,
        col_span: 1.0,
        row_span: 1.0,
        header_state: TableCellHeaderStates::NO_STATUS,
    }
}

/// `buildTableEditor(rows)`: an editor whose table has exactly the given
/// rows. The table transform is deliberately not registered: this is the
/// state a merge of concurrent collaborative edits can leave behind (e.g.
/// one peer inserts a row while another inserts a column), because remote
/// updates are applied with transforms skipped.
fn build_table_editor(rows: Vec<Vec<CellSpec>>) -> LexicalEditorWithDispose {
    build_with_table_nodes("[ragged-table]", move || {
        let table = table_of(
            rows.iter()
                .map(|cells| {
                    cells
                        .iter()
                        .map(|spec| {
                            let cell =
                                create_table_cell_node(spec.header_state, spec.col_span, None);
                            cell.set_row_span(spec.row_span);
                            cell.append(&[text_paragraph(spec.text)]).unwrap();
                            cell
                        })
                        .collect()
                })
                .collect(),
        );
        get_root().clear()?;
        get_root().append(&[table, create_paragraph_node()])
    })
}

/// Rows of 3, 3 and 2 cells.
fn ragged() -> Vec<Vec<CellSpec>> {
    vec![
        vec![t("00"), t("01"), t("02")],
        vec![t("10"), t("11"), t("12")],
        vec![t("20"), t("21")],
    ]
}

/// `$cell(r, c)`.
fn cell(r: usize, c: usize) -> NodeRef {
    first_table()
        .get_child_at_index(r)
        .unwrap()
        .get_child_at_index(c)
        .unwrap()
}

/// `$selectCell(r, c)`.
fn select_cell(r: usize, c: usize) -> Result<(), LexicalError> {
    cell(r, c)
        .get_first_child()
        .unwrap()
        .select_end()
        .map(|_| ())
}

/// `$widths()`: grid columns covered by each row, spans included.
fn widths() -> Vec<usize> {
    grid(&first_table())
        .iter()
        .map(|row| row.iter().filter(|v| v.is_some()).count())
        .collect()
}

/// `$texts()`: text of each row's cells, in document order.
fn texts() -> Vec<Vec<String>> {
    cell_texts(&first_table())
}

fn strings(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|cells| cells.iter().map(|s| (*s).to_owned()).collect())
        .collect()
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > insert column after the last column of a full row
#[test]
fn insert_column_after_the_last_column_of_a_full_row() {
    let editor = build_table_editor(ragged());
    update(&editor, || {
        select_cell(0, 2)?;
        insert_table_column_at_selection(true).map(|_| ())
    });
    assert_eq!(read(&editor, widths), [4, 4, 4]);
    assert_eq!(
        read(&editor, texts),
        strings(&[
            &["00", "01", "02", ""],
            &["10", "11", "12", ""],
            &["20", "21", "", ""],
        ])
    );
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > insert column after the last cell of the short row
#[test]
fn insert_column_after_the_last_cell_of_the_short_row() {
    let editor = build_table_editor(ragged());
    update(&editor, || {
        select_cell(2, 1)?;
        insert_table_column_at_selection(true).map(|_| ())
    });
    assert_eq!(read(&editor, widths), [4, 4, 4]);
    assert_eq!(
        read(&editor, texts),
        strings(&[
            &["00", "01", "", "02"],
            &["10", "11", "", "12"],
            &["20", "21", "", ""],
        ])
    );
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > insert row after the short row
#[test]
fn insert_row_after_the_short_row() {
    let editor = build_table_editor(ragged());
    update(&editor, || {
        select_cell(2, 0)?;
        insert_table_row_at_selection(true).map(|_| ())
    });
    assert_eq!(read(&editor, widths), [3, 3, 3, 3]);
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > insert row before the first row
#[test]
fn insert_row_before_the_first_row() {
    let editor = build_table_editor(ragged());
    update(&editor, || {
        select_cell(0, 0)?;
        insert_table_row_at_selection(false).map(|_| ())
    });
    assert_eq!(read(&editor, widths), [3, 3, 3, 3]);
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > delete the last column
#[test]
fn delete_the_last_column() {
    let editor = build_table_editor(ragged());
    update(&editor, || {
        select_cell(0, 2)?;
        delete_table_column_at_selection()
    });
    assert_eq!(read(&editor, widths), [2, 2, 2]);
    assert_eq!(
        read(&editor, texts),
        strings(&[&["00", "01"], &["10", "11"], &["20", "21"]])
    );
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > delete the short row
#[test]
fn delete_the_short_row() {
    let editor = build_table_editor(ragged());
    update(&editor, || {
        select_cell(2, 0)?;
        delete_table_row_at_selection()
    });
    assert_eq!(read(&editor, widths), [3, 3]);
    assert_eq!(
        read(&editor, texts),
        strings(&[&["00", "01", "02"], &["10", "11", "12"]])
    );
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > merge a block that reaches past the end of the short row
#[test]
fn merge_a_block_that_reaches_past_the_end_of_the_short_row() {
    let editor = build_table_editor(ragged());
    update(&editor, || {
        merge_cells(&[cell(1, 1), cell(1, 2), cell(2, 1)]).map(|_| ())
    });
    assert_eq!(read(&editor, widths), [3, 3, 3]);
    let texts = read(&editor, texts);
    assert_eq!(texts[0], ["00", "01", "02"]);
    assert_eq!(texts[1][0], "10");
    // `toMatch(/11[^]*12[^]*21/)`.
    let merged = &texts[1][1];
    let i11 = merged.find("11").expect("11");
    let i12 = merged[i11 + 2..].find("12").expect("12") + i11 + 2;
    assert!(merged[i12 + 2..].contains("21"), "{merged:?}");
    assert_eq!(texts[2], ["20"]);
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > merge cells in the last column
#[test]
fn merge_cells_in_the_last_column() {
    let editor = build_table_editor(ragged());
    update(&editor, || {
        merge_cells(&[cell(1, 1), cell(1, 2)]).map(|_| ())
    });
    assert_eq!(read(&editor, widths), [3, 3, 3]);
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::table commands on a ragged table > unmerge a header-column cell when another row is short
#[test]
fn unmerge_a_header_column_cell_when_another_row_is_short() {
    let editor = build_table_editor(vec![
        vec![
            t("00"),
            t("01"),
            CellSpec {
                header_state: TableCellHeaderStates::COLUMN,
                row_span: 2.0,
                ..t("M")
            },
        ],
        vec![t("10"), t("11")],
        vec![t("20")],
    ]);
    update(&editor, || {
        select_cell(0, 2)?;
        unmerge_cell()
    });
    assert_eq!(read(&editor, widths), [3, 3, 3]);
    assert_eq!(
        read(&editor, texts),
        strings(&[&["00", "01", "M"], &["10", "11", ""], &["20", "", ""]])
    );
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::pasting a table into a ragged table > paste a 2x2 table at the last cell of the short row
#[test]
fn paste_a_2x2_table_at_the_last_cell_of_the_short_row() {
    let editor = build_table_editor(ragged());
    update(&editor, || {
        let template = table_of(
            [["a", "b"], ["c", "d"]]
                .iter()
                .map(|texts| {
                    texts
                        .iter()
                        .map(|text| super::support_a::text_cell(text))
                        .collect()
                })
                .collect(),
        );
        select_cell(2, 1)?;
        let selection = get_selection().filter(|s| s.is_range());
        let selection = selection.expect("Expected a RangeSelection");
        insert_table_into_grid(&template, &selection).map(|_| ())
    });
    assert_eq!(read(&editor, widths), [3, 3, 3, 3]);
    assert_eq!(
        read(&editor, texts),
        strings(&[
            &["00", "01", "02"],
            &["10", "11", "12"],
            &["20", "a", "b"],
            &["", "c", "d"],
        ])
    );
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::$padTableRows > pads short rows and leaves a rectangular table alone
#[test]
fn pads_short_rows_and_leaves_a_rectangular_table_alone() {
    let editor = build_table_editor(ragged());
    let changed = std::rc::Rc::new(std::cell::Cell::new(None));
    let out = changed.clone();
    update(&editor, move || {
        out.set(Some(pad_table_rows(&first_table(), |_| true)?));
        Ok(())
    });
    assert_eq!(changed.get(), Some(true));
    assert_eq!(read(&editor, widths), [3, 3, 3]);
    let out = changed.clone();
    update(&editor, move || {
        out.set(Some(pad_table_rows(&first_table(), |_| true)?));
        Ok(())
    });
    assert_eq!(changed.get(), Some(false));
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/RaggedTableCommands.test.ts::$padTableRows > fills a hole left of a rowSpan from an earlier row
#[test]
fn fills_a_hole_left_of_a_row_span_from_an_earlier_row() {
    let editor = build_table_editor(vec![
        vec![
            t("00"),
            t("01"),
            CellSpec {
                row_span: 2.0,
                ..t("02")
            },
            t("03"),
        ],
        vec![t("10")],
    ]);
    update(&editor, || {
        pad_table_rows(&first_table(), |_| true).map(|_| ())
    });
    assert_eq!(read(&editor, widths), [4, 4]);
    assert_eq!(
        read(&editor, texts),
        strings(&[&["00", "01", "02", "03"], &["10", "", ""]])
    );
    editor.dispose();
}

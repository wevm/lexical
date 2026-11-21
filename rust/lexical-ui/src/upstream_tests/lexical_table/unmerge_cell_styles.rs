//! Ports of `packages/lexical-table/src/__tests__/unit/UnmergeCellStyles.test.ts`.

use super::support_a::{build_table_editor, cells_of, first_table, read, update};
use crate::extension::LexicalEditorWithDispose;
use crate::table::{
    InsertTableCommandPayloadHeaders, create_table_node_with_dimensions, merge_cells,
    unmerge_cell_node,
};
use crate::{Direction, LexicalError, NodeRef, get_root};

fn build_editor() -> LexicalEditorWithDispose {
    build_table_editor("unmerge-styles-host")
}

/// `$cells()`: the cells of each of the root's table's rows.
fn cells() -> Vec<Vec<NodeRef>> {
    cells_of(&first_table())
}

/// `$getRoot().clear().append($createTableNodeWithDimensions(rows, columns,
/// false))`, then `$mergeCells` of every cell.
fn merge_all(rows: usize, columns: usize) -> Result<NodeRef, LexicalError> {
    get_root().clear()?;
    get_root().append(&[create_table_node_with_dimensions(
        rows,
        columns,
        InsertTableCommandPayloadHeaders::Both(false),
    )?])?;
    let all: Vec<NodeRef> = cells().into_iter().flatten().collect();
    Ok(merge_cells(&all)?.expect("expected the cells to merge"))
}

// upstream: lexical-table/src/__tests__/unit/UnmergeCellStyles.test.ts::$unmergeCellNode keeps the cell presentation > the cells a merged cell splits into keep its backgroundColor and verticalAlign
#[test]
fn the_cells_a_merged_cell_splits_into_keep_its_background_color_and_vertical_align() {
    let editor = build_editor();
    update(&editor, || {
        let merged = merge_all(2, 2)?;
        merged.set_background_color(Some("rgb(255, 0, 0)"));
        merged.set_vertical_align(Some("middle"));
        Ok(())
    });
    update(&editor, || {
        let merged = cells()[0][0].clone();
        assert_eq!(merged.get_col_span(), 2.0);
        assert_eq!(merged.get_row_span(), 2.0);
        unmerge_cell_node(&merged)
    });
    read(&editor, || {
        let rows = cells();
        assert_eq!(rows.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2]);
        for cell in rows.iter().flatten() {
            assert_eq!(cell.get_col_span(), 1.0);
            assert_eq!(cell.get_row_span(), 1.0);
            assert_eq!(
                cell.get_background_color().as_deref(),
                Some("rgb(255, 0, 0)")
            );
            assert_eq!(cell.get_vertical_align().as_deref(), Some("middle"));
        }
    });
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/UnmergeCellStyles.test.ts::$unmergeCellNode keeps the cell presentation > the split cells keep the element format, style and direction
#[test]
fn the_split_cells_keep_the_element_format_style_and_direction() {
    let editor = build_editor();
    update(&editor, || {
        let merged = merge_all(2, 2)?;
        merged.set_element_format("center");
        merged.set_element_style("color: red;");
        merged.set_direction(Some(Direction::Rtl));
        Ok(())
    });
    update(&editor, || unmerge_cell_node(&cells()[0][0]));
    read(&editor, || {
        // Every cell, not only the one that was already there: a split that
        // formats the original differently from the cells beside it is the bug.
        for cell in cells().iter().flatten() {
            let data = cell.element_data();
            assert_eq!(data.get_format_type(), "center");
            assert_eq!(data.get_style(), "color: red;");
            assert_eq!(data.get_direction(), Some(Direction::Rtl));
        }
    });
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/UnmergeCellStyles.test.ts::$unmergeCellNode keeps the cell presentation > the split cells take no width, which the merged cell measured wide
#[test]
fn the_split_cells_take_no_width_which_the_merged_cell_measured_wide() {
    let editor = build_editor();
    update(&editor, || {
        let merged = merge_all(1, 2)?;
        assert_eq!(merged.get_col_span(), 2.0);
        merged.set_cell_width(Some(240.0));
        Ok(())
    });
    update(&editor, || unmerge_cell_node(&cells()[0][0]));
    read(&editor, || {
        // The cell that was measured at 240px still is; the cell split off from
        // it must not claim that width for a single column as well, and like
        // every other cell created in LexicalTableUtils it leaves its width to
        // the table's colWidths.
        assert_eq!(
            cells()[0]
                .iter()
                .map(NodeRef::get_cell_width)
                .collect::<Vec<_>>(),
            [Some(240.0), None]
        );
    });
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/UnmergeCellStyles.test.ts::$unmergeCellNode keeps the cell presentation > an unstyled merged cell still splits into unstyled cells
#[test]
fn an_unstyled_merged_cell_still_splits_into_unstyled_cells() {
    let editor = build_editor();
    update(&editor, || merge_all(1, 2).map(|_| ()));
    update(&editor, || unmerge_cell_node(&cells()[0][0]));
    read(&editor, || {
        for cell in &cells()[0] {
            assert_eq!(cell.get_background_color(), None);
            assert_eq!(cell.get_vertical_align(), None);
        }
    });
    editor.dispose();
}

//! Ports of `packages/lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx`.

use super::support_c::{
    cell_texts, initialize_table_unit_test, map_cell, root_table, table_with_dimensions,
};
use crate::nodes::paragraph::ParagraphNode;
use crate::selection::{StylePatchValue, patch_style_text};
use crate::table::{
    TableCellHeaderStates, TableSelectionShape, create_table_cell_node, create_table_node,
    create_table_row_node, create_table_selection_from, delete_table_row_at_selection,
    is_table_node,
};
use crate::upstream_tests::support::update;
use crate::{
    Editor, LexicalError, NodeRef, PointType, create_paragraph_node, create_text_node, get_root,
    get_selection, select_all_in, set_selection,
};

/// `$createTableCellNode()`.
fn cell() -> NodeRef {
    create_table_cell_node(TableCellHeaderStates::NO_STATUS, 1.0, None)
}

/// `$createParagraphNode().append($createTextNode(text))`.
fn text_paragraph(text: &str) -> Result<NodeRef, LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)])?;
    Ok(paragraph)
}

/// The `beforeEach` of `table selection`: a 2x2 table whose cells read
/// `col,row`, all selected.
fn table_selection_editor() -> Editor {
    let editor = initialize_table_unit_test();
    update(&editor, || {
        let table_node = create_table_node();
        for row in 0..2 {
            let row_node = create_table_row_node(None);
            for col in 0..2 {
                let cell_node = cell();
                cell_node.append(&[text_paragraph(&format!("{col},{row}"))?])?;
                row_node.append(&[cell_node])?;
            }
            table_node.append(&[row_node])?;
        }
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&table_node))?;
        set_selection(Some(create_table_selection_from(
            &table_node,
            &map_cell(&table_node, 0, 0),
            &map_cell(&table_node, 1, 1),
        )?));
        Ok(())
    });
    editor
}

fn texts(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| row.iter().map(|s| (*s).to_owned()).collect())
        .collect()
}

/// `tableSelection.insertRawText(text)`, then `$getCellTexts()`.
fn insert_raw_text(text: &'static str) -> Vec<Vec<String>> {
    let editor = table_selection_editor();
    crate::upstream_tests::support::update_with(&editor, move || {
        get_selection()
            .expect("a selection")
            .insert_raw_text(text)?;
        Ok(cell_texts(&root_table()))
    })
}

fn text_styles() -> Vec<String> {
    get_root()
        .get_all_text_nodes()
        .iter()
        .map(|node| node.text_data().get_style().to_owned())
        .collect()
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > regression #7076 > $patchStyleText works on a TableSelection
#[test]
fn patch_style_text_works_on_a_table_selection() {
    let editor = table_selection_editor();
    update(&editor, || {
        assert_eq!(text_styles(), vec![""; 4]);
        let selection = get_selection().expect("a selection");
        assert!(selection.is_table());
        patch_style_text(&selection, &[("color", StylePatchValue::from("red"))])?;
        assert!(get_selection().is_some_and(|s| s.is_table()));
        assert_eq!(text_styles(), vec!["color: red;"; 4]);
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > regression #7076 > $patchStyleText applies styles to empty table cells
#[test]
fn patch_style_text_applies_styles_to_empty_table_cells() {
    let editor = table_selection_editor();
    update(&editor, || {
        let selection = get_selection().expect("a selection");
        assert!(selection.is_table());
        let table = root_table();
        let empty_paragraph = map_cell(&table, 0, 0)
            .get_first_child()
            .expect("a paragraph");
        assert!(empty_paragraph.get_latest().is::<ParagraphNode>());
        empty_paragraph.clear()?;
        assert_eq!(empty_paragraph.get_children_size(), 0);

        patch_style_text(&selection, &[("color", StylePatchValue::from("blue"))])?;

        let filled_paragraph = map_cell(&table, 0, 1)
            .get_first_child()
            .expect("a paragraph");
        assert!(filled_paragraph.get_latest().is::<ParagraphNode>());
        let text_node = filled_paragraph.get_first_child().expect("a text node");
        assert!(text_node.is_text());
        assert_eq!(text_node.text_data().get_style(), "color: blue;");
        assert_eq!(
            empty_paragraph.element_data().get_text_style(),
            "color: blue;"
        );
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > fills 2x2 table with matching TSV
#[test]
fn fills_2x2_table_with_matching_tsv() {
    assert_eq!(
        insert_raw_text("A\tB\nC\tD"),
        texts(&[&["A", "B"], &["C", "D"]])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > single value fills anchor cell only
#[test]
fn single_value_fills_anchor_cell_only() {
    assert_eq!(
        insert_raw_text("hello"),
        texts(&[&["hello", "1,0"], &["0,1", "1,1"]])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > single row TSV fills one row with two columns
#[test]
fn single_row_tsv_fills_one_row_with_two_columns() {
    assert_eq!(
        insert_raw_text("a\tb"),
        texts(&[&["a", "b"], &["0,1", "1,1"]])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > expands rows when TSV has more rows
#[test]
fn expands_rows_when_tsv_has_more_rows() {
    let texts_ = insert_raw_text("A\tB\nC\tD\nE\tF");
    assert_eq!(texts_.len(), 3);
    assert_eq!(texts_, texts(&[&["A", "B"], &["C", "D"], &["E", "F"]]));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > expands columns when TSV has more columns
#[test]
fn expands_columns_when_tsv_has_more_columns() {
    let texts_ = insert_raw_text("A\tB\tC\nD\tE\tF");
    assert_eq!(texts_[0], ["A", "B", "C"]);
    assert_eq!(texts_[1], ["D", "E", "F"]);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > expands both rows and columns
#[test]
fn expands_both_rows_and_columns() {
    assert_eq!(
        insert_raw_text("A\tB\tC\nD\tE\tF\nG\tH\tI"),
        texts(&[&["A", "B", "C"], &["D", "E", "F"], &["G", "H", "I"]])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > strips trailing newline from clipboard
#[test]
fn strips_trailing_newline_from_clipboard() {
    assert_eq!(
        insert_raw_text("X\tY\nZ\tW\n"),
        texts(&[&["X", "Y"], &["Z", "W"]])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > paste from non-origin anchor fills offset cells
#[test]
fn paste_from_non_origin_anchor_fills_offset_cells() {
    let editor = table_selection_editor();
    update(&editor, || {
        let table = root_table();
        let corner = map_cell(&table, 1, 1);
        let offset_selection = create_table_selection_from(&table, &corner, &corner)?;
        offset_selection.insert_raw_text("X\tY\nZ\tW")?;
        let texts = cell_texts(&table);
        // Original cells preserved, offset filled + expanded.
        assert_eq!(texts[0][0], "0,0");
        assert_eq!(texts[0][1], "1,0");
        assert_eq!(texts[1][1], "X");
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > unmerges and fills merged cells
#[test]
fn unmerges_and_fills_merged_cells() {
    let editor = table_selection_editor();
    update(&editor, || {
        // A table whose first cell spans 2 columns.
        let merged = create_table_node();
        let top_left = cell();
        top_left.set_col_span(2.0);
        top_left.append(&[text_paragraph("M")?])?;
        let c = cell();
        c.append(&[text_paragraph("c")?])?;
        let first_row = create_table_row_node(None);
        first_row.append(&[top_left, c])?;
        let second_row = create_table_row_node(None);
        for _ in 0..3 {
            let x = cell();
            x.append(&[text_paragraph("x")?])?;
            second_row.append(&[x])?;
        }
        merged.append(&[first_row, second_row])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&merged))?;
        let sel = create_table_selection_from(
            &merged,
            &map_cell(&merged, 0, 0),
            &map_cell(&merged, 1, 2),
        )?;
        sel.insert_raw_text("A\tB\tC\nD\tE\tF")?;
        let texts = cell_texts(&merged);
        // The merged cell is unmerged, each cell gets its TSV value.
        assert_eq!(texts[0], ["A", "B", "C"]);
        assert_eq!(texts[1], ["D", "E", "F"]);
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > insertRawText > empty string is no-op
#[test]
fn empty_string_is_no_op() {
    assert_eq!(
        insert_raw_text(""),
        texts(&[&["0,0", "1,0"], &["0,1", "1,1"]])
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > getShape > returns correct shape for non-merged table
#[test]
fn returns_correct_shape_for_non_merged_table() {
    let editor = table_selection_editor();
    update(&editor, || {
        let shape = get_selection().expect("a selection").get_table_shape()?;
        assert_eq!(
            shape,
            Some(TableSelectionShape {
                from_x: 0,
                from_y: 0,
                to_x: 1,
                to_y: 1,
            })
        );
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::table selection > regression #7140 > selection points to missing nodes after deleting table rows
#[test]
fn selection_points_to_missing_nodes_after_deleting_table_rows() {
    let editor = table_selection_editor();
    update(&editor, || {
        let table = root_table();
        let root = get_root();
        root.clear()?;
        root.append(&[text_paragraph("Before table")?])?;
        root.append(std::slice::from_ref(&table))?;
        set_selection(Some(create_table_selection_from(
            &table,
            &map_cell(&table, 0, 0),
            &map_cell(&table, 1, 1),
        )?));
        Ok(())
    });
    // Delete the table rows.
    update(&editor, || {
        assert!(get_selection().is_some_and(|s| s.is_table()));
        delete_table_row_at_selection()
    });
    // Access the selection after the deletion.
    update(&editor, || {
        if let Some(selection) = get_selection().filter(|s| s.is_table()) {
            assert!(!selection.table().expect("a table selection").is_valid());
            assert!(selection.get_nodes()?.is_empty());
        }
        Ok(())
    });
}

/// `$deleteForward()`.
fn delete_forward() -> Result<(), LexicalError> {
    let selection = get_selection().filter(|s| s.is_range());
    selection.expect("a RangeSelection").delete_character(false)
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::regression #8075 > forward delete removes an empty paragraph before a table
#[test]
fn forward_delete_removes_an_empty_paragraph_before_a_table() {
    let editor = initialize_table_unit_test();
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().clear()?;
        get_root().append(&[paragraph.clone(), table_with_dimensions(2, 2, false)])?;
        paragraph.select_start()?;
        Ok(())
    });
    update(&editor, delete_forward);
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 1);
            assert!(is_table_node(&children[0]));
        })
        .unwrap();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::regression #8075 > forward delete does not merge a table into a non-empty paragraph
#[test]
fn forward_delete_does_not_merge_a_table_into_a_non_empty_paragraph() {
    let editor = initialize_table_unit_test();
    update(&editor, || {
        let paragraph = text_paragraph("before")?;
        get_root().clear()?;
        get_root().append(&[paragraph.clone(), table_with_dimensions(2, 2, false)])?;
        paragraph.select_end()?;
        Ok(())
    });
    update(&editor, delete_forward);
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].get_text_content(), "before");
            assert!(is_table_node(&children[1]));
        })
        .unwrap();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::regression #8075 > forward delete from an empty paragraph in a table cell does nothing
#[test]
fn forward_delete_from_an_empty_paragraph_in_a_table_cell_does_nothing() {
    let editor = initialize_table_unit_test();
    update(&editor, || {
        let table = table_with_dimensions(1, 2, false);
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&table))?;
        let cell = table
            .get_first_child_or_throw()?
            .get_first_child_or_throw()?;
        let paragraph = create_paragraph_node();
        cell.clear()?;
        cell.append(std::slice::from_ref(&paragraph))?;
        paragraph.select_start()?;
        Ok(())
    });
    update(&editor, delete_forward);
    editor
        .read(|| {
            let row = root_table().get_first_child().expect("a row");
            // The next cell was not pulled into the empty one.
            assert_eq!(row.get_children_size(), 2);
            assert_eq!(
                row.get_first_child_or_throw().unwrap().get_text_content(),
                ""
            );
        })
        .unwrap();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableSelection.test.tsx::repeated select-all inside a table cell > stays scoped to the cell and leaves the table intact
#[test]
fn stays_scoped_to_the_cell_and_leaves_the_table_intact() {
    let editor = initialize_table_unit_test();
    update(&editor, || {
        let table = table_with_dimensions(2, 2, false);
        get_root().clear()?;
        get_root().append(&[text_paragraph("before")?, table.clone()])?;
        let cell = table
            .get_first_child_or_throw()?
            .get_first_child_or_throw()?;
        let paragraph = cell.get_first_child_or_throw()?;
        assert!(paragraph.get_latest().is::<ParagraphNode>());
        paragraph.append(&[create_text_node("a1")])?;
        paragraph.select_end()?;
        Ok(())
    });
    // All in one update: no DOM round-trip renormalizes the points between.
    update(&editor, || {
        let initial = get_selection().filter(|s| s.is_range());
        let mut selection = initial.expect("a RangeSelection");
        for _ in 0..3 {
            selection = select_all_in(&selection)?;
            let range = selection.range().expect("a RangeSelection");
            assert_eq!(range.anchor.point_type, PointType::Text);
            assert_eq!(range.focus.point_type, PointType::Text);
        }
        selection.remove_text()
    });
    editor
        .read(|| {
            assert_eq!(get_root().get_children_size(), 2);
            let table = get_root().get_last_child().expect("a table");
            assert!(is_table_node(&table));
            // Every row and cell survives, and no paragraph was appended
            // directly into the table or a row.
            assert_eq!(cell_texts(&table), texts(&[&["", ""], &["", ""]]));
        })
        .unwrap();
}

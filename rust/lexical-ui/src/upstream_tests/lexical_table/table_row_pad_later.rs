//! Ports of `packages/lexical-table/src/__tests__/unit/TableRowPadLater.test.ts`.
//!
//! Three rows of three cells; an update removes a cell from the first two,
//! and a transform that runs later in the same update widens the third row
//! (it appends two cells, or raises a cell's span). The rows that lost
//! cells are padded to the width the table has once the update is done:
//! every row covers five grid positions.

use super::support_d::{
    build_null_initial, cell, discrete, map_widths, root_table, text_paragraph,
};
use crate::table::{create_table_node, create_table_row_node, table_extension};
use crate::{LexicalError, get_root};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// `$cell()`.
fn x_cell() -> crate::NodeRef {
    cell(0, &[text_paragraph("x")])
}

#[derive(Clone, Copy)]
enum Change {
    Append,
    Span,
}

fn rows_that_lost_cells_are_padded(change: Change) {
    let editor = build_null_initial("table-row-pad-later", vec![table_extension().into()]);
    let first = Rc::new(RefCell::new(String::new()));
    let active = Rc::new(Cell::new(false));
    let changed = Rc::new(Cell::new(false));
    let (f, a, c) = (first.clone(), active.clone(), changed.clone());
    let _remove = editor.register_node_transform("tablerow", move |row| {
        if a.get() && !c.get() && *row.get_key() == *f.borrow() {
            c.set(true);
            let table = row.get_parent().unwrap();
            let last = table.get_last_child_or_throw()?;
            match change {
                Change::Append => last.append(&[x_cell(), x_cell()])?,
                Change::Span => last.get_first_child_or_throw()?.set_col_span(3.0),
            }
        }
        Ok(())
    });
    let f = first.clone();
    editor
        .update(
            move || {
                let table = create_table_node();
                for i in 0..3 {
                    let row = create_table_row_node(None);
                    row.append(&[x_cell(), x_cell(), x_cell()])?;
                    if i == 0 {
                        *f.borrow_mut() = row.get_key().to_owned();
                    }
                    table.append(&[row])?;
                }
                get_root().append(&[table])
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(
        editor.read(|| map_widths(&root_table())).unwrap(),
        [3, 3, 3]
    );
    active.set(true);
    editor
        .update(
            || -> Result<(), LexicalError> {
                let rows = root_table().get_children();
                rows[0].get_last_child_or_throw()?.remove(false)?;
                rows[1].get_last_child_or_throw()?.remove(false)
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(
        editor.read(|| map_widths(&root_table())).unwrap(),
        [5, 5, 5]
    );
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/TableRowPadLater.test.ts::rows that lost cells are padded to a width another row reaches later in the update (append)
#[test]
fn rows_that_lost_cells_are_padded_to_a_width_another_row_reaches_later_in_the_update_append() {
    rows_that_lost_cells_are_padded(Change::Append);
}

// upstream: lexical-table/src/__tests__/unit/TableRowPadLater.test.ts::rows that lost cells are padded to a width another row reaches later in the update (span)
#[test]
fn rows_that_lost_cells_are_padded_to_a_width_another_row_reaches_later_in_the_update_span() {
    rows_that_lost_cells_are_padded(Change::Span);
}

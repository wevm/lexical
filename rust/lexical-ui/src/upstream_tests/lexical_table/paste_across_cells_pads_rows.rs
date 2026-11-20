//! Ports of `packages/lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts`.

use super::support_d::{
    build_null_initial, cell, cells_per_row, discrete, discrete_skip_transforms, root_table,
};
use crate::clipboard::insert_generated_nodes;
use crate::dom::parse_from_string;
use crate::extension::node_structure_extension;
use crate::html::generate_nodes_from_dom;
use crate::list::{ListType, create_list_item_node, create_list_node, list_extension};
use crate::rich_text::rich_text_extension;
use crate::table::{
    TableConfigOverrides, create_table_node, create_table_row_node, is_table_row_node,
    table_extension,
};
use crate::{
    AnyLexicalExtensionArgument, LexicalError, NodeRef, PointType, Which, config_extension,
    create_paragraph_node, create_range_selection, create_text_node, get_editor, get_root,
    set_selection,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// `configExtension(TableExtension, {hasHorizontalScroll: false})`.
fn table_without_scroll() -> AnyLexicalExtensionArgument {
    config_extension(
        &table_extension(),
        [TableConfigOverrides {
            has_horizontal_scroll: Some(false),
            ..TableConfigOverrides::default()
        }],
    )
}

/// `$createListTable()`: a 3x3 table whose cells each hold a bullet list
/// with one item "rNcM text".
fn create_list_table() -> Result<(), LexicalError> {
    let table = create_table_node();
    for r in 0..3 {
        let row = create_table_row_node(None);
        for c in 0..3 {
            let item = create_list_item_node(None);
            item.append(&[create_text_node(&format!("r{r}c{c} text"))])?;
            let list = create_list_node(ListType::Bullet, 1.0);
            list.append(&[item])?;
            row.append(&[cell(0, &[list])])?;
        }
        table.append(&[row])?;
    }
    let root = get_root();
    root.clear()?;
    root.append(&[table, create_paragraph_node()])
}

/// `$pasteAcross(from, to)`: selects from offset 2 of the text starting
/// with `from` to offset 2 of the text starting with `to`, and pastes rich
/// HTML holding a list over it.
fn paste_across(from: &str, to: &str) -> Result<(), LexicalError> {
    let texts = get_root().get_all_text_nodes();
    let find = |prefix: &str| {
        let text = texts
            .iter()
            .find(|t| t.get_text_content().starts_with(prefix));
        text.unwrap_or_else(|| panic!("no text starting with {prefix}"))
            .clone()
    };
    let selection = create_range_selection();
    selection.point_set(Which::Anchor, find(from).get_key(), 2, PointType::Text)?;
    selection.point_set(Which::Focus, find(to).get_key(), 2, PointType::Text)?;
    set_selection(Some(selection.clone()));
    let html = "<p><b>bold</b> <i>italic</i></p><ul><li>item</li></ul>";
    let editor = get_editor();
    let nodes = generate_nodes_from_dom(&editor, &parse_from_string(html))?;
    insert_generated_nodes(&editor, nodes, &selection)
}

fn paste_keeps_rows_wide(with_node_structure: bool, from: &'static str, to: &'static str) {
    let mut dependencies: Vec<AnyLexicalExtensionArgument> = Vec::new();
    if with_node_structure {
        dependencies.push(node_structure_extension().into());
    }
    dependencies.extend([
        rich_text_extension().into(),
        list_extension().into(),
        table_without_scroll(),
    ]);
    let editor = build_null_initial("paste-across-cells", dependencies);
    editor.update(create_list_table, discrete()).unwrap();
    editor
        .update(move || paste_across(from, to), discrete())
        .unwrap();
    assert_eq!(editor.read(cells_per_row).unwrap(), "3,3,3");
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::a paste over a selection across cells > from r0c1 to r1c1 keeps every row as wide as the table
#[test]
fn from_r0c1_to_r1c1_keeps_every_row_as_wide_as_the_table() {
    paste_keeps_rows_wide(false, "r0c1", "r1c1");
}

// upstream: lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::a paste over a selection across cells > from r0c2 to r1c1 keeps every row as wide as the table
#[test]
fn from_r0c2_to_r1c1_keeps_every_row_as_wide_as_the_table() {
    paste_keeps_rows_wide(false, "r0c2", "r1c1");
}

// upstream: lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::a paste over a selection across cells with NodeStructureExtension > from r0c1 to r1c1 keeps every row as wide as the table
#[test]
fn with_node_structure_extension_from_r0c1_to_r1c1_keeps_every_row_as_wide_as_the_table() {
    paste_keeps_rows_wide(true, "r0c1", "r1c1");
}

// upstream: lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::a paste over a selection across cells with NodeStructureExtension > from r0c2 to r1c1 keeps every row as wide as the table
#[test]
fn with_node_structure_extension_from_r0c2_to_r1c1_keeps_every_row_as_wide_as_the_table() {
    paste_keeps_rows_wide(true, "r0c2", "r1c1");
}

/// `$row(width)`: a row of `width` cells, each with an empty paragraph.
fn row_of(width: usize) -> NodeRef {
    let row = create_table_row_node(None);
    for _ in 0..width {
        row.append(&[cell(0, &[create_paragraph_node()])]).unwrap();
    }
    row
}

// upstream: lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::a change to one row pads that row only, not the other short rows
#[test]
fn a_change_to_one_row_pads_that_row_only_not_the_other_short_rows() {
    // Rows ragged before the update (a concurrent row and column insert from
    // collaborators, merged without transforms) belong to whoever made them:
    // padding them here would race with their authors' own padding.
    let editor = build_null_initial(
        "pad-changed-row",
        vec![rich_text_extension().into(), table_without_scroll()],
    );
    editor
        .update(
            || {
                let table = create_table_node();
                for width in [3, 2, 3] {
                    table.append(&[row_of(width)])?;
                }
                let root = get_root();
                root.clear()?;
                root.append(&[table, create_paragraph_node()])
            },
            discrete_skip_transforms(),
        )
        .unwrap();
    assert_eq!(editor.read(cells_per_row).unwrap(), "3,2,3");
    editor
        .update(
            || {
                let last = root_table().get_last_child().filter(is_table_row_node);
                let last = last.expect("expected a row");
                last.get_last_child_or_throw()?.remove(false)
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(editor.read(cells_per_row).unwrap(), "3,2,3");
    editor.dispose();
}

fn an_update_that_adds_a_row_pads_every_short_row(in_transform: bool) {
    let editor = build_null_initial(
        "pad-after-row-added",
        vec![rich_text_extension().into(), table_without_scroll()],
    );
    let first = Rc::new(RefCell::new(String::new()));
    let armed = Rc::new(Cell::new(false));
    // Registered before the table exists, so that registering it does not
    // run the transforms on the ragged table.
    let (f, a) = (first.clone(), armed.clone());
    let remove = editor.register_node_transform("tablerow", move |row| {
        if a.get() && *row.get_key() == *f.borrow() {
            a.set(false);
            row.insert_before(&row_of(3), true)?;
        }
        Ok(())
    });
    let f = first.clone();
    editor
        .update(
            move || {
                let table = create_table_node();
                for width in [3, 3, 2] {
                    table.append(&[row_of(width)])?;
                }
                let root = get_root();
                root.clear()?;
                root.append(&[table.clone(), create_paragraph_node()])?;
                *f.borrow_mut() = table.get_first_child_or_throw()?.get_key().to_owned();
                Ok(())
            },
            discrete_skip_transforms(),
        )
        .unwrap();
    assert_eq!(editor.read(cells_per_row).unwrap(), "3,3,2");
    armed.set(in_transform);
    editor
        .update(
            move || {
                let row = root_table().get_first_child().filter(is_table_row_node);
                let row = row.expect("expected a row");
                row.get_last_child_or_throw()?.remove(false)?;
                if !in_transform {
                    row.insert_before(&row_of(3), true)?;
                }
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    remove();
    assert_eq!(editor.read(cells_per_row).unwrap(), "3,3,3,3");
    editor.dispose();
}

// upstream: lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::an update that adds a row pads every short row, also when a row transform adds it after rows changed (in a transform: false)
#[test]
fn an_update_that_adds_a_row_pads_every_short_row_not_in_a_transform() {
    an_update_that_adds_a_row_pads_every_short_row(false);
}

// upstream: lexical-table/src/__tests__/unit/PasteAcrossCellsPadsRows.test.ts::an update that adds a row pads every short row, also when a row transform adds it after rows changed (in a transform: true)
#[test]
fn an_update_that_adds_a_row_pads_every_short_row_in_a_transform() {
    an_update_that_adds_a_row_pads_every_short_row(true);
}

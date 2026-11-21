//! Ports of `packages/lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx`.
//!
//! `<TablePlugin hasHorizontalScroll>` is `TableExtension` with that
//! config (and `RichTextExtension`, which the React test env's rich text
//! plugin stands for). Where upstream reads the mounted editor's DOM, these
//! render the committed document with each node's `createDOM` and the
//! attributes the reconciler adds (`support_b::render_dom`), and
//! `getCellNodeFromCords(x, y, DOMTable)` reads the table map.

use super::support_b::{
    TableHtmlGenerator, build_table_editor, cell_at, empty_table_contents_html,
    expect_html_to_be_equal, outer_html, rendered_children, rendered_html, table_unit_test_editor,
    theme_config, wrap_table_html,
};
use crate::extension::{LexicalEditorWithDispose, get_extension_dependency_from_editor};
use crate::html::{generate_html_from_nodes, generate_nodes_from_dom};
use crate::rich_text::rich_text_extension;
use crate::selection_insert_nodes::select_all;
use crate::table::{
    InsertTableCommandPayloadHeaders, create_default_table_cell_node, create_table_node,
    create_table_node_with_dimensions, create_table_row_node, create_table_selection_from,
    insert_table_column_at_selection, is_scrollable_tables_active, is_table_cell_node,
    is_table_node, is_table_row_node, table_extension,
};
use crate::upstream_tests::support::{json_to_equal, panics, update};
use crate::utils::dfs;
use crate::{
    CUT_COMMAND, ClipboardEvent, ClipboardOrKeyboardEvent, DataTransfer, Direction, Editor,
    EditorConfig, EditorReadMode, LexicalError, NodeRef, create_paragraph_node, create_text_node,
    get_root, get_selection, set_selection,
};
use std::rc::Rc;

const THEME: &str = r#"{"table":"test-table-class","tableAlignment":{"center":"test-table-alignment-center","right":"test-table-alignment-right"},"tableFrozenColumn":"test-table-frozen-column-class","tableFrozenRow":"test-table-frozen-row-class","tableRowStriping":"test-table-row-striping-class","tableScrollableWrapper":"table-scrollable-wrapper"}"#;

fn editor_config() -> EditorConfig {
    theme_config(THEME)
}

/// The editor of `describe('hasHorizontalScroll={...}')`, after its
/// `beforeEach`: a selected empty paragraph. The test env's root starts
/// empty, where `RichTextExtension` gives it a paragraph, so the root is
/// cleared first.
fn editor(has_horizontal_scroll: bool) -> LexicalEditorWithDispose {
    let editor = build_table_editor(
        "table-node-test",
        has_horizontal_scroll,
        THEME,
        vec![rich_text_extension().into()],
    );
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.select(None, None).map(drop)
    });
    editor
}

/// `expectTableHtmlToBeEqual(actual, expected)`.
#[track_caller]
fn expect_table_html_to_be_equal(scroll: bool, actual: &str, expected: &str) {
    if scroll {
        expect_html_to_be_equal(actual, &wrap_table_html(expected));
    } else {
        expect_html_to_be_equal(actual, expected);
    }
}

/// `expectReconciledTableHtmlToBeEqual(actual, expected, dir)`.
#[track_caller]
fn expect_reconciled_table_html_to_be_equal(scroll: bool, actual: &str, expected: &str, dir: &str) {
    expect_table_html_to_be_equal(
        scroll,
        actual,
        &expected.replace("<table", &format!(r#"<table dir="{dir}""#)),
    );
}

fn last_table() -> NodeRef {
    let table = get_root().get_last_child().expect("a child");
    assert!(is_table_node(&table));
    table
}

/// `createDOM(editorConfig).outerHTML` of the root's last child, a table,
/// in a `'force-commit'` read.
fn last_table_create_dom(editor: &Editor) -> String {
    let e = editor.clone();
    editor
        .read_with_mode(EditorReadMode::ForceCommit, move || {
            outer_html(&last_table(), &editor_config(), &e)
        })
        .unwrap()
}

fn append_table(rows: usize, columns: usize) -> impl FnOnce() -> Result<(), LexicalError> {
    move || {
        let table = create_table_node_with_dimensions(
            rows,
            columns,
            InsertTableCommandPayloadHeaders::Both(true),
        )?;
        get_root().append(&[table])
    }
}

/// `$assertNodeType(cell.getLastChild(), $isParagraphNode).append(text)`.
fn append_to_cell(table: &NodeRef, x: usize, y: usize, text: &str) -> Result<(), LexicalError> {
    let paragraph = cell_at(table, x, y)
        .and_then(|c| c.get_last_child())
        .expect("a paragraph");
    assert_eq!(paragraph.get_type(), "paragraph");
    paragraph.append(&[create_text_node(text)])
}

fn cut(editor: &Editor) {
    editor
        .dispatch_command(
            &CUT_COMMAND,
            Some(ClipboardOrKeyboardEvent::Clipboard(Rc::new(
                ClipboardEvent::new("cut", Some(Rc::new(DataTransfer::new()))),
            ))),
        )
        .unwrap();
    // JS's cut removes the selection after the copy settles; a read
    // flushes the queued update.
    editor.read(|| ()).unwrap();
}

fn table_node_constructor(scroll: bool) {
    let editor = editor(scroll);
    update(&editor, || {
        let table_node = create_table_node();
        assert_eq!(table_node.get_type(), "table");
        Ok(())
    });
    assert!(panics(create_table_node));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > TableNode.constructor
#[test]
fn no_scroll_table_node_constructor() {
    table_node_constructor(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > TableNode.constructor
#[test]
fn scroll_table_node_constructor() {
    table_node_constructor(true);
}

fn table_node_create_dom(scroll: bool) {
    let editor = editor(scroll);
    let e = (*editor).clone();
    update(&editor, move || {
        let table_node = create_table_node();
        expect_table_html_to_be_equal(
            scroll,
            &outer_html(&table_node, &editor_config(), &e),
            r#"<table class="test-table-class"></table>"#,
        );
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > TableNode.createDOM()
#[test]
fn no_scroll_table_node_create_dom() {
    table_node_create_dom(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > TableNode.createDOM()
#[test]
fn scroll_table_node_create_dom() {
    table_node_create_dom(true);
}

fn table_node_create_dom_and_update_dom_style(scroll: bool) {
    let editor = editor(scroll);
    let create_small_table = || -> Result<NodeRef, LexicalError> {
        let cell = create_default_table_cell_node();
        cell.append(&[create_paragraph_node()])?;
        let row = create_table_row_node(None);
        row.append(&[cell])?;
        let table = create_table_node();
        table.append(&[row])?;
        Ok(table)
    };
    update(&editor, move || {
        let root = get_root();
        root.clear()?;
        let styled = create_small_table()?;
        styled.set_element_style("background-color: blue;");
        root.append(&[create_small_table()?, styled])?;
        set_selection(None);
        Ok(())
    });
    let contents = empty_table_contents_html(1, 1, TableHtmlGenerator::Plain);
    let tables = rendered_children(&editor);
    assert_eq!(tables.len(), 2);
    expect_reconciled_table_html_to_be_equal(
        scroll,
        &tables[0],
        &format!(r#"<table class="test-table-class">{contents}</table>"#),
        "auto",
    );
    expect_reconciled_table_html_to_be_equal(
        scroll,
        &tables[1],
        &format!(
            r#"<table class="test-table-class" style="background-color: blue">{contents}</table>"#
        ),
        "auto",
    );
    update(&editor, || {
        for (i, node) in get_root().get_children().iter().enumerate() {
            if is_table_node(node) {
                node.set_element_style(&format!("--table-index: {i};"));
            }
        }
        Ok(())
    });
    let tables = rendered_children(&editor);
    assert_eq!(tables.len(), 2);
    for (i, table) in tables.iter().enumerate() {
        expect_reconciled_table_html_to_be_equal(
            scroll,
            table,
            &format!(
                r#"<table class="test-table-class" style="--table-index: {i}">{contents}</table>"#
            ),
            "auto",
        );
    }
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > TableNode.createDOM() and updateDOM() style
#[test]
fn no_scroll_table_node_create_dom_and_update_dom_style() {
    table_node_create_dom_and_update_dom_style(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > TableNode.createDOM() and updateDOM() style
#[test]
fn scroll_table_node_create_dom_and_update_dom_style() {
    table_node_create_dom_and_update_dom_style(true);
}

const HEADER_CELL_STYLE: &str = "border: 1px solid black; width: 75px; vertical-align: top; text-align: start; background-color: rgb(242, 243, 245);";
const CELL_STYLE: &str =
    "border: 1px solid black; width: 75px; vertical-align: top; text-align: start;";

fn cell_html(tag: &str, text: &str) -> String {
    let style = if tag == "th" {
        HEADER_CELL_STYLE
    } else {
        CELL_STYLE
    };
    format!(
        r#"<{tag} style="{style}"><p><span style="white-space: pre-wrap">{text}</span></p></{tag}>"#
    )
}

/// A 2x2 table with headers, column widths `[100, 200]` and its text
/// nodes numbered.
fn numbered_table() -> Result<NodeRef, LexicalError> {
    let table_node =
        create_table_node_with_dimensions(2, 2, InsertTableCommandPayloadHeaders::Both(true))?;
    table_node.set_col_widths(Some(vec![100.0, 200.0]));
    for (i, node) in table_node.get_all_text_nodes().iter().enumerate() {
        node.set_text_content(&i.to_string());
    }
    Ok(table_node)
}

fn table_node_export_dom_with_range_selection(scroll: bool) {
    let editor = editor(scroll);
    let e = (*editor).clone();
    update(&editor, move || {
        let table_node = numbered_table()?;
        let root = get_root();
        root.clear()?;
        root.append(&[table_node])?;
        let selection = root.select(Some(0), None)?;
        expect_html_to_be_equal(
            &generate_html_from_nodes(&e, Some(&selection))?,
            &format!(
                r#"<table class="test-table-class"><colgroup><col style="width: 100px" /><col style="width: 200px" /></colgroup><tbody><tr>{}{}</tr><tr>{}{}</tr></tbody></table>"#,
                cell_html("th", "0"),
                cell_html("th", "1"),
                cell_html("th", "2"),
                cell_html("td", "3"),
            ),
        );
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > TableNode.exportDOM() with range selection
#[test]
fn no_scroll_table_node_export_dom_with_range_selection() {
    table_node_export_dom_with_range_selection(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > TableNode.exportDOM() with range selection
#[test]
fn scroll_table_node_export_dom_with_range_selection() {
    table_node_export_dom_with_range_selection(true);
}

fn table_node_export_dom_with_partial_table_selection(scroll: bool) {
    let editor = editor(scroll);
    let e = (*editor).clone();
    update(&editor, move || {
        let table_node = numbered_table()?;
        get_root().append(std::slice::from_ref(&table_node))?;
        let cells: Vec<NodeRef> = dfs(Some(&table_node), None)
            .into_iter()
            .map(|n| n.node)
            .filter(is_table_cell_node)
            .collect();
        // The second column.
        let table_selection = create_table_selection_from(&table_node, &cells[1], &cells[3])?;
        expect_html_to_be_equal(
            &generate_html_from_nodes(&e, Some(&table_selection))?,
            &format!(
                r#"<table class="test-table-class"><colgroup><col style="width: 200px" /></colgroup><tbody><tr>{}</tr><tr>{}</tr></tbody></table>"#,
                cell_html("th", "1"),
                cell_html("td", "3"),
            ),
        );
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > TableNode.exportDOM() with partial table selection
#[test]
fn no_scroll_table_node_export_dom_with_partial_table_selection() {
    table_node_export_dom_with_partial_table_selection(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > TableNode.exportDOM() with partial table selection
#[test]
fn scroll_table_node_export_dom_with_partial_table_selection() {
    table_node_export_dom_with_partial_table_selection(true);
}

/// `$insertDataTransferForRichText` of `text/html` at the selection.
fn paste_html(editor: &Editor, html: &'static str) {
    let e = editor.clone();
    update(editor, move || {
        let data_transfer = DataTransfer::new();
        data_transfer.set_data("text/html", html);
        let selection = get_selection().expect("a selection");
        assert!(selection.is_range(), "isRangeSelection(selection)");
        crate::clipboard::insert_data_transfer_for_rich_text(&data_transfer, &selection, &e)
    });
}

const EXTERNAL_TABLE: &str = r#"<html><body><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-16a69100-7fff-6cb9-b829-cb1def16a58d"><div dir="ltr" style="margin-left:0pt;" align="left"><table style="border:none;border-collapse:collapse;table-layout:fixed"><colgroup><col style="width:100px"/><col style="width:200px"/></colgroup><tbody><tr style="height:22.015pt"><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;vertical-align:top;padding:5pt 5pt 5pt 5pt;overflow:hidden;overflow-wrap:break-word;"><p dir="ltr" style="line-height:1.2;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Hello there</span></p></td><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;vertical-align:top;padding:5pt 5pt 5pt 5pt;overflow:hidden;overflow-wrap:break-word;"><p dir="ltr" style="line-height:1.2;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">General Kenobi!</span></p></td></tr><tr style="height:22.015pt"><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;vertical-align:top;padding:5pt 5pt 5pt 5pt;overflow:hidden;overflow-wrap:break-word;"><p dir="ltr" style="line-height:1.2;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Lexical is nice</span></p></td><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;vertical-align:top;padding:5pt 5pt 5pt 5pt;overflow:hidden;overflow-wrap:break-word;"><br data-lexical-managed-linebreak="true"></td></tr></tbody></table></div></b><!--EndFragment--></body></html>"#;

fn copy_table_from_an_external_source(scroll: bool) {
    let editor = editor(scroll);
    paste_html(&editor, EXTERNAL_TABLE);
    // Make sure a paragraph is inserted inside empty cells.
    let empty_cell =
        r#"<td dir="auto"><p dir="auto"><br data-lexical-managed-linebreak="true"></p></td>"#;
    expect_reconciled_table_html_to_be_equal(
        scroll,
        &rendered_html(&editor),
        &format!(
            r#"<table class="test-table-class"><colgroup><col style="width: 100px;" /><col style="width: 200px;" /></colgroup><tr dir="auto"><td dir="auto"><p dir="ltr"><span data-lexical-text="true">Hello there</span></p></td><td dir="auto"><p dir="ltr"><span data-lexical-text="true">General Kenobi!</span></p></td></tr><tr dir="auto"><td dir="auto"><p dir="ltr"><span data-lexical-text="true">Lexical is nice</span></p></td>{empty_cell}</tr></table>"#
        ),
        "auto",
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Copy table from an external source
#[test]
fn no_scroll_copy_table_from_an_external_source() {
    copy_table_from_an_external_source(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Copy table from an external source
#[test]
fn scroll_copy_table_from_an_external_source() {
    copy_table_from_an_external_source(true);
}

const THEAD_TABLE: &str = r#"
<meta charset="utf-8" />
<table
  style="box-sizing: border-box; border-collapse: collapse; border: 2px solid rgb(140, 140, 140); font-family: sans-serif; font-size: 0.8rem; letter-spacing: 1px; color: rgb(21, 20, 26); font-style: normal; font-variant-ligatures: normal; font-variant-caps: normal; font-weight: 400; orphans: 2; text-align: start; text-transform: none; widows: 2; word-spacing: 0px; -webkit-text-stroke-width: 0px; white-space: normal; background-color: rgb(255, 255, 255); text-decoration-thickness: initial; text-decoration-style: initial; text-decoration-color: initial;">
  <caption
    style="box-sizing: border-box; caption-side: bottom; padding: 10px;">
    Council budget (in £) 2018
  </caption>
  <thead
    style="box-sizing: border-box; background-color: rgb(44, 94, 119); color: rgb(255, 255, 255);">
    <tr style="box-sizing: border-box;">
      <th
        scope="col"
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px;">
        Items
      </th>
      <th
        scope="col"
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px;">
        Expenditure
      </th>
    </tr>
  </thead>
  <tbody
    style="box-sizing: border-box; background-color: rgb(228, 240, 245);">
    <tr style="box-sizing: border-box;">
      <th
        scope="row"
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px;">
        Donuts
      </th>
      <td
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center;">
        3,000
      </td>
    </tr>
    <tr style="box-sizing: border-box;">
      <th
        scope="row"
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px;">
        Stationery
      </th>
      <td
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center;">
        18,000
      </td>
    </tr>
  </tbody>
  <tfoot
    style="box-sizing: border-box; background-color: rgb(44, 94, 119); color: rgb(255, 255, 255);">
    <tr style="box-sizing: border-box;">
      <th
        scope="row"
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px;">
        Totals
      </th>
      <td
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center;">
        21,000
      </td>
    </tr>
  </tfoot>
</table>
"#;

fn copy_table_with_caption_tbody_thead_tfoot_from_an_external_source(scroll: bool) {
    let editor = editor(scroll);
    paste_html(&editor, THEAD_TABLE);
    let header = |text: &str| {
        format!(
            r#"<th dir="auto"><p dir="auto"><span data-lexical-text="true">{text}</span></p></th>"#
        )
    };
    let centered = |text: &str| {
        format!(
            r#"<td dir="auto"><p dir="auto" style="text-align: center;"><span data-lexical-text="true">{text}</span></p></td>"#
        )
    };
    let row = |cells: String| format!(r#"<tr dir="auto" style="text-align: start">{cells}</tr>"#);
    // createDOM, not exportDOM, so there is no tbody.
    expect_reconciled_table_html_to_be_equal(
        scroll,
        &rendered_html(&editor),
        &format!(
            r#"<table class="test-table-class">{}{}{}{}</table>"#,
            row(header("Items") + &header("Expenditure")),
            row(header("Donuts") + &centered("3,000")),
            row(header("Stationery") + &centered("18,000")),
            row(header("Totals") + &centered("21,000")),
        ),
        "auto",
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Copy table with caption/tbody/thead/tfoot from an external source
#[test]
fn no_scroll_copy_table_with_caption_tbody_thead_tfoot_from_an_external_source() {
    copy_table_with_caption_tbody_thead_tfoot_from_an_external_source(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Copy table with caption/tbody/thead/tfoot from an external source
#[test]
fn scroll_copy_table_with_caption_tbody_thead_tfoot_from_an_external_source() {
    copy_table_with_caption_tbody_thead_tfoot_from_an_external_source(true);
}

const CAPTION_TABLE: &str = r#"
<meta charset="utf-8" />
<table
  style="box-sizing: border-box; border-collapse: collapse; border: 2px solid rgb(140, 140, 140); font-family: sans-serif; font-size: 0.8rem; letter-spacing: 1px; color: rgb(21, 20, 26); font-style: normal; font-variant-ligatures: normal; font-variant-caps: normal; font-weight: 400; orphans: 2; text-align: start; text-transform: none; widows: 2; word-spacing: 0px; -webkit-text-stroke-width: 0px; white-space: normal; background-color: rgb(255, 255, 255); text-decoration-thickness: initial; text-decoration-style: initial; text-decoration-color: initial;">
  <caption
    style="box-sizing: border-box; caption-side: bottom; padding: 10px; font-weight: bold;">
    He-Man and Skeletor facts
  </caption>
  <tbody style="box-sizing: border-box;">
    <tr style="box-sizing: border-box;">
      <td
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(240, 240, 240);"></td>
      <th
        class="heman"
        scope="col"
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; background-color: rgb(230, 230, 230); font: 1.4rem molot; text-shadow: rgb(255, 255, 255) 1px 1px 1px, rgb(0, 0, 0) 2px 2px 1px;">
        He-Man
      </th>
      <th
        class="skeletor"
        scope="col"
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; background-color: rgb(230, 230, 230); font: 1.7rem rapscallion; letter-spacing: 3px; text-shadow: rgb(255, 255, 255) 1px 1px 0px, rgb(0, 0, 0) 0px 0px 9px;">
        Skeletor
      </th>
    </tr>
    <tr style="box-sizing: border-box;">
      <th
        scope="row"
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; background-color: rgb(230, 230, 230);">
        Role
      </th>
      <td
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(250, 250, 250);">
        Hero
      </td>
      <td
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(250, 250, 250);">
        Villain
      </td>
    </tr>
    <tr style="box-sizing: border-box;">
      <th
        scope="row"
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; background-color: rgb(230, 230, 230);">
        Weapon
      </th>
      <td
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(240, 240, 240);">
        Power Sword
      </td>
      <td
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(240, 240, 240);">
        Havoc Staff
      </td>
    </tr>
    <tr style="box-sizing: border-box;">
      <th
        scope="row"
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; background-color: rgb(230, 230, 230);">
        Dark secret
      </th>
      <td
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(250, 250, 250);">
        Expert florist
      </td>
      <td
        style="box-sizing: border-box; border: 1px solid rgb(160, 160, 160); padding: 8px 10px; text-align: center; background-color: rgb(250, 250, 250);">
        Cries at romcoms
      </td>
    </tr>
  </tbody>
</table>
"#;

fn copy_table_with_caption_from_an_external_source(scroll: bool) {
    let editor = editor(scroll);
    paste_html(&editor, CAPTION_TABLE);
    let header = |text: &str| {
        format!(
            r#"<th dir="auto" style="background-color: rgb(230, 230, 230)"><p dir="auto"><span data-lexical-text="true">{text}</span></p></th>"#
        )
    };
    let cell = |bg: &str, text: &str| {
        format!(
            r#"<td dir="auto" style="background-color: rgb({bg})"><p dir="auto" style="text-align: center"><span data-lexical-text="true">{text}</span></p></td>"#
        )
    };
    let row = |cells: String| format!(r#"<tr dir="auto" style="text-align: start">{cells}</tr>"#);
    let first = r#"<td dir="auto" style="background-color: rgb(240, 240, 240)"><p dir="auto" style="text-align: center"><br data-lexical-managed-linebreak="true" /></p></td>"#;
    let light = "250, 250, 250";
    let dark = "240, 240, 240";
    expect_reconciled_table_html_to_be_equal(
        scroll,
        &rendered_html(&editor),
        &format!(
            r#"<table class="test-table-class">{}{}{}{}</table>"#,
            row(first.to_owned() + &header("He-Man") + &header("Skeletor")),
            row(header("Role") + &cell(light, "Hero") + &cell(light, "Villain")),
            row(header("Weapon") + &cell(dark, "Power Sword") + &cell(dark, "Havoc Staff")),
            row(header("Dark secret")
                + &cell(light, "Expert florist")
                + &cell(light, "Cries at romcoms")),
        ),
        "auto",
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Copy table with caption from an external source
#[test]
fn no_scroll_copy_table_with_caption_from_an_external_source() {
    copy_table_with_caption_from_an_external_source(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Copy table with caption from an external source
#[test]
fn scroll_copy_table_with_caption_from_an_external_source() {
    copy_table_with_caption_from_an_external_source(true);
}

const GDOC_TABLE: &str = r#"<google-sheets-html-origin><style type="text/css"><!--td {border: 1px solid #cccccc;}br {mso-data-placement:same-cell;}--></style><table xmlns="http://www.w3.org/1999/xhtml" cellspacing="0" cellpadding="0" dir="ltr" border="1" style="table-layout:fixed;font-size:10pt;font-family:Arial;width:0px;border-collapse:collapse;border:none" data-sheets-root="1"><colgroup><col width="100"/><col width="189"/><col width="171"/></colgroup><tbody><tr style="height:21px;"><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;font-weight:bold;" data-sheets-value="{&quot;1&quot;:2,&quot;2&quot;:&quot;Surface&quot;}">Surface</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;font-style:italic;" data-sheets-value="{&quot;1&quot;:2,&quot;2&quot;:&quot;MWP_WORK_LS_COMPOSER&quot;}">MWP_WORK_LS_COMPOSER</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;text-decoration:underline;text-align:right;" data-sheets-value="{&quot;1&quot;:3,&quot;3&quot;:77349}">77349</td></tr><tr style="height:21px;"><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;" data-sheets-value="{&quot;1&quot;:2,&quot;2&quot;:&quot;Lexical&quot;}">Lexical</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;text-decoration:line-through;" data-sheets-value="{&quot;1&quot;:2,&quot;2&quot;:&quot;XDS_RICH_TEXT_AREA&quot;}">XDS_RICH_TEXT_AREA</td><td style="overflow:hidden;padding:2px 3px 2px 3px;vertical-align:bottom;" data-sheets-value="{&quot;1&quot;:2,&quot;2&quot;:&quot;sdvd sdfvsfs&quot;}" data-sheets-textstyleruns="{&quot;1&quot;:0}{&quot;1&quot;:5,&quot;2&quot;:{&quot;5&quot;:1}}"><span style="font-size:10pt;font-family:Arial;font-style:normal;">sdvd </span><span style="font-size:10pt;font-family:Arial;font-weight:bold;font-style:normal;">sdfvsfs</span></td></tr></tbody></table>"#;

fn copy_table_from_an_external_source_like_gdoc_with_formatting(scroll: bool) {
    let editor = editor(scroll);
    paste_html(&editor, GDOC_TABLE);
    let cell =
        |inner: &str| format!(r#"<td dir="auto" style="vertical-align: bottom">{inner}</td>"#);
    // The pasted table carries dir="ltr", which importDOM preserves, so the
    // table is ltr and its rows don't bidi-auto-detect.
    expect_reconciled_table_html_to_be_equal(
        scroll,
        &rendered_html(&editor),
        &format!(
            r#"<table class="test-table-class"><colgroup><col style="width: 100px" /><col style="width: 189px" /><col style="width: 171px" /></colgroup><tr style="height: 21px;">{}{}{}</tr><tr style="height: 21px;">{}{}{}</tr></table>"#,
            cell(r#"<p dir="auto"><strong data-lexical-text="true">Surface</strong></p>"#),
            cell(r#"<p dir="auto"><em data-lexical-text="true">MWP_WORK_LS_COMPOSER</em></p>"#),
            cell(
                r#"<p dir="auto" style="text-align: right;"><span data-lexical-text="true">77349</span></p>"#
            ),
            cell(r#"<p dir="auto"><span data-lexical-text="true">Lexical</span></p>"#),
            cell(r#"<p dir="auto"><span data-lexical-text="true">XDS_RICH_TEXT_AREA</span></p>"#),
            cell(
                r#"<p dir="auto"><span data-lexical-text="true">sdvd</span><strong data-lexical-text="true">sdfvsfs</strong></p>"#
            ),
        ),
        "ltr",
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Copy table from an external source like gdoc with formatting
#[test]
fn no_scroll_copy_table_from_an_external_source_like_gdoc_with_formatting() {
    copy_table_from_an_external_source_like_gdoc_with_formatting(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Copy table from an external source like gdoc with formatting
#[test]
fn scroll_copy_table_from_an_external_source_like_gdoc_with_formatting() {
    copy_table_from_an_external_source_like_gdoc_with_formatting(true);
}

const EMPTY_PARAGRAPH: &str = r#"<p dir="auto"><br data-lexical-managed-linebreak="true" /></p>"#;

/// Appends `before`, a 4x4 table and `after` to the paragraph, selects
/// all and cuts.
fn cut_table_in_range_selection(
    scroll: bool,
    before: Option<&'static str>,
    after: Option<&'static str>,
) {
    let editor = editor(scroll);
    update(&editor, move || {
        let paragraph = get_root().get_first_child().expect("a paragraph");
        assert_eq!(paragraph.get_type(), "paragraph");
        if let Some(before) = before {
            paragraph.append(&[create_text_node(before)])?;
        }
        paragraph.append(&[create_table_node_with_dimensions(
            4,
            4,
            InsertTableCommandPayloadHeaders::Both(true),
        )?])?;
        if let Some(after) = after {
            paragraph.append(&[create_text_node(after)])?;
        }
        Ok(())
    });
    let e = (*editor).clone();
    update(&editor, move || {
        e.focus(None, Default::default())?;
        select_all().map(drop)
    });
    cut(&editor);
    expect_html_to_be_equal(&rendered_html(&editor), EMPTY_PARAGRAPH);
}

fn cut_table_in_the_middle_of_a_range_selection(scroll: bool) {
    cut_table_in_range_selection(
        scroll,
        Some("text before the table"),
        Some("text after the table"),
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Cut table in the middle of a range selection
#[test]
fn no_scroll_cut_table_in_the_middle_of_a_range_selection() {
    cut_table_in_the_middle_of_a_range_selection(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Cut table in the middle of a range selection
#[test]
fn scroll_cut_table_in_the_middle_of_a_range_selection() {
    cut_table_in_the_middle_of_a_range_selection(true);
}

fn cut_table_as_last_node_in_range_selection(scroll: bool) {
    cut_table_in_range_selection(scroll, Some("text before the table"), None);
}

// upstream: "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Cut table as last node in range selection "
#[test]
fn no_scroll_cut_table_as_last_node_in_range_selection() {
    cut_table_as_last_node_in_range_selection(false);
}

// upstream: "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Cut table as last node in range selection "
#[test]
fn scroll_cut_table_as_last_node_in_range_selection() {
    cut_table_as_last_node_in_range_selection(true);
}

fn cut_table_as_first_node_in_range_selection(scroll: bool) {
    cut_table_in_range_selection(scroll, None, Some("text after the table"));
}

// upstream: "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Cut table as first node in range selection "
#[test]
fn no_scroll_cut_table_as_first_node_in_range_selection() {
    cut_table_as_first_node_in_range_selection(false);
}

// upstream: "lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Cut table as first node in range selection "
#[test]
fn scroll_cut_table_as_first_node_in_range_selection() {
    cut_table_as_first_node_in_range_selection(true);
}

/// Appends a 4x4 table, writes `some text` in its first cell, selects the
/// cells from (0, 0) to (`to`, `to`) and cuts.
fn cut_table_cells(scroll: bool, to: usize) -> LexicalEditorWithDispose {
    let editor = editor(scroll);
    update(&editor, append_table(4, 4));
    update(&editor, move || {
        let table = last_table();
        append_to_cell(&table, 0, 0, "some text")?;
        let selection = create_table_selection_from(
            &table,
            &cell_at(&table, 0, 0).unwrap(),
            &cell_at(&table, to, to).unwrap(),
        )?;
        set_selection(Some(selection));
        Ok(())
    });
    cut(&editor);
    editor
}

fn cut_table_is_whole_selection_should_remove_it(scroll: bool) {
    let editor = cut_table_cells(scroll, 3);
    expect_html_to_be_equal(&rendered_html(&editor), EMPTY_PARAGRAPH);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Cut table is whole selection, should remove it
#[test]
fn no_scroll_cut_table_is_whole_selection_should_remove_it() {
    cut_table_is_whole_selection_should_remove_it(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Cut table is whole selection, should remove it
#[test]
fn scroll_cut_table_is_whole_selection_should_remove_it() {
    cut_table_is_whole_selection_should_remove_it(true);
}

fn cut_subsection_of_table_cells_should_just_clear_contents(scroll: bool) {
    let editor = cut_table_cells(scroll, 2);
    expect_reconciled_table_html_to_be_equal(
        scroll,
        &rendered_html(&editor),
        &format!(
            r#"{EMPTY_PARAGRAPH}<table class="test-table-class">{}</table>"#,
            empty_table_contents_html(4, 4, TableHtmlGenerator::Heading)
        ),
        "auto",
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Cut subsection of table cells, should just clear contents
#[test]
fn no_scroll_cut_subsection_of_table_cells_should_just_clear_contents() {
    cut_subsection_of_table_cells_should_just_clear_contents(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Cut subsection of table cells, should just clear contents
#[test]
fn scroll_cut_subsection_of_table_cells_should_just_clear_contents() {
    cut_subsection_of_table_cells_should_just_clear_contents(true);
}

fn table_plain_text_output_validation(scroll: bool) {
    let editor = editor(scroll);
    update(&editor, append_table(4, 4));
    update(&editor, || {
        let table = last_table();
        append_to_cell(&table, 0, 0, "1")?;
        append_to_cell(&table, 1, 0, "")?;
        append_to_cell(&table, 2, 0, "2")?;
        append_to_cell(&table, 0, 1, "3")?;
        append_to_cell(&table, 1, 1, "4")?;
        append_to_cell(&table, 2, 1, "")?;
        let selection = create_table_selection_from(
            &table,
            &cell_at(&table, 0, 0).unwrap(),
            &cell_at(&table, 2, 1).unwrap(),
        )?;
        assert_eq!(selection.get_text_content()?, "1\t\t2\n3\t4\t\n");
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Table plain text output validation
#[test]
fn no_scroll_table_plain_text_output_validation() {
    table_plain_text_output_validation(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Table plain text output validation
#[test]
fn scroll_table_plain_text_output_validation() {
    table_plain_text_output_validation(true);
}

fn toggle_row_striping_on_off(scroll: bool) {
    let editor = editor(scroll);
    update(&editor, append_table(4, 4));
    update(&editor, || {
        last_table().set_row_striping(true);
        Ok(())
    });
    expect_table_html_to_be_equal(
        scroll,
        &last_table_create_dom(&editor),
        r#"<table class="test-table-class test-table-row-striping-class" data-lexical-row-striping="true"></table>"#,
    );
    update(&editor, || {
        last_table().set_row_striping(false);
        Ok(())
    });
    expect_table_html_to_be_equal(
        scroll,
        &last_table_create_dom(&editor),
        r#"<table class="test-table-class"></table>"#,
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Toggle row striping ON/OFF
#[test]
fn no_scroll_toggle_row_striping_on_off() {
    toggle_row_striping_on_off(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Toggle row striping ON/OFF
#[test]
fn scroll_toggle_row_striping_on_off() {
    toggle_row_striping_on_off(true);
}

/// The legacy JSON of a cell with an empty paragraph.
fn cell_json(header_state: i32) -> String {
    format!(
        r#"{{"backgroundColor":null,"children":[{{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}}],"colSpan":1,"direction":null,"format":"","headerState":{header_state},"indent":0,"rowSpan":1,"type":"tablecell","version":1}}"#
    )
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Toggle frozen first column ON/OFF
#[test]
fn scroll_toggle_frozen_first_column_on_off() {
    let editor = editor(true);
    update(&editor, append_table(4, 4));
    update(&editor, || {
        last_table().set_frozen_columns(1.0);
        Ok(())
    });
    expect_html_to_be_equal(
        &last_table_create_dom(&editor),
        r#"<div class="table-scrollable-wrapper test-table-frozen-column-class"><table class="test-table-class" data-lexical-frozen-column="true"></table></div>"#,
    );
    let rows: Vec<String> = (0..4)
        .map(|row| {
            let cells: Vec<String> = (0..4)
                .map(|col| {
                    cell_json(match (row, col) {
                        (0, 0) => 3,
                        (0, _) => 1,
                        (_, 0) => 2,
                        _ => 0,
                    })
                })
                .collect();
            format!(
                r#"{{"children":[{}],"direction":null,"format":"","indent":0,"type":"tablerow","version":1}}"#,
                cells.join(",")
            )
        })
        .collect();
    let expected = format!(
        r#"{{"root":{{"children":[{{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}},{{"children":[{}],"direction":null,"format":"","frozenColumnCount":1,"indent":0,"type":"table","version":1}}],"direction":null,"format":"","indent":0,"type":"root","version":1}}}}"#,
        rows.join(",")
    );
    let actual = editor.get_editor_state().to_json(false);
    let expected = crate::json::parse(&expected).unwrap();
    assert!(
        json_to_equal(&actual, &expected),
        "{actual}\n!=\n{expected}"
    );
    update(&editor, || {
        last_table().set_frozen_columns(0.0);
        Ok(())
    });
    expect_html_to_be_equal(
        &last_table_create_dom(&editor),
        r#"<div class="table-scrollable-wrapper"><table class="test-table-class"></table></div>"#,
    );
}

fn change_table_level_alignment(scroll: bool) {
    let editor = editor(scroll);
    update(&editor, append_table(4, 4));
    update(&editor, || {
        last_table().set_element_format("center");
        Ok(())
    });
    expect_table_html_to_be_equal(
        scroll,
        &last_table_create_dom(&editor),
        r#"<table class="test-table-class test-table-alignment-center"></table>"#,
    );
    update(&editor, || {
        last_table().set_element_format("left");
        Ok(())
    });
    expect_table_html_to_be_equal(
        scroll,
        &last_table_create_dom(&editor),
        r#"<table class="test-table-class"></table>"#,
    );
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Change Table-level alignment
#[test]
fn no_scroll_change_table_level_alignment() {
    change_table_level_alignment(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Change Table-level alignment
#[test]
fn scroll_change_table_level_alignment() {
    change_table_level_alignment(true);
}

/// The mounted DOM of the root's last child, a table
/// (`editor.getElementByKey(table.__key).outerHTML`), in a `'force-commit'`
/// read.
fn last_table_element(editor: &Editor) -> String {
    editor
        .read_with_mode(EditorReadMode::ForceCommit, || ())
        .unwrap();
    rendered_children(editor).pop().expect("a table")
}

fn update_column_widths(scroll: bool) {
    let editor = editor(scroll);
    update(&editor, append_table(4, 2));
    expect_reconciled_table_html_to_be_equal(
        scroll,
        &last_table_element(&editor),
        &format!(
            r#"<table class="test-table-class">{}</table>"#,
            empty_table_contents_html(4, 2, TableHtmlGenerator::Heading)
        ),
        "auto",
    );
    // Set widths.
    update(&editor, || {
        last_table().set_col_widths(Some(vec![50.0, 50.0]));
        Ok(())
    });
    expect_reconciled_table_html_to_be_equal(
        scroll,
        &last_table_element(&editor),
        &format!(
            r#"<table class="test-table-class"><colgroup><col style="width: 50px;" /><col style="width: 50px;" /></colgroup>{}</table>"#,
            empty_table_contents_html(4, 2, TableHtmlGenerator::Heading)
        ),
        "auto",
    );
    // `getColWidths()` returns a copy, so it can't be changed through it
    // (where JS freezes the array in DEV).
    editor
        .read(|| {
            let table = last_table();
            assert_eq!(table.get_col_widths(), Some(vec![50.0, 50.0]));
            assert_eq!(table.get_column_count(), 2);
        })
        .unwrap();
    // Add a column.
    update(&editor, || {
        let table = last_table();
        let cell = cell_at(&table, 0, 0).unwrap();
        let selection = create_table_selection_from(&table, &cell, &cell)?;
        set_selection(Some(selection));
        insert_table_column_at_selection(true)?;
        table.set_col_widths(Some(vec![50.0, 50.0, 100.0]));
        Ok(())
    });
    expect_reconciled_table_html_to_be_equal(
        scroll,
        &last_table_element(&editor),
        &format!(
            r#"<table class="test-table-class"><colgroup><col style="width: 50px;" /><col style="width: 50px;" /><col style="width: 100px;" /></colgroup>{}</table>"#,
            empty_table_contents_html(4, 3, TableHtmlGenerator::Heading)
        ),
        "auto",
    );
    editor
        .read(|| {
            let table = last_table();
            assert_eq!(table.get_col_widths(), Some(vec![50.0, 50.0, 100.0]));
            assert_eq!(table.get_column_count(), 3);
        })
        .unwrap();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={false} > Update column widths
#[test]
fn no_scroll_update_column_widths() {
    update_column_widths(false);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll={true} > Update column widths
#[test]
fn scroll_update_column_widths() {
    update_column_widths(true);
}

/// `setHasHorizontalScroll(value)`: the `TableExtension` output's
/// `hasHorizontalScroll` signal, which the React wrapper's state feeds.
fn set_has_horizontal_scroll(editor: &Editor, value: bool) {
    get_extension_dependency_from_editor(editor, &table_extension())
        .unwrap()
        .output
        .expect("TableExtension has a build")
        .has_horizontal_scroll
        .set_value(value);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > hasHorizontalScroll false -> true > table is re-rendered when scroll changes
#[test]
fn table_is_re_rendered_when_scroll_changes() {
    let editor = build_table_editor(
        "table-node-scroll-test",
        false,
        THEME,
        vec![rich_text_extension().into()],
    );
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[create_table_node_with_dimensions(
            2,
            2,
            InsertTableCommandPayloadHeaders::Both(true),
        )?])
    });
    let contents = empty_table_contents_html(2, 2, TableHtmlGenerator::Heading);
    let unwrapped = format!(r#"<table class="test-table-class" dir="auto">{contents}</table>"#);
    expect_html_to_be_equal(&rendered_html(&editor), &unwrapped);
    let e = (*editor).clone();
    assert!(!editor.read(|| is_scrollable_tables_active(&e)).unwrap());
    set_has_horizontal_scroll(&editor, true);
    assert!(editor.read(|| is_scrollable_tables_active(&e)).unwrap());
    expect_html_to_be_equal(
        &rendered_html(&editor),
        &format!(
            r#"<div class="table-scrollable-wrapper" dir="auto"><table class="test-table-class">{contents}</table></div>"#
        ),
    );
    set_has_horizontal_scroll(&editor, false);
    assert!(!editor.read(|| is_scrollable_tables_active(&e)).unwrap());
    expect_html_to_be_equal(&rendered_html(&editor), &unwrapped);
}

/// `importDOM preserves the dir attribute > dir=%s`.
fn import_dom_preserves_the_dir_attribute(expected: Option<Direction>) {
    let attr = expected.map_or(String::new(), |d| format!(r#" dir="{}""#, d.as_str()));
    let input_html = format!("<table{attr}><tr{attr}><td{attr}>a</td></tr></table>");
    let editor = table_unit_test_editor("{}");
    let e = editor.clone();
    update(&editor, move || {
        let dom = crate::dom::parse_from_string(&input_html);
        let nodes = generate_nodes_from_dom(&e, &dom)?;
        let root = get_root();
        root.clear()?;
        root.append(&nodes)
    });
    editor
        .read(|| {
            let table = get_root().get_first_child().unwrap();
            assert!(is_table_node(&table));
            let row = table.get_first_child().unwrap();
            assert!(is_table_row_node(&row));
            let cell = row.get_first_child().unwrap();
            assert!(is_table_cell_node(&cell));
            assert_eq!(table.element_data().get_direction(), expected);
            assert_eq!(row.element_data().get_direction(), expected);
            assert_eq!(cell.element_data().get_direction(), expected);
        })
        .unwrap();
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > importDOM preserves the dir attribute > dir=rtl
#[test]
fn import_dom_preserves_the_dir_attribute_rtl() {
    import_dom_preserves_the_dir_attribute(Some(Direction::Rtl));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > importDOM preserves the dir attribute > dir=ltr
#[test]
fn import_dom_preserves_the_dir_attribute_ltr() {
    import_dom_preserves_the_dir_attribute(Some(Direction::Ltr));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > importDOM preserves the dir attribute > dir=undefined
#[test]
fn import_dom_preserves_the_dir_attribute_none() {
    import_dom_preserves_the_dir_attribute(None);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableNode.test.tsx::LexicalTableNode tests > importDOM preserves the dir attribute > round trips the dir attribute through exportDOM
#[test]
fn round_trips_the_dir_attribute_through_export_dom() {
    let editor = table_unit_test_editor("{}");
    update(&editor, || {
        let table =
            create_table_node_with_dimensions(1, 1, InsertTableCommandPayloadHeaders::Both(false))?;
        table.set_direction(Some(Direction::Rtl));
        let row = table.get_first_child().unwrap();
        assert!(is_table_row_node(&row));
        row.set_direction(Some(Direction::Rtl));
        let root = get_root();
        root.clear()?;
        root.append(&[table])
    });
    let e = editor.clone();
    let exported = editor
        .read(move || generate_html_from_nodes(&e, None))
        .unwrap()
        .unwrap();
    let e = editor.clone();
    update(&editor, move || {
        let dom = crate::dom::parse_from_string(&exported);
        let nodes = generate_nodes_from_dom(&e, &dom)?;
        let root = get_root();
        root.clear()?;
        root.append(&nodes)
    });
    editor
        .read(|| {
            let table = get_root().get_first_child().unwrap();
            assert!(is_table_node(&table));
            let row = table.get_first_child().unwrap();
            assert!(is_table_row_node(&row));
            assert_eq!(table.element_data().get_direction(), Some(Direction::Rtl));
            assert_eq!(row.element_data().get_direction(), Some(Direction::Rtl));
        })
        .unwrap();
}

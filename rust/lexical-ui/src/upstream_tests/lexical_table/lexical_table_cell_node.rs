//! Ports of `packages/lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts`.

use super::support_b::{expect_html_to_be_equal, outer_html, table_unit_test_editor, theme_config};
use crate::dom::{DomNode, parse_from_string};
use crate::html::{generate_html_from_nodes, generate_nodes_from_dom};
use crate::lexical_dom::{DomConversionNode, DomConversionOutput, get_conversion_function};
use crate::table::{TableCellHeaderStates, create_table_cell_node, is_table_cell_node};
use crate::updates::parse_serialized_node;
use crate::upstream_tests::support::{panics, update};
use crate::{Editor, EditorConfig, LexicalError, NodeRef, create_text_node, get_root};

fn editor_config() -> EditorConfig {
    theme_config(r#"{"tableCell":"test-table-cell-class"}"#)
}

fn editor() -> Editor {
    table_unit_test_editor("{}")
}

fn import_json(json: &str) -> Result<NodeRef, LexicalError> {
    parse_serialized_node(&crate::json::parse(json).unwrap())
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > importJSON coerces out-of-domain span/width/verticalAlign
#[test]
fn import_json_coerces_out_of_domain_span_width_vertical_align() {
    let editor = editor();
    update(&editor, || {
        let cell = import_json(
            r#"{"colSpan":0,"headerState":0,"rowSpan":-3,"type":"tablecell","verticalAlign":"nonsense","width":0}"#,
        )?;
        assert!(is_table_cell_node(&cell));
        assert_eq!(cell.get_col_span(), 1.0);
        assert_eq!(cell.get_row_span(), 1.0);
        assert_eq!(cell.get_cell_width(), None);
        assert_eq!(cell.get_vertical_align(), None);
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > importJSON round-trips in-domain values
#[test]
fn import_json_round_trips_in_domain_values() {
    let editor = editor();
    update(&editor, || {
        let cell = import_json(
            r##"{"backgroundColor":"#fff","children":[],"colSpan":2,"direction":null,"format":"","headerState":1,"indent":0,"rowSpan":3,"type":"tablecell","version":1,"verticalAlign":"middle","width":120}"##,
        )?;
        assert!(is_table_cell_node(&cell));
        assert_eq!(cell.get_col_span(), 2.0);
        assert_eq!(cell.get_row_span(), 3.0);
        assert_eq!(cell.get_cell_width(), Some(120.0));
        assert_eq!(cell.get_vertical_align().as_deref(), Some("middle"));
        assert_eq!(cell.get_background_color().as_deref(), Some("#fff"));
        assert_eq!(cell.get_header_styles(), TableCellHeaderStates::ROW);
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > TableCellNode.constructor
#[test]
fn table_cell_node_constructor() {
    let editor = editor();
    update(&editor, || {
        let cell_node = create_table_cell_node(TableCellHeaderStates::NO_STATUS, 1.0, None);
        assert_eq!(cell_node.get_type(), "tablecell");
        Ok(())
    });
    assert!(panics(|| create_table_cell_node(
        TableCellHeaderStates::NO_STATUS,
        1.0,
        None
    )));
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > TableCellNode.createDOM()
#[test]
fn table_cell_node_create_dom() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let config = editor_config();
        let cell_node = create_table_cell_node(TableCellHeaderStates::NO_STATUS, 1.0, None);
        assert_eq!(
            outer_html(&cell_node, &config, &e),
            r#"<td class="test-table-cell-class"></td>"#
        );
        let header_cell_node = create_table_cell_node(TableCellHeaderStates::ROW, 1.0, None);
        assert_eq!(
            outer_html(&header_cell_node, &config, &e),
            r#"<th class="test-table-cell-class"></th>"#
        );
        let cell_with_row_span_node =
            create_table_cell_node(TableCellHeaderStates::NO_STATUS, 2.0, None);
        assert_eq!(
            outer_html(&cell_with_row_span_node, &config, &e),
            r#"<td colspan="2" class="test-table-cell-class"></td>"#
        );
        let cell_with_custom_width_node =
            create_table_cell_node(TableCellHeaderStates::NO_STATUS, 1.0, Some(200.0));
        assert_eq!(
            outer_html(&cell_with_custom_width_node, &config, &e),
            r#"<td style="width: 200px;" class="test-table-cell-class"></td>"#
        );
        let cell_with_ignored_vertical_align_node =
            create_table_cell_node(TableCellHeaderStates::NO_STATUS, 1.0, None);
        cell_with_ignored_vertical_align_node.set_vertical_align(Some("top"));
        assert_eq!(
            outer_html(&cell_with_ignored_vertical_align_node, &config, &e),
            r#"<td class="test-table-cell-class"></td>"#
        );
        let cell_with_valid_vertical_align_node =
            create_table_cell_node(TableCellHeaderStates::NO_STATUS, 1.0, None);
        cell_with_valid_vertical_align_node.set_vertical_align(Some("middle"));
        assert_eq!(
            outer_html(&cell_with_valid_vertical_align_node, &config, &e),
            r#"<td style="vertical-align: middle;" class="test-table-cell-class"></td>"#
        );
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > TableCellNode.importDOM
#[test]
fn table_cell_node_import_dom() {
    let cases: [(&str, &str); 3] = [
        (
            r#"
            <table>
              <tr>
                <td>
                  <test-decorator></test-decorator>
                  1
                </td>
              </tr>
            </table>
          "#,
            r#"
            <table>
              <tbody>
                <tr>
                  <td
                    style="border: 1px solid black; width: 75px; vertical-align: top; text-align: start;">
                    <p>
                      <test-decorator></test-decorator>
                      <span style="white-space: pre-wrap;">1</span>
                    </p>
                  </td>
                </tr>
              </tbody>
            </table>
          "#,
        ),
        (
            r#"
            <table>
              <tr>
                <td>
                  1
                  <br />
                  <br />
                  <br />
                  <br />
                  2
                </td>
              </tr>
            </table>
          "#,
            r#"
            <table>
              <tbody>
                <tr>
                  <td
                    style="border: 1px solid black; width: 75px; vertical-align: top; text-align: start;">
                    <p>
                      <span style="white-space: pre-wrap;">1</span>
                      <br />
                      <br />
                      <br />
                      <br />
                      <span style="white-space: pre-wrap;">2</span>
                    </p>
                  </td>
                </tr>
              </tbody>
            </table>
          "#,
        ),
        (
            r#"
            <table>
              <tr>
                <td>
                  <p>1</p>
                  <br />
                  <p>2</p>
                </td>
              </tr>
            </table>
          "#,
            r#"
            <table>
              <tbody>
                <tr>
                  <td
                    style="border: 1px solid black; width: 75px; vertical-align: top; text-align: start;">
                    <p>
                      <span style="white-space: pre-wrap;">1</span>
                    </p>
                    <p>
                      <br />
                    </p>
                    <p>
                      <span style="white-space: pre-wrap;">2</span>
                    </p>
                  </td>
                </tr>
              </tbody>
            </table>
          "#,
        ),
    ];
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let root = get_root();
        for (input, output) in cases {
            root.clear()?;
            let dom = parse_from_string(input);
            let nodes = generate_nodes_from_dom(&e, &dom)?;
            root.append(&nodes)?;
            expect_html_to_be_equal(&generate_html_from_nodes(&e, None)?, output);
        }
        Ok(())
    });
}

/// `$convertHTMLTag(element)`: `$runDOMConversion(editor, element)`, the
/// editor's registered conversion for the element, run in place.
fn convert_html_tag(
    editor: &Editor,
    element: &DomNode,
) -> Result<Option<DomConversionOutput>, LexicalError> {
    match get_conversion_function(element, editor) {
        Some(conversion) => conversion(element),
        None => Ok(None),
    }
}

/// `expectTableCellNode(result)`.
fn expect_table_cell_node(result: &Option<DomConversionOutput>) -> NodeRef {
    let Some(DomConversionOutput {
        node: DomConversionNode::One(node),
        ..
    }) = result
    else {
        panic!("Expected result.node to be a TableCellNode");
    };
    assert!(
        is_table_cell_node(node),
        "Expected result.node to be a TableCellNode"
    );
    node.clone()
}

fn element(tag: &str) -> DomNode {
    crate::lexical_dom::get_document().create_element(tag)
}

/// The header state of the cell `th`'s conversion makes.
fn header_state_of(editor: &Editor, th: DomNode) -> i32 {
    let e = editor.clone();
    let out = std::rc::Rc::new(std::cell::Cell::new(-1));
    let o = out.clone();
    update(editor, move || {
        let result = convert_html_tag(&e, &th)?;
        o.set(expect_table_cell_node(&result).get_header_styles());
        Ok(())
    });
    out.get()
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> with scope="col" becomes COLUMN header
#[test]
fn dom_conversion_th_with_scope_col_becomes_column_header() {
    let editor = editor();
    let th = element("th");
    th.set_attribute("scope", "col");
    assert_eq!(header_state_of(&editor, th), TableCellHeaderStates::COLUMN);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> with scope="row" becomes ROW header
#[test]
fn dom_conversion_th_with_scope_row_becomes_row_header() {
    let editor = editor();
    let th = element("th");
    th.set_attribute("scope", "row");
    assert_eq!(header_state_of(&editor, th), TableCellHeaderStates::ROW);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> without scope defaults to ROW header
#[test]
fn dom_conversion_th_without_scope_defaults_to_row_header() {
    let editor = editor();
    let table = element("table");
    let tr = element("tr");
    let th = element("th");
    tr.append_child(&th);
    table.append_child(&tr);
    // First row, first column → BOTH (ROW from header row + COLUMN from
    // first column).
    assert_eq!(header_state_of(&editor, th), TableCellHeaderStates::BOTH);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> in first row without scope becomes ROW header
#[test]
fn dom_conversion_th_in_first_row_without_scope_becomes_row_header() {
    let editor = editor();
    let table = element("table");
    let tr = element("tr");
    let th = element("th");
    let td = element("td");
    tr.append_child(&th);
    tr.append_child(&td);
    table.append_child(&tr);
    // First row, first column → BOTH.
    assert_eq!(header_state_of(&editor, th), TableCellHeaderStates::BOTH);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> in first column of non-first row becomes COLUMN header
#[test]
fn dom_conversion_th_in_first_column_of_non_first_row_becomes_column_header() {
    let editor = editor();
    let table = element("table");
    let tr1 = element("tr");
    let tr2 = element("tr");
    let th1 = element("th");
    let td1 = element("td");
    let th2 = element("th");
    let td2 = element("td");
    tr1.append_child(&th1);
    tr1.append_child(&td1);
    tr2.append_child(&th2);
    tr2.append_child(&td2);
    table.append_child(&tr1);
    table.append_child(&tr2);
    // Non-first row, first column → COLUMN.
    assert_eq!(header_state_of(&editor, th2), TableCellHeaderStates::COLUMN);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <th> in thead without scope becomes ROW header
#[test]
fn dom_conversion_th_in_thead_without_scope_becomes_row_header() {
    let editor = editor();
    let table = element("table");
    let thead = element("thead");
    let tr = element("tr");
    let th = element("th");
    let th2 = element("th");
    tr.append_child(&th);
    tr.append_child(&th2);
    thead.append_child(&tr);
    table.append_child(&thead);
    // Second th in thead → ROW (not first column, so only ROW from the
    // first row).
    assert_eq!(header_state_of(&editor, th2), TableCellHeaderStates::ROW);
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <td> with style.backgroundColor reads inline background-color
#[test]
fn dom_conversion_td_with_style_background_color_reads_inline_background_color() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let td = element("td");
        td.style().set_property("background-color", "#F4B084", "");
        let result = convert_html_tag(&e, &td)?;
        let node = expect_table_cell_node(&result);
        // The style declaration normalizes the color.
        assert_eq!(
            node.get_background_color(),
            Some(td.style().get_property_value("background-color"))
        );
        Ok(())
    });
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <td> with no background color sets backgroundColor to null
#[test]
fn dom_conversion_td_with_no_background_color_sets_background_color_to_null() {
    let editor = editor();
    let e = editor.clone();
    update(&editor, move || {
        let td = element("td");
        let result = convert_html_tag(&e, &td)?;
        let node = expect_table_cell_node(&result);
        assert_eq!(node.get_background_color(), None);
        Ok(())
    });
}

/// Runs the `after` of `td`'s conversion on a text node with `style`, and
/// returns the text node's style.
fn style_after_conversion(editor: &Editor, td: DomNode, style: &'static str) -> String {
    let e = editor.clone();
    let out = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let o = out.clone();
    update(editor, move || {
        let result = convert_html_tag(&e, &td)?;
        expect_table_cell_node(&result);
        let text_node = create_text_node("Hello");
        if !style.is_empty() {
            text_node.set_style(style);
        }
        let after = result.and_then(|r| r.after).expect("an after callback");
        after(vec![text_node.clone()])?;
        *o.borrow_mut() = text_node.text_data().get_style().to_owned();
        Ok(())
    });
    out.take()
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <td> with color propagates color to child TextNodes via after callback
#[test]
fn dom_conversion_td_with_color_propagates_color_to_child_text_nodes_via_after_callback() {
    let editor = editor();
    let td = element("td");
    td.style().set_property("color", "blue", "");
    let style = style_after_conversion(&editor, td, "");
    assert!(style.contains("color: blue"), "{style}");
}

// upstream: lexical-table/src/__tests__/unit/LexicalTableCellNode.test.ts::LexicalTableCellNode tests > DOM Conversion: <td> color does not overwrite existing child TextNode color
#[test]
fn dom_conversion_td_color_does_not_overwrite_existing_child_text_node_color() {
    let editor = editor();
    let td = element("td");
    td.style().set_property("color", "blue", "");
    let style = style_after_conversion(&editor, td, "color: red;");
    assert!(style.contains("color: red"), "{style}");
    assert!(!style.contains("color: blue"), "{style}");
}

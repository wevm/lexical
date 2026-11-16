//! Ports of `packages/lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts`.
//! The `describe.each(IMPORTERS)` tests are one Rust test per upstream
//! test, run with each importer by `import_with`.

#![cfg(all(feature = "extension", feature = "rich-text", feature = "link"))]

use super::super::support::update;
use crate::dom::{DomNode, parse_from_string};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::import::generate_nodes_from_dom_via_extension;
use crate::html::{generate_html_from_nodes, generate_nodes_from_dom};
use crate::link::{LinkAttributes, create_link_node};
use crate::nodes::line_break::create_line_break_node;
use crate::{
    Editor, InitialEditorStateType, LexicalError, LexicalExtension, NodeRef, create_paragraph_node,
    create_text_node, define_extension, get_root,
};

fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            crate::link::link_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[linebreak-round-trip]", ())
    })
    .into()])
    .unwrap()
}

fn text_a() -> NodeRef {
    create_text_node("a")
}

/// `$seed(...children)` in an update.
fn seed(editor: &Editor, children: &'static [fn() -> NodeRef]) {
    update(editor, move || {
        let paragraph = create_paragraph_node();
        paragraph.append(&children.iter().map(|f| f()).collect::<Vec<_>>())?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])
    });
}

fn export_html(editor: &Editor) -> String {
    editor
        .read(|| generate_html_from_nodes(editor, None))
        .unwrap()
        .unwrap()
}

fn parse(html: &str) -> DomNode {
    parse_from_string(&format!("<!doctype html><html><body>{html}</body></html>"))
}

/// `childTypes(nodes)`.
fn child_types(nodes: &[NodeRef]) -> Vec<String> {
    match nodes.first() {
        Some(first) if first.is_element() => first
            .get_children()
            .iter()
            .map(|c| c.get_type().to_owned())
            .collect(),
        _ => vec![],
    }
}

/// `IMPORTERS`: `$generateNodesFromDOM` (`false`) and
/// `$generateNodesFromDOMViaExtension` (`true`).
fn import_with(
    via_extension: bool,
    editor: &Editor,
    html: &str,
) -> Result<Vec<NodeRef>, LexicalError> {
    if via_extension {
        generate_nodes_from_dom_via_extension(&parse(html), None)
    } else {
        generate_nodes_from_dom(editor, &parse(html))
    }
}

/// Imports `html` with each importer, expecting the child types.
fn expect_import(editor: &Editor, html: &str, expected: &'static [&'static str]) {
    for via_extension in [false, true] {
        let (e, html) = ((*editor).clone(), html.to_owned());
        update(editor, move || {
            assert_eq!(
                child_types(&import_with(via_extension, &e, &html)?),
                expected,
                "via extension: {via_extension}"
            );
            Ok(())
        });
    }
}

// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > a trailing line break exports with a terminating <br>
#[test]
fn a_trailing_line_break_exports_with_a_terminating_br() {
    let editor = build_editor();
    seed(&editor, &[text_a, create_line_break_node]);
    assert_eq!(
        export_html(&editor),
        r#"<p><span style="white-space: pre-wrap;">a</span><br><br data-lexical-managed-linebreak="true"></p>"#
    );
}

// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > a lone line break exports with a terminating <br>
#[test]
fn a_lone_line_break_exports_with_a_terminating_br() {
    let editor = build_editor();
    seed(&editor, &[create_line_break_node]);
    assert_eq!(
        export_html(&editor),
        r#"<p><br><br data-lexical-managed-linebreak="true"></p>"#
    );
}

// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > the exported terminator carries the reconciler marker
#[test]
fn the_exported_terminator_carries_the_reconciler_marker() {
    let editor = build_editor();
    seed(&editor, &[text_a, create_line_break_node]);
    let brs = parse(&export_html(&editor))
        .query_selector_all("br")
        .unwrap();
    assert_eq!(brs.len(), 2);
    assert!(
        brs[0]
            .get_attribute("data-lexical-managed-linebreak")
            .is_none()
    );
    assert_eq!(
        brs[1]
            .get_attribute("data-lexical-managed-linebreak")
            .as_deref(),
        Some("true")
    );
}

// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > an interior line break exports unchanged
#[test]
fn an_interior_line_break_exports_unchanged() {
    let editor = build_editor();
    seed(
        &editor,
        &[text_a, create_line_break_node, || create_text_node("b")],
    );
    assert_eq!(
        export_html(&editor),
        r#"<p><span style="white-space: pre-wrap;">a</span><br><span style="white-space: pre-wrap;">b</span></p>"#
    );
}

// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > a line break trailing an inline element exports unchanged
#[test]
fn a_line_break_trailing_an_inline_element_exports_unchanged() {
    let editor = build_editor();
    seed(
        &editor,
        &[|| {
            let link = create_link_node("https://lexical.dev/", &LinkAttributes::default());
            link.append(&[create_text_node("a"), create_line_break_node()])
                .unwrap();
            link
        }],
    );
    assert_eq!(
        export_html(&editor),
        r#"<p><a href="https://lexical.dev/"><span style="white-space: pre-wrap;">a</span><br></a></p>"#
    );
}

// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOM > restores a trailing line break
// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOMViaExtension > restores a trailing line break
#[test]
fn restores_a_trailing_line_break() {
    let editor = build_editor();
    seed(&editor, &[text_a, create_line_break_node]);
    let html = export_html(&editor);
    expect_import(&editor, &html, &["text", "linebreak"]);
}

// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOM > restores a lone line break
// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOMViaExtension > restores a lone line break
#[test]
fn restores_a_lone_line_break() {
    let editor = build_editor();
    seed(&editor, &[create_line_break_node]);
    let html = export_html(&editor);
    expect_import(&editor, &html, &["linebreak"]);
}

// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOM > restores consecutive trailing line breaks
// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOMViaExtension > restores consecutive trailing line breaks
#[test]
fn restores_consecutive_trailing_line_breaks() {
    let editor = build_editor();
    seed(
        &editor,
        &[text_a, create_line_break_node, create_line_break_node],
    );
    let html = export_html(&editor);
    expect_import(&editor, &html, &["text", "linebreak", "linebreak"]);
}

// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOM > restores a trailing line break with the marker stripped
// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOMViaExtension > restores a trailing line break with the marker stripped
#[test]
fn restores_a_trailing_line_break_with_the_marker_stripped() {
    let editor = build_editor();
    seed(&editor, &[text_a, create_line_break_node]);
    let html = export_html(&editor).replace(r#" data-lexical-managed-linebreak="true""#, "");
    assert!(!html.contains("data-lexical-managed-linebreak"));
    expect_import(&editor, &html, &["text", "linebreak"]);
}

// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOM > still drops an unmarked trailing <br> from external HTML
// upstream: lexical-html/src/__tests__/unit/LineBreakRoundTrip.test.ts::trailing LineBreakNode survives an HTML round trip > $generateNodesFromDOMViaExtension > still drops an unmarked trailing <br> from external HTML
#[test]
fn still_drops_an_unmarked_trailing_br_from_external_html() {
    let editor = build_editor();
    expect_import(&editor, "<p>a<br></p>", &["text"]);
}

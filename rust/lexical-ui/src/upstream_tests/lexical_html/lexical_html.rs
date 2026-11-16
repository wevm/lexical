//! Ports of `packages/lexical-html/src/__tests__/unit/LexicalHtml.test.ts`.
//!
//! Upstream's headless editors list `CodeNode` and `LinkNode` too; their
//! features gate the module. The DOM is the `html` feature's.

#![cfg(all(
    feature = "rich-text",
    feature = "list",
    feature = "link",
    feature = "code",
    feature = "clipboard"
))]

use super::super::support::{
    TestShadowRootNode, create_test_shadow_root_node, update, update_with,
};
use crate::clipboard::insert_generated_nodes;
use crate::dom::parse_from_string;
use crate::headless::create_headless_editor;
use crate::html::{generate_html_from_nodes, generate_nodes_from_dom};
use crate::lexical_dom::{DomExportOutput, HtmlConfig, get_document};
use crate::list::{ListType, create_list_item_node, create_list_node};
use crate::nodes::element::Direction;
use crate::{
    CreateEditorArgs, Editor, EditorReadMode, LexicalError, NodeClass, create_paragraph_node,
    create_range_selection, create_text_node, get_root, set_slot,
};
use std::rc::Rc;

/// `[HeadingNode, ListNode, ListItemNode, QuoteNode, CodeNode, LinkNode]`.
fn html_nodes() -> Vec<NodeClass> {
    vec![
        NodeClass::of::<crate::rich_text::HeadingNode>(),
        NodeClass::of::<crate::list::ListNode>(),
        NodeClass::of::<crate::list::ListItemNode>(),
        NodeClass::of::<crate::rich_text::QuoteNode>(),
        NodeClass::of::<crate::code::CodeNode>(),
        NodeClass::of::<crate::link::LinkNode>(),
    ]
}

fn headless(nodes: Vec<NodeClass>) -> Editor {
    create_headless_editor(CreateEditorArgs {
        nodes,
        ..CreateEditorArgs::default()
    })
}

fn serialize(initialize: impl FnOnce() -> Result<(), LexicalError> + 'static) -> String {
    let editor = headless(html_nodes());
    update(&editor, initialize);
    let e = editor.clone();
    editor
        .read_with_mode(EditorReadMode::Latest, move || {
            generate_html_from_nodes(&e, None)
        })
        .unwrap()
        .unwrap()
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > [Lexical -> HTML]: Empty editor state
#[test]
fn lexical_to_html_empty_editor_state() {
    assert_eq!(
        serialize(|| get_root().append(&[create_paragraph_node()])),
        "<p><br></p>"
    );
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > [Lexical -> HTML]: Nested ordered list numbering (Regression #7207)
#[test]
fn lexical_to_html_nested_ordered_list_numbering() {
    let html = serialize(|| {
        let item = |text: &str| -> Result<_, LexicalError> {
            let item = create_list_item_node(None);
            item.append(&[create_text_node(text)])?;
            Ok(item)
        };
        let list = create_list_node(ListType::Number, 1.0);
        let nested_wrapper = create_list_item_node(None);
        let nested_list = create_list_node(ListType::Number, 1.0);
        nested_list.append(&[item("LA")?, item("TX")?])?;
        nested_wrapper.append(&[nested_list])?;
        list.append(&[
            item("Canada")?,
            item("USA")?,
            nested_wrapper,
            item("Germany")?,
        ])?;
        get_root().append(&[list])
    });
    assert_eq!(
        html,
        r#"<ol><li value="1"><span style="white-space: pre-wrap;">Canada</span></li><li value="2"><span style="white-space: pre-wrap;">USA</span><ol><li value="1"><span style="white-space: pre-wrap;">LA</span></li><li value="2"><span style="white-space: pre-wrap;">TX</span></li></ol></li><li value="3"><span style="white-space: pre-wrap;">Germany</span></li></ol>"#
    );
}

/// Two paragraphs, `Hello` (selected) and `World`; returns the `World`
/// text node.
fn hello_world() -> Result<crate::NodeRef, LexicalError> {
    let root = get_root();
    let p1 = create_paragraph_node();
    let text1 = create_text_node("Hello");
    p1.append(std::slice::from_ref(&text1))?;
    let p2 = create_paragraph_node();
    let text2 = create_text_node("World");
    p2.append(std::slice::from_ref(&text2))?;
    root.append(&[p1.clone(), p2])?;
    p1.select(Some(0), Some(text1.get_text_content_size()))?;
    Ok(text2)
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > [Lexical -> HTML]: Use provided selection
#[test]
fn lexical_to_html_use_provided_selection() {
    let editor = headless(html_nodes());
    let selection = update_with(&editor, || {
        let text2 = hello_world()?;
        let selection = create_range_selection();
        selection.set_text_node_range(&text2, 0, &text2, text2.get_text_content_size())?;
        Ok(selection)
    });
    let e = editor.clone();
    let html = update_with(&editor, move || {
        generate_html_from_nodes(&e, Some(&selection))
    });
    assert_eq!(html, r#"<span style="white-space: pre-wrap;">World</span>"#);
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > [Lexical -> HTML]: Default selection (undefined) should serialize entire editor state
#[test]
fn lexical_to_html_default_selection_serializes_entire_editor_state() {
    let editor = headless(html_nodes());
    update(&editor, || hello_world().map(|_| ()));
    let e = editor.clone();
    let html = update_with(&editor, move || generate_html_from_nodes(&e, None));
    assert_eq!(
        html,
        r#"<p><span style="white-space: pre-wrap;">Hello</span></p><p><span style="white-space: pre-wrap;">World</span></p>"#
    );
}

/// Imports `html` into a headless editor and exports it again.
fn import_then_export(html: &'static str) -> String {
    let editor = headless(vec![]);
    let e = editor.clone();
    update(&editor, move || {
        let dom = parse_from_string(html);
        let nodes = generate_nodes_from_dom(&e, &dom)?;
        get_root().append(&nodes)
    });
    let e = editor.clone();
    update_with(&editor, move || generate_html_from_nodes(&e, None))
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > If alignment is set on the paragraph, don't overwrite from parent empty format
#[test]
fn alignment_on_the_paragraph_is_not_overwritten_from_parent_empty_format() {
    assert_eq!(
        import_then_export(r#"<div><p style="text-align: center;">Hello world!</p></div>"#),
        r#"<p style="text-align: center;"><span style="white-space: pre-wrap;">Hello world!</span></p>"#
    );
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > If alignment is set on the paragraph, it should take precedence over its parent block alignment
#[test]
fn alignment_on_the_paragraph_takes_precedence_over_its_parent_block_alignment() {
    assert_eq!(
        import_then_export(
            r#"<div style="text-align: right;"><p style="text-align: center;">Hello world!</p></div>"#
        ),
        r#"<p style="text-align: center;"><span style="white-space: pre-wrap;">Hello world!</span></p>"#
    );
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > It should output correctly nodes whose export is DocumentFragment
#[test]
fn outputs_nodes_whose_export_is_a_document_fragment() {
    let editor = create_headless_editor(CreateEditorArgs {
        html: HtmlConfig {
            export: vec![(
                "paragraph",
                Rc::new(|_, _, _| {
                    Ok(DomExportOutput::element(
                        get_document().create_document_fragment(),
                    ))
                }),
            )],
            ..HtmlConfig::default()
        },
        ..CreateEditorArgs::default()
    });
    update(&editor, || hello_world().map(|_| ()));
    let e = editor.clone();
    let html = update_with(&editor, move || generate_html_from_nodes(&e, None));
    assert_eq!(
        html,
        r#"<span style="white-space: pre-wrap;">Hello</span><span style="white-space: pre-wrap;">World</span>"#
    );
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > $generateNodesFromDOM: CSS class style inlining > HTML with <style> tags inlines styles by class
#[test]
fn html_with_style_tags_inlines_styles_by_class() {
    let editor = crate::extension::build_editor_from_extensions([]).unwrap();
    let dom = parse_from_string(
        "<html><head><style>.highlight { font-weight: bold; }</style></head><body><p><span class=\"highlight\">Hello</span></p></body></html>",
    );
    // `dom.styleSheets`: the document's one `<style>`.
    assert_eq!(dom.query_selector_all("style").unwrap().len(), 1);
    let e = (*editor).clone();
    update(&editor, move || {
        let root = get_root();
        let nodes = generate_nodes_from_dom(&e, &dom)?;
        insert_generated_nodes(&e, nodes, &root.select(Some(0), None)?)?;
        assert_eq!(root.get_text_content(), "Hello");
        let highlight_span = dom.query_selector_all("span.highlight").unwrap();
        let highlight_span = highlight_span
            .first()
            .expect("span.highlight is in the dom");
        assert_eq!(
            highlight_span.style().get_property_value("font-weight"),
            "bold"
        );
        let text_nodes = root.get_all_text_nodes();
        assert_eq!(text_nodes.len(), 1);
        assert!(text_nodes[0].has_format("bold"));
        Ok(())
    });
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > $generateNodesFromDOM: CSS class style inlining > existing inline styles are preserved after inlining pass
#[test]
fn existing_inline_styles_are_preserved_after_inlining_pass() {
    let editor = headless(vec![]);
    let e = editor.clone();
    update(&editor, move || {
        let dom = parse_from_string(
            "<html><head><style>.colored { color: red; }</style></head><body><p><span class=\"colored\" style=\"color: blue;\">Hello</span></p></body></html>",
        );
        generate_nodes_from_dom(&e, &dom)?;
        let span = dom.query_selector_all(".colored").unwrap();
        let span = span.first().expect(".colored is still in the dom");
        assert_eq!(span.style().get_property_value("color"), "blue");
        Ok(())
    });
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > $generateNodesFromDOM: CSS class style inlining > HTML without <style> tags works as before
#[test]
fn html_without_style_tags_works_as_before() {
    let editor = headless(vec![]);
    let e = editor.clone();
    update(&editor, move || {
        let root = get_root();
        let dom = parse_from_string("<p>Hello world</p>");
        let nodes = generate_nodes_from_dom(&e, &dom)?;
        insert_generated_nodes(&e, nodes, &root.select(Some(0), None)?)?;
        assert_eq!(
            root.get_all_text_nodes()
                .iter()
                .map(|n| (n.get_text_content(), n.has_format("bold")))
                .collect::<Vec<_>>(),
            [("Hello world".to_owned(), false)]
        );
        Ok(())
    });
}

/// `importAndGetDirection(html)`.
fn import_and_get_direction(html: &'static str) -> Option<Direction> {
    let editor = headless(vec![
        NodeClass::of::<crate::rich_text::HeadingNode>(),
        NodeClass::of::<crate::list::ListNode>(),
        NodeClass::of::<crate::list::ListItemNode>(),
        NodeClass::of::<crate::rich_text::QuoteNode>(),
    ]);
    let e = editor.clone();
    update(&editor, move || {
        let dom = parse_from_string(html);
        let nodes = generate_nodes_from_dom(&e, &dom)?;
        get_root().select_end()?;
        let selection = get_root().select(Some(0), None)?;
        selection.insert_nodes(nodes)
    });
    editor
        .read(|| {
            get_root()
                .get_first_child()
                .filter(|c| c.is_element())
                .and_then(|c| c.element_data().get_direction())
        })
        .unwrap()
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > paragraph with dir="rtl"
// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > paragraph with dir="ltr"
// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > paragraph without dir
// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > heading with dir="rtl"
// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > blockquote with dir="rtl"
// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > importDOM preserves dir attribute > list with dir="rtl"
#[test]
fn import_dom_preserves_dir_attribute() {
    let cases = [
        ("<p dir=\"rtl\">مرحبا</p>", Some(Direction::Rtl)),
        ("<p dir=\"ltr\">Hello</p>", Some(Direction::Ltr)),
        ("<p>Hello</p>", None),
        ("<h1 dir=\"rtl\">عنوان</h1>", Some(Direction::Rtl)),
        (
            "<blockquote dir=\"rtl\">اقتباس</blockquote>",
            Some(Direction::Rtl),
        ),
        ("<ul dir=\"rtl\"><li>عنصر</li></ul>", Some(Direction::Rtl)),
    ];
    for (html, expected) in cases {
        assert_eq!(import_and_get_direction(html), expected, "{html}");
    }
}

// upstream: lexical-html/src/__tests__/unit/LexicalHtml.test.ts::HTML > [Lexical -> HTML]: slots are not auto-serialized to HTML
#[test]
fn lexical_to_html_slots_are_not_auto_serialized() {
    let editor = create_headless_editor(CreateEditorArgs {
        namespace: "slot".to_owned(),
        nodes: vec![NodeClass::of::<TestShadowRootNode>()],
        ..CreateEditorArgs::default()
    });
    update(&editor, || {
        let host = create_paragraph_node();
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("SLOTTEXT")])?;
        let slot = create_test_shadow_root_node();
        slot.append(&[paragraph])?;
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &slot)?;
        Ok(())
    });
    let e = editor.clone();
    let html = editor
        .read(move || generate_html_from_nodes(&e, None))
        .unwrap()
        .unwrap();
    assert!(!html.contains("SLOTTEXT"));
}

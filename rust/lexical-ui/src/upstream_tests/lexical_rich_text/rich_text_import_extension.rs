//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts`.

#![cfg(all(feature = "extension", feature = "html"))]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::import::{
    DomImportConfigOverrides, dom_import_extension, generate_nodes_from_dom_via_extension,
};
use crate::lexical_utils::is_shadow_root_node;
use crate::rich_text::{
    HeadingTagType, is_quote_node, rich_text_extension, shadow_root_quote_rule,
};
use crate::upstream_tests::support::update;
use crate::{
    AnyLexicalExtensionArgument, LexicalExtension, NodeRef, config_extension, define_extension,
    get_root,
};

fn build(
    name: &'static str,
    dependencies: Vec<AnyLexicalExtensionArgument>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `buildEditor()`: RichTextExtension registers its own import rules.
fn build_editor() -> LexicalEditorWithDispose {
    build("rich-text-host", vec![rich_text_extension().into()])
}

/// With `ShadowRootQuoteRule` registered after RichTextExtension's rules.
fn build_shadow_root_quote_editor() -> LexicalEditorWithDispose {
    build(
        "rich-text-shadow-root-quote-host",
        vec![
            rich_text_extension().into(),
            config_extension(
                &dom_import_extension(),
                [DomImportConfigOverrides {
                    rules: Some(vec![shadow_root_quote_rule().into()]),
                    ..DomImportConfigOverrides::default()
                }],
            ),
        ],
    )
}

/// `importInto(editor, html)`.
fn import_into(editor: &LexicalEditorWithDispose, html: &str) {
    let html = format!("<!doctype html><html><body>{html}</body></html>");
    update(editor, move || {
        let dom = crate::dom::parse_from_string(&html);
        let nodes = generate_nodes_from_dom_via_extension(&dom, None)?;
        let root = get_root();
        root.clear()?;
        root.append(&nodes)
    });
}

fn is_paragraph_node(node: &NodeRef) -> bool {
    node.get_type() == "paragraph"
}

fn imports_as_heading(html: &str, tag: HeadingTagType) {
    let editor = build_editor();
    import_into(&editor, html);
    editor
        .read(|| {
            let node = get_root().get_first_child().unwrap();
            assert_eq!(node.get_heading_tag(), Some(tag), "expected HeadingNode");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h1>x</h1> imports as heading h1
#[test]
fn h1_imports_as_heading_h1() {
    imports_as_heading("<h1>x</h1>", HeadingTagType::H1);
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h2>x</h2> imports as heading h2
#[test]
fn h2_imports_as_heading_h2() {
    imports_as_heading("<h2>x</h2>", HeadingTagType::H2);
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h3>x</h3> imports as heading h3
#[test]
fn h3_imports_as_heading_h3() {
    imports_as_heading("<h3>x</h3>", HeadingTagType::H3);
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h4>x</h4> imports as heading h4
#[test]
fn h4_imports_as_heading_h4() {
    imports_as_heading("<h4>x</h4>", HeadingTagType::H4);
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h5>x</h5> imports as heading h5
#[test]
fn h5_imports_as_heading_h5() {
    imports_as_heading("<h5>x</h5>", HeadingTagType::H5);
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > <h6>x</h6> imports as heading h6
#[test]
fn h6_imports_as_heading_h6() {
    imports_as_heading("<h6>x</h6>", HeadingTagType::H6);
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > blockquote imports as quote
#[test]
fn blockquote_imports_as_quote() {
    let editor = build_editor();
    import_into(&editor, "<blockquote>quoted</blockquote>");
    editor
        .read(|| {
            let node = get_root().get_first_child().unwrap();
            assert!(is_quote_node(&node), "expected QuoteNode");
            assert_eq!(node.get_text_content(), "quoted");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > blockquote does not import as a shadow root by default
#[test]
fn blockquote_does_not_import_as_a_shadow_root_by_default() {
    let editor = build_editor();
    import_into(&editor, "<blockquote><p>a</p><p>b</p></blockquote>");
    editor
        .read(|| {
            let node = get_root().get_first_child().unwrap();
            assert!(is_quote_node(&node), "expected QuoteNode");
            assert!(!is_shadow_root_node(&node));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > ShadowRootQuoteRule imports blockquote as a shadow root quote
#[test]
fn shadow_root_quote_rule_imports_blockquote_as_a_shadow_root_quote() {
    let editor = build_shadow_root_quote_editor();
    import_into(&editor, "<blockquote><p>a</p><p>b</p></blockquote>");
    editor
        .read(|| {
            let node = get_root().get_first_child().unwrap();
            assert!(is_quote_node(&node), "expected QuoteNode");
            assert!(is_shadow_root_node(&node));
            let children = node.get_children();
            assert_eq!(children.len(), 2);
            assert!(children.iter().all(is_paragraph_node));
            let texts: Vec<String> = children.iter().map(NodeRef::get_text_content).collect();
            assert_eq!(texts, ["a", "b"]);
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > ShadowRootQuoteRule wraps bare inline content in a paragraph
#[test]
fn shadow_root_quote_rule_wraps_bare_inline_content_in_a_paragraph() {
    let editor = build_shadow_root_quote_editor();
    import_into(&editor, "<blockquote>quoted</blockquote>");
    editor
        .read(|| {
            let node = get_root().get_first_child().unwrap();
            assert!(is_quote_node(&node), "expected QuoteNode");
            assert!(is_shadow_root_node(&node));
            let children = node.get_children();
            assert_eq!(children.len(), 1);
            assert!(is_paragraph_node(&children[0]), "expected ParagraphNode");
            assert_eq!(children[0].get_text_content(), "quoted");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/RichTextImportExtension.test.ts::RichTextImportExtension > Google Docs title (26pt span) promoted to h1
#[test]
fn google_docs_title_26pt_span_promoted_to_h1() {
    let editor = build_editor();
    import_into(
        &editor,
        r#"<p><span style="font-size:26pt">Title</span></p>"#,
    );
    editor
        .read(|| {
            let node = get_root().get_first_child().unwrap();
            assert_eq!(
                node.get_heading_tag(),
                Some(HeadingTagType::H1),
                "expected HeadingNode"
            );
            assert_eq!(node.get_text_content(), "Title");
        })
        .unwrap();
    editor.dispose();
}

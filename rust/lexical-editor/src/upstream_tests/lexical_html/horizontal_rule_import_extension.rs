//! Ports of `packages/lexical-html/src/__tests__/unit/HorizontalRuleImportExtension.test.ts`.

#![cfg(feature = "extension")]

use super::super::support::update;
use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, horizontal_rule_extension,
    is_horizontal_rule_node,
};
use crate::html::import::{core_import_extension, generate_nodes_from_dom_via_extension};
use crate::{AnyLexicalExtensionArgument, Editor, LexicalExtension, define_extension, get_root};

fn build(name: &str, dependencies: Vec<AnyLexicalExtensionArgument>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `buildEditor()`.
fn build_editor() -> LexicalEditorWithDispose {
    build(
        "hr-host",
        vec![
            core_import_extension().into(),
            horizontal_rule_extension().into(),
        ],
    )
}

/// `importInto(editor, html)`.
fn import_into(editor: &Editor, html: &str) {
    let html = format!("<!doctype html><html><body>{html}</body></html>");
    update(editor, move || {
        let dom = crate::dom::parse_from_string(&html);
        let nodes = generate_nodes_from_dom_via_extension(&dom, None)?;
        let root = get_root();
        root.clear()?;
        root.splice(0, 0, &nodes)
    });
}

// upstream: lexical-html/src/__tests__/unit/HorizontalRuleImportExtension.test.ts::HorizontalRuleImportExtension > <hr> imports as HorizontalRuleNode
#[test]
fn hr_imports_as_horizontal_rule_node() {
    let editor = build_editor();
    import_into(&editor, "<hr>");
    editor
        .read(|| {
            let node = get_root().get_first_child().unwrap();
            assert!(
                is_horizontal_rule_node(&node),
                "expected HorizontalRuleNode"
            );
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/HorizontalRuleImportExtension.test.ts::HorizontalRuleImportExtension > <hr> between paragraphs preserves surrounding structure
#[test]
fn hr_between_paragraphs_preserves_surrounding_structure() {
    let editor = build_editor();
    import_into(&editor, "<p>before</p><hr><p>after</p>");
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 3);
            assert_eq!(children[0].get_type(), "paragraph");
            assert!(is_horizontal_rule_node(&children[1]));
            assert_eq!(children[2].get_type(), "paragraph");
            assert_eq!(children[0].get_text_content(), "before");
            assert_eq!(children[2].get_text_content(), "after");
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/HorizontalRuleImportExtension.test.ts::HorizontalRuleImportExtension > <hr> is dropped when HorizontalRuleNode is not registered
#[test]
fn hr_is_dropped_when_horizontal_rule_node_is_not_registered() {
    let editor = build("hr-gated-host", vec![core_import_extension().into()]);
    import_into(&editor, "<p>before</p><hr><p>after</p>");
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].get_type(), "paragraph");
            assert_eq!(children[1].get_type(), "paragraph");
            assert_eq!(children[0].get_text_content(), "before");
            assert_eq!(children[1].get_text_content(), "after");
        })
        .unwrap();
}

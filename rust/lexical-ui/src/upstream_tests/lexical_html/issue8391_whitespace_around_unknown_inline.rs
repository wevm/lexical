//! Ports of `packages/lexical-html/src/__tests__/unit/Issue8391WhitespaceAroundUnknownInline.test.ts`.

#![cfg(feature = "extension")]

use super::super::support::update;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::import::{
    DomImportConfigOverrides, DomImportRule, GenerateNodesFromDomOptions,
    ImportContextPairOrUpdater, WhitespaceImportConfig, context_value, core_import_extension,
    default_is_inline, default_preserves_whitespace, define_import_rule, dom_import_extension,
    generate_nodes_from_dom_via_extension, import_whitespace_config, sel,
};
use crate::{
    Editor, LexicalExtension, config_extension, create_text_node, define_extension, get_root,
};
use std::rc::Rc;

const HTML: &str = "<p>In web development, the DOM <tooltip>Document Object Model</tooltip> allows scripts to update content.</p>";

const TOOLTIP_MARKER: &str = "[TOOLTIP]";

/// `TooltipRule`.
fn tooltip_rule() -> DomImportRule {
    define_import_rule(Some("test/tooltip"), sel::tag(&["tooltip"]), |_, _, _| {
        let text = create_text_node(TOOLTIP_MARKER);
        text.set_format(f64::from(crate::constants::IS_HIGHLIGHT));
        Ok(vec![text])
    })
}

/// `contextValue(ImportWhitespaceConfig, {isInline: <tooltip> too,
/// preservesWhitespace: defaultPreservesWhitespace})`.
fn tooltip_inline_context() -> ImportContextPairOrUpdater {
    context_value(
        &import_whitespace_config(),
        WhitespaceImportConfig {
            is_inline: Rc::new(|node| {
                default_is_inline(node) || (node.is_element() && node.node_name() == "TOOLTIP")
            }),
            preserves_whitespace: Rc::new(default_preserves_whitespace),
        },
    )
}

fn build(context_defaults: Option<Vec<ImportContextPairOrUpdater>>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            core_import_extension().into(),
            config_extension(
                &dom_import_extension(),
                [DomImportConfigOverrides {
                    context_defaults,
                    rules: Some(vec![tooltip_rule().into()]),
                    ..Default::default()
                }],
            ),
        ],
        ..LexicalExtension::<(), (), ()>::new("host", ())
    })
    .into()])
    .unwrap()
}

/// `importInto(editor, HTML)`, with `options`.
fn import_into(editor: &Editor, options: Option<GenerateNodesFromDomOptions>) {
    update(editor, move || {
        let dom = crate::dom::parse_from_string(&format!(
            "<!doctype html><html><body>{HTML}</body></html>"
        ));
        let nodes = generate_nodes_from_dom_via_extension(&dom, options.as_ref())?;
        let root = get_root();
        root.clear()?;
        root.splice(0, 0, &nodes)
    });
}

/// `$rootParagraphTextContents()`.
fn root_paragraph_text_contents(editor: &Editor) -> Vec<String> {
    editor
        .read(|| {
            let para = get_root().get_first_child().expect("expected paragraph");
            assert_eq!(para.get_type(), "paragraph", "expected paragraph");
            para.get_children()
                .iter()
                .map(|c| c.get_text_content())
                .collect()
        })
        .unwrap()
}

// upstream: lexical-html/src/__tests__/unit/Issue8391WhitespaceAroundUnknownInline.test.ts::issue #8391 — whitespace around unknown inline elements > default config: spaces are trimmed against an unknown <tooltip> (reproduces the bug)
#[test]
fn default_config_trims_spaces_against_an_unknown_tooltip() {
    let editor = build(None);
    import_into(&editor, None);
    let texts = root_paragraph_text_contents(&editor);
    assert_eq!(texts[0], "In web development, the DOM");
    assert_eq!(texts[1], TOOLTIP_MARKER);
    assert_eq!(texts[2], "allows scripts to update content.");
}

// upstream: lexical-html/src/__tests__/unit/Issue8391WhitespaceAroundUnknownInline.test.ts::issue #8391 — whitespace around unknown inline elements > override isInline to recognize <tooltip>: spaces preserved
#[test]
fn override_is_inline_to_recognize_tooltip_preserves_spaces() {
    let editor = build(Some(vec![tooltip_inline_context()]));
    import_into(&editor, None);
    let texts = root_paragraph_text_contents(&editor);
    assert_eq!(texts[0], "In web development, the DOM ");
    assert_eq!(texts[1], TOOLTIP_MARKER);
    assert_eq!(texts[2], " allows scripts to update content.");
}

// upstream: lexical-html/src/__tests__/unit/Issue8391WhitespaceAroundUnknownInline.test.ts::issue #8391 — whitespace around unknown inline elements > per-call override: pass the isInline predicate via the context option
#[test]
fn per_call_override_passes_is_inline_via_the_context_option() {
    let editor = build(None);
    import_into(
        &editor,
        Some(GenerateNodesFromDomOptions {
            context: Some(vec![tooltip_inline_context()]),
            ..Default::default()
        }),
    );
    let texts = root_paragraph_text_contents(&editor);
    assert_eq!(texts[0], "In web development, the DOM ");
    assert_eq!(texts[1], TOOLTIP_MARKER);
    assert_eq!(texts[2], " allows scripts to update content.");
}

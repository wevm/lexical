//! Ports of `packages/lexical-html/src/__tests__/unit/CoreImportExtension.test.ts`.

#![cfg(feature = "extension")]

use super::support::{build, find_text_by_content, import_into};
use crate::extension::LexicalEditorWithDispose;
use crate::html::import::{
    DomImportConfigOverrides, DomImportRule, ImportContextPairOrUpdater, WhitespaceImportConfig,
    context_value, core_import_extension, create_import_state, default_is_inline,
    default_preserves_whitespace, define_import_rule, dom_import_extension,
    import_whitespace_config, sel,
};
use crate::{config_extension, create_paragraph_node, create_text_node, get_root};
use std::cell::RefCell;
use std::rc::Rc;

/// `buildEditor()`.
fn build_editor() -> LexicalEditorWithDispose {
    build("core-host", vec![core_import_extension().into()])
}

/// An editor with `CoreImportExtension` and a `DOMImportExtension` config.
fn build_with(
    rules: Vec<DomImportRule>,
    context_defaults: Option<Vec<ImportContextPairOrUpdater>>,
) -> LexicalEditorWithDispose {
    build(
        "host",
        vec![
            core_import_extension().into(),
            config_extension(
                &dom_import_extension(),
                [DomImportConfigOverrides {
                    rules: (!rules.is_empty()).then(|| rules.into_iter().map(Into::into).collect()),
                    context_defaults,
                    ..Default::default()
                }],
            ),
        ],
    )
}

fn first_paragraph() -> crate::NodeRef {
    let para = get_root().get_first_child().expect("expected paragraph");
    assert_eq!(para.get_type(), "paragraph", "expected paragraph");
    para
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > paragraph + text
#[test]
fn paragraph_and_text() {
    let editor = build_editor();
    import_into(&editor, "<p>Hello world</p>");
    editor
        .read(|| assert_eq!(first_paragraph().get_text_content(), "Hello world"))
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > inline format tags propagate via ImportTextFormat
#[test]
fn inline_format_tags_propagate_via_import_text_format() {
    let editor = build_editor();
    import_into(
        &editor,
        "<p>a <strong>b <em>c</em></strong> <code>d</code></p>",
    );
    editor
        .read(|| {
            assert_eq!(find_text_by_content("a ").text_data().get_format(), 0.0);
            assert!(find_text_by_content("b ").has_format("bold"));
            assert!(!find_text_by_content("b ").has_format("italic"));
            assert!(find_text_by_content("c").has_format("bold"));
            assert!(find_text_by_content("c").has_format("italic"));
            assert!(find_text_by_content("d").has_format("code"));
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > span with Google-Docs-style CSS pushes formats into context
#[test]
fn span_with_google_docs_css_pushes_formats_into_context() {
    let editor = build_editor();
    import_into(
        &editor,
        r#"<p><span style="font-weight:700">bold</span> <span style="font-style:italic">italic</span> <span style="text-decoration:underline line-through">both</span></p>"#,
    );
    editor
        .read(|| {
            assert!(find_text_by_content("bold").has_format("bold"));
            assert!(find_text_by_content("italic").has_format("italic"));
            assert!(find_text_by_content("both").has_format("underline"));
            assert!(find_text_by_content("both").has_format("strikethrough"));
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > <b style="font-weight:normal"> (Google Docs wrapper) does NOT add bold
#[test]
fn google_docs_b_wrapper_does_not_add_bold() {
    let editor = build_editor();
    import_into(&editor, r#"<b style="font-weight:normal"><p>plain</p></b>"#);
    editor
        .read(|| assert!(!find_text_by_content("plain").has_format("bold")))
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > <pre> preserves whitespace, splits on \n into LineBreakNode
#[test]
fn pre_preserves_whitespace_and_splits_on_newline() {
    let editor = build_editor();
    import_into(&editor, "<pre>line1\nline2</pre>");
    editor
        .read(|| {
            let children = first_paragraph().get_children();
            assert_eq!(children.len(), 3);
            assert!(children[0].is_text());
            assert_eq!(children[0].get_text_content(), "line1");
            assert_eq!(children[1].get_type(), "linebreak");
            assert!(children[2].is_text());
            assert_eq!(children[2].get_text_content(), "line2");
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > whitespace collapsing matches legacy behavior
#[test]
fn whitespace_collapsing_matches_legacy_behavior() {
    let editor = build_editor();
    import_into(&editor, "<p>  hello   world  </p>");
    editor
        .read(|| assert_eq!(first_paragraph().get_text_content(), "hello world"))
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > <br> creates a LineBreakNode
#[test]
fn br_creates_a_line_break_node() {
    let editor = build_editor();
    import_into(&editor, "<p>a<br>b</p>");
    editor
        .read(|| {
            let children = first_paragraph().get_children();
            assert_eq!(children.len(), 3);
            assert_eq!(children[1].get_type(), "linebreak");
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > paragraph align attribute fallback
#[test]
fn paragraph_align_attribute_fallback() {
    let editor = build_editor();
    import_into(&editor, r#"<p align="center">center</p>"#);
    editor
        .read(|| assert_eq!(first_paragraph().element_data().get_format_type(), "center"))
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > inline style with font-weight:normal clears inherited bold
#[test]
fn font_weight_normal_clears_inherited_bold() {
    let editor = build_editor();
    import_into(
        &editor,
        r#"<p><strong>bold <span style="font-weight: normal">plain</span> bold</strong></p>"#,
    );
    editor
        .read(|| {
            assert!(find_text_by_content("bold ").has_format("bold"));
            assert!(!find_text_by_content("plain").has_format("bold"));
            assert!(find_text_by_content(" bold").has_format("bold"));
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > sub/sup mutex: <sub><sup>x</sup></sub> ⇒ superscript only
#[test]
fn sub_sup_mutex() {
    let editor = build_editor();
    import_into(&editor, "<p><sub><sup>x</sup></sub></p>");
    editor
        .read(|| {
            let text = get_root().get_all_text_nodes()[0].clone();
            assert!(text.has_format("superscript"));
            assert!(!text.has_format("subscript"));
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > text-decoration:none clears inherited underline/strikethrough
#[test]
fn text_decoration_none_clears_inherited_decorations() {
    let editor = build_editor();
    import_into(
        &editor,
        r#"<p><u><s>both <span style="text-decoration: none">neither</span></s></u></p>"#,
    );
    editor
        .read(|| {
            let both = find_text_by_content("both ");
            let neither = find_text_by_content("neither");
            assert!(both.has_format("underline"));
            assert!(both.has_format("strikethrough"));
            assert!(!neither.has_format("underline"));
            assert!(!neither.has_format("strikethrough"));
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > whitespace config can override what counts as preserving whitespace
#[test]
fn whitespace_config_overrides_preserving_whitespace() {
    let editor = build_with(
        vec![],
        Some(vec![context_value(
            &import_whitespace_config(),
            WhitespaceImportConfig {
                is_inline: Rc::new(default_is_inline),
                preserves_whitespace: Rc::new(|node| {
                    default_preserves_whitespace(node)
                        || (node.is_element() && node.class_list().contains("keep-ws"))
                }),
            },
        )]),
    );
    import_into(&editor, "<div class=\"keep-ws\">a   b\nc</div>");
    editor
        .read(|| {
            let children = first_paragraph().get_children();
            assert!(children[0].is_text());
            assert_eq!(children[0].get_text_content(), "a   b");
            assert_eq!(children[1].get_type(), "linebreak");
            assert!(children[2].is_text());
            assert_eq!(children[2].get_text_content(), "c");
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > whitespace config can override what counts as an inline sibling
#[test]
fn whitespace_config_overrides_inline_sibling() {
    let editor = build_with(
        vec![],
        Some(vec![context_value(
            &import_whitespace_config(),
            WhitespaceImportConfig {
                is_inline: Rc::new(|node| {
                    (node.is_element() && node.node_name() == "HR") || default_is_inline(node)
                }),
                preserves_whitespace: Rc::new(default_preserves_whitespace),
            },
        )]),
    );
    import_into(&editor, "<p>a <hr>b</p>");
    editor
        .read(|| {
            assert!(
                get_root()
                    .get_all_text_nodes()
                    .iter()
                    .any(|t| t.get_text_content() == "a "),
                "expected \"a \" (with trailing space)"
            );
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > session can be written by an early rule and read by a later one
#[test]
fn session_written_by_an_early_rule_is_read_by_a_later_one() {
    let style_sheets = create_import_state::<Vec<String>>("collected-styles", Vec::new);
    let sheets = style_sheets.clone();
    let collected_styles_rule = define_import_rule(
        Some("test/collect-style"),
        sel::tag(&["style"]),
        move |ctx, el, _| {
            ctx.session.update(&sheets, |mut prev| {
                prev.push(el.text_content());
                prev
            });
            Ok(vec![])
        },
    );
    let style_aware_rule = define_import_rule(
        Some("test/style-aware"),
        sel::tag(&["article"]),
        move |ctx, el, _| {
            let sheets = ctx.session.get(&style_sheets);
            let p = create_paragraph_node();
            p.append(&[create_text_node(&format!(
                "[saw {} sheets] {}",
                sheets.len(),
                el.text_content()
            ))])?;
            Ok(vec![p])
        },
    );
    let editor = build_with(vec![collected_styles_rule, style_aware_rule], None);
    import_into(
        &editor,
        "<style>.a{}</style><style>.b{}</style><article>body</article>",
    );
    editor
        .read(|| {
            assert_eq!(
                get_root().get_all_text_nodes()[0].get_text_content(),
                "[saw 2 sheets] body"
            );
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > <style> can be overridden by an app-specific rule (default is now a rule, not a framework skip)
#[test]
fn style_can_be_overridden_by_an_app_rule() {
    let captured: Rc<RefCell<Option<String>>> = Rc::default();
    let slot = captured.clone();
    let style_capture_rule = define_import_rule(
        Some("test/style-capture"),
        sel::tag(&["style"]),
        move |_, el, _| {
            *slot.borrow_mut() = Some(el.text_content());
            Ok(vec![])
        },
    );
    let editor = build_with(vec![style_capture_rule], None);
    import_into(&editor, "<style>.foo{color:red}</style><p>x</p>");
    assert_eq!(captured.borrow().as_deref(), Some(".foo{color:red}"));
    editor
        .read(|| assert_eq!(first_paragraph().get_text_content(), "x"))
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > unconverted block elements (not just <div>) preserve block boundaries
#[test]
fn unconverted_block_elements_preserve_block_boundaries() {
    let editor = build_editor();
    import_into(&editor, "<section>a</section><article>b</article>");
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(
                children
                    .iter()
                    .map(|n| n.get_text_content())
                    .collect::<Vec<_>>(),
                ["a", "b"]
            );
            assert!(children.iter().all(|n| n.get_type() == "paragraph"));
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/CoreImportExtension.test.ts::CoreImportExtension > text-align on a non-<div> block element is propagated to its paragraph
#[test]
fn text_align_on_a_non_div_block_is_propagated_to_its_paragraph() {
    let editor = build_editor();
    import_into(&editor, r#"<header style="text-align: right">x</header>"#);
    editor
        .read(|| assert_eq!(first_paragraph().element_data().get_format_type(), "right"))
        .unwrap();
}

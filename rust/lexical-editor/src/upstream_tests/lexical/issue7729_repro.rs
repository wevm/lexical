//! Ports of `packages/lexical/src/__tests__/unit/Issue7729Repro.test.ts`.

#![cfg(all(feature = "html", feature = "extension", feature = "rich-text"))]

use crate::clipboard::insert_generated_nodes;
use crate::dom::parse_from_string;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::{generate_html_from_nodes, generate_nodes_from_dom};
use crate::nodes::paragraph::ParagraphNode;
use crate::{
    InitialEditorStateType, LexicalError, LexicalExtension, UpdateOptions, create_paragraph_node,
    create_text_node, define_extension, get_editor, get_root, select_all,
};
use std::rc::Rc;

fn set_up_editor(initial: Option<InitialEditorStateType>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: initial,
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("issue-7729-repro", ())
    })
    .into()])
    .unwrap()
}

/// `$importHtml(html)`.
fn import_html(html: &str) -> Result<(), LexicalError> {
    let editor = get_editor();
    let dom = parse_from_string(html);
    let nodes = generate_nodes_from_dom(&editor, &dom)?;
    insert_generated_nodes(&editor, nodes, &select_all()?)
}

fn import(editor: &LexicalEditorWithDispose, html: String) {
    editor
        .update(
            move || import_html(&html),
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

fn expect_first_paragraph_indent(editor: &LexicalEditorWithDispose, indent: f64) {
    editor
        .read(|| {
            let para = get_root().get_first_child_or_throw().unwrap();
            assert!(
                para.get_latest().downcast_ref::<ParagraphNode>().is_some(),
                "expected ParagraphNode as first child"
            );
            assert_eq!(para.get_indent().unwrap(), indent);
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/Issue7729Repro.test.ts::Issue #7729: paragraph indent round-trip via data-lexical-indent > exportDOM emits data-lexical-indent and importDOM round-trips
#[test]
fn export_dom_emits_data_lexical_indent_and_import_dom_round_trips() {
    let editor = set_up_editor(Some(InitialEditorStateType::Update(Rc::new(|_| {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("hi")])?;
        paragraph.set_indent(2.0)?;
        get_root().append(&[paragraph])
    }))));
    let e = (*editor).clone();
    let html = editor
        .read(|| generate_html_from_nodes(&e, None).unwrap())
        .unwrap();
    assert!(html.contains(r#"data-lexical-indent="2""#), "{html}");
    assert!(html.contains("padding-inline-start: 80px"), "{html}");
    import(&editor, html);
    expect_first_paragraph_indent(&editor, 2.0);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue7729Repro.test.ts::Issue #7729: paragraph indent round-trip via data-lexical-indent > data-lexical-indent wins over a calc(...) padding-inline-start
#[test]
fn data_lexical_indent_wins_over_a_calc_padding_inline_start() {
    let editor = set_up_editor(None);
    import(
        &editor,
        r#"<p data-lexical-indent="2" style="padding-inline-start: calc(2 * var(--lexical-indent-base-value, 40px));">hi</p>"#.to_owned(),
    );
    expect_first_paragraph_indent(&editor, 2.0);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue7729Repro.test.ts::Issue #7729: paragraph indent round-trip via data-lexical-indent > data-lexical-indent wins over a non-40-multiple padding-inline-start
#[test]
fn data_lexical_indent_wins_over_a_non_40_multiple_padding_inline_start() {
    let editor = set_up_editor(None);
    import(
        &editor,
        r#"<p data-lexical-indent="2" style="padding-inline-start: 32px;">hi</p>"#.to_owned(),
    );
    expect_first_paragraph_indent(&editor, 2.0);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue7729Repro.test.ts::Issue #7729: paragraph indent round-trip via data-lexical-indent > falls back to padding heuristic when data-lexical-indent is absent
#[test]
fn falls_back_to_padding_heuristic_when_data_lexical_indent_is_absent() {
    let editor = set_up_editor(None);
    import(
        &editor,
        r#"<p style="padding-inline-start: 80px;">hi</p>"#.to_owned(),
    );
    expect_first_paragraph_indent(&editor, 2.0);
    editor.dispose();
}

//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/QuoteInsertNewAfter.test.ts`.

#![cfg(feature = "extension")]

use crate::extension::build_editor_from_extensions;
use crate::rich_text::{create_quote_node, rich_text_extension};
use crate::upstream_tests::support::{range_selection, set_test_root_element, update};
use crate::{
    InitialEditorStateType, LexicalExtension, NodeRef, create_paragraph_node, create_text_node,
    define_extension, get_root,
};

/// `BlockSnapshot`.
#[derive(Debug, PartialEq)]
struct BlockSnapshot {
    format: String,
    style: String,
    text_format: f64,
    type_: String,
}

/// `pressEnterAtEndOf($createBlock, boldPending)`.
fn press_enter_at_end_of(create_block: fn() -> NodeRef, bold_pending: bool) -> BlockSnapshot {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("[test-quote-insert-new-after]", ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    update(&editor, move || {
        let block = create_block();
        block.set_element_format("center");
        block.set_element_style("color: red");
        block.append(&[create_text_node("hello")])?;
        let root = get_root();
        root.clear()?;
        root.append(std::slice::from_ref(&block))?;
        block.select_end()?;
        Ok(())
    });
    if bold_pending {
        update(&editor, || range_selection().format_text("bold", None));
    }
    update(&editor, || range_selection().insert_paragraph().map(drop));
    let snapshot = editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(
                children.len(),
                2,
                "expected the block plus its continuation"
            );
            let last = children.last().unwrap();
            let data = last.element_data();
            BlockSnapshot {
                format: data.get_format_type().to_owned(),
                style: data.get_style().to_owned(),
                text_format: data.get_text_format(),
                type_: last.get_type().to_owned(),
            }
        })
        .unwrap();
    editor.dispose();
    snapshot
}

fn quote() -> NodeRef {
    create_quote_node(false)
}

// upstream: lexical-rich-text/src/__tests__/unit/QuoteInsertNewAfter.test.ts::QuoteNode.insertNewAfter > keeps the block alignment and style, as ParagraphNode does
#[test]
fn keeps_the_block_alignment_and_style_as_paragraph_node_does() {
    for create_block in [create_paragraph_node as fn() -> NodeRef, quote] {
        let snapshot = press_enter_at_end_of(create_block, false);
        assert_eq!(snapshot.format, "center");
        assert_eq!(snapshot.style, "color: red");
        assert_eq!(snapshot.type_, "paragraph");
    }
}

// upstream: lexical-rich-text/src/__tests__/unit/QuoteInsertNewAfter.test.ts::QuoteNode.insertNewAfter > seeds the continuation block with the pending text format
#[test]
fn seeds_the_continuation_block_with_the_pending_text_format() {
    let from_paragraph = press_enter_at_end_of(create_paragraph_node, true);
    let from_quote = press_enter_at_end_of(quote, true);
    assert_ne!(from_paragraph.text_format, 0.0);
    assert_eq!(from_quote.text_format, from_paragraph.text_format);
}

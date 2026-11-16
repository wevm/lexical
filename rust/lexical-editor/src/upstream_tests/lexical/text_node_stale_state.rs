//! Ports of `packages/lexical/src/__tests__/unit/TextNodeStaleState.test.ts`.
//!
//! Upstream holds the node object the seeding update made, which the next
//! update's first write leaves behind as a stale version. A [`NodeRef`]
//! resolves the latest version on every call, so the same calls through it
//! must give the same results.

#![cfg(feature = "extension")]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::upstream_tests::support::{update, update_with};
use crate::{
    LexicalExtension, NodeRef, create_paragraph_node, create_text_node, define_extension, get_root,
};

fn build(name: &str) -> LexicalEditorWithDispose {
    build_editor_from_extensions([
        define_extension(LexicalExtension::<(), (), ()>::new(name, ())).into(),
    ])
    .unwrap()
}

/// `seedStaleTextNode(editor)`.
fn seed_stale_text_node(editor: &crate::Editor) -> NodeRef {
    update_with(editor, || {
        let text = create_text_node("a");
        get_root().clear()?;
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text))?;
        get_root().append(&[paragraph])?;
        Ok(text)
    })
}

// upstream: lexical/src/__tests__/unit/TextNodeStaleState.test.ts::TextNode stale state readers > setTextContent can restore the original text through a stale reference
#[test]
fn set_text_content_can_restore_the_original_text_through_a_stale_reference() {
    let editor = build("textnode-stale-state");
    let text = seed_stale_text_node(&editor);
    let t = text.clone();
    update(&editor, move || {
        t.set_text_content("b");
        t.set_text_content("a");
        Ok(())
    });
    assert_eq!(editor.read(|| text.get_text_content()).unwrap(), "a");
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/TextNodeStaleState.test.ts::TextNode stale state readers > setMode can restore the original mode through a stale reference
#[test]
fn set_mode_can_restore_the_original_mode_through_a_stale_reference() {
    let editor = build("textnode-stale-state");
    let text = seed_stale_text_node(&editor);
    let t = text.clone();
    update(&editor, move || {
        t.set_mode("token")?;
        t.set_mode("normal")?;
        Ok(())
    });
    assert_eq!(
        editor.read(|| text.text_data().get_mode()).unwrap(),
        "normal"
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/TextNodeStaleState.test.ts::TextNode stale state readers > isSimpleText reflects the latest mode
#[test]
fn is_simple_text_reflects_the_latest_mode() {
    let editor = build("textnode-stale-state");
    let text = seed_stale_text_node(&editor);
    let t = text.clone();
    update(&editor, move || t.set_mode("token"));
    editor
        .read(|| {
            assert!(text.is_token());
            assert!(!text.is_simple_text());
        })
        .unwrap();
    editor.dispose();
}

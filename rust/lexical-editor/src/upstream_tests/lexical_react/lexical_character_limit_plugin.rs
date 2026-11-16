//! Ports of `packages/lexical-react/src/__tests__/unit/LexicalCharacterLimitPlugin.test.tsx`
//! against [`register_character_limit`], the plugin's logic without React:
//! the count the plugin's `<span className="characters-limit">` shows is
//! what it reports to `remaining_characters`, and its `charset` is the
//! `strlen` it measures with.

#![cfg(all(feature = "overflow", feature = "extension", feature = "rich-text"))]

use super::super::support::{set_test_root_element, update};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::overflow::{Strlen, is_overflow_node, overflow_extension, register_character_limit};
use crate::{
    LexicalExtension, Unregister, create_paragraph_node, create_text_node, define_extension,
    get_root,
};
use std::cell::Cell;
use std::rc::Rc;

const MAX_LENGTH: i64 = 5;

/// `mountWithText(text, charset)`: an editor that already holds `text`,
/// with the plugin mounted on top of it, as when the character-limit
/// setting is turned on. The characters left it reports, the editor, and
/// what unmounts the plugin.
fn mount_with_text(
    text: &'static str,
    strlen: Option<Strlen>,
) -> (Option<i64>, LexicalEditorWithDispose, Unregister) {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            overflow_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("character-limit", ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    update(&editor, move || {
        let root = get_root();
        root.clear()?;
        let p = create_paragraph_node();
        p.append(&[create_text_node(text)])?;
        root.append(&[p])
    });
    let remaining = Rc::new(Cell::new(None));
    let reported = remaining.clone();
    let unregister = register_character_limit(
        &editor,
        MAX_LENGTH,
        strlen,
        Some(Rc::new(move |n| reported.set(Some(n)))),
    )
    .unwrap();
    editor.read(|| {}).unwrap();
    (remaining.get(), editor, unregister)
}

fn overflow_node_count(editor: &crate::Editor) -> usize {
    editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .filter(|n| n.get_parent().is_some_and(|p| is_overflow_node(&p)))
                .count()
        })
        .unwrap()
}

// upstream: lexical-react/src/__tests__/unit/LexicalCharacterLimitPlugin.test.tsx::CharacterLimitPlugin > counts the text that is already in the editor when it mounts
#[test]
fn counts_the_text_that_is_already_in_the_editor_when_it_mounts() {
    let (remaining, editor, unregister) = mount_with_text("hello world", None);
    // 5 - 11
    assert_eq!(remaining, Some(-6));
    assert!(overflow_node_count(&editor) > 0);
    unregister();
    editor.dispose();
}

// upstream: lexical-react/src/__tests__/unit/LexicalCharacterLimitPlugin.test.tsx::CharacterLimitPlugin > reports a full budget for an editor that is under the limit
#[test]
fn reports_a_full_budget_for_an_editor_that_is_under_the_limit() {
    let (remaining, editor, unregister) = mount_with_text("hi", None);
    assert_eq!(remaining, Some(3));
    assert_eq!(overflow_node_count(&editor), 0);
    unregister();
    editor.dispose();
}

// upstream: lexical-react/src/__tests__/unit/LexicalCharacterLimitPlugin.test.tsx::CharacterLimitPlugin > counts in the charset it was given
#[test]
fn counts_in_the_charset_it_was_given() {
    // Three 3-byte characters: 3 UTF-16 code units, 9 UTF-8 bytes.
    let (remaining, editor, unregister) =
        mount_with_text("一二三", Some(Rc::new(|s: &str| s.len() as i64)));
    assert_eq!(remaining, Some(-4));
    unregister();
    editor.dispose();
}

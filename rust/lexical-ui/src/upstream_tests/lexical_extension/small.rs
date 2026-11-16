//! Ports of the small `packages/lexical-extension` unit test files:
//! `deepThemeMergeInPlace.test.ts`, `NormalizeInlineElements.test.ts` and
//! the one case of `NestedEditorExtension.test.ts` not already ported next
//! to the extension (`extension/nested_editor_extension.rs`).

use super::super::support::{TestInlineElementNode, create_test_inline_element_node, update};
use crate::extension::lexical_builder::deep_theme_merge_in_place;
use crate::extension::{
    NestedEditorConfigOverrides, NormalizeInlineElementsConfigOverrides,
    build_editor_from_extensions, nested_editor_extension, normalize_inline_elements_extension,
};
use crate::json::{Value, parse};
use crate::{
    AnyLexicalExtensionArgument, InitialEditorStateType, LexicalExtension, NodeClass,
    config_extension, create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::rc::Rc;

fn j(text: &str) -> Value {
    parse(text).unwrap()
}

// upstream: lexical-extension/src/__tests__/unit/deepThemeMergeInPlace.test.ts::deepThemeMergeInPlace > merges recursively
#[test]
fn deep_theme_merge_merges_recursively() {
    // JS merges into `a` and returns it; the Rust merge takes `a` by value
    // and returns it merged.
    let merged = deep_theme_merge_in_place(
        j(r#"{"a":"a","nested":{"a":1}}"#),
        j(r#"{"b":"b","nested":{"b":2}}"#),
    );
    assert_eq!(merged, j(r#"{"a":"a","nested":{"a":1,"b":2},"b":"b"}"#));
}

// upstream: lexical-extension/src/__tests__/unit/deepThemeMergeInPlace.test.ts::deepThemeMergeInPlace > does not pollute the prototype via __proto__
#[test]
fn deep_theme_merge_skips_proto() {
    // There is no prototype to pollute; what is checked is that the key is
    // not merged.
    let merged = deep_theme_merge_in_place(
        j(r#"{"paragraph":"base"}"#),
        j(r#"{"paragraph":"x","__proto__":{"polluted":1}}"#),
    );
    assert_eq!(merged, j(r#"{"paragraph":"x"}"#));
}

// upstream: lexical-extension/src/__tests__/unit/deepThemeMergeInPlace.test.ts::deepThemeMergeInPlace > does not pollute via constructor/prototype keys
#[test]
fn deep_theme_merge_skips_constructor_and_prototype() {
    let merged = deep_theme_merge_in_place(
        j("{}"),
        j(r#"{"constructor":{"prototype":{"polluted":1}},"prototype":{"polluted":1}}"#),
    );
    assert_eq!(merged, j("{}"));
}

fn inline_editor(
    dependencies: Vec<AnyLexicalExtensionArgument>,
) -> crate::extension::LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let paragraph = create_paragraph_node();
            get_root().append(std::slice::from_ref(&paragraph))?;
            paragraph.append(&[
                create_test_inline_element_node(),
                create_test_inline_element_node(),
            ])?;
            Ok(())
        }))),
        dependencies,
        nodes: vec![NodeClass::of::<TestInlineElementNode>()],
        ..LexicalExtension::<(), (), ()>::new("test", ())
    })
    .into()])
    .unwrap()
}

fn is_test_inline(node: &crate::NodeRef) -> bool {
    node.get_type() == "test_inline_block"
}

// upstream: lexical-extension/src/__tests__/unit/NormalizeInlineElements.test.ts::NormalizeInlineElements > should remove empty inline elements by default
#[test]
fn normalize_inline_elements_removes_empty_inline_elements() {
    let editor = inline_editor(vec![normalize_inline_elements_extension().into()]);
    update(&editor, || {
        let paragraph = get_root().get_first_child().unwrap();
        // An empty inline element cannot be inserted
        assert!(paragraph.is_empty());
        // try to create non-empty and edit it to empty later
        let inline = create_test_inline_element_node();
        inline.append(&[create_text_node("foo")])?;
        paragraph.append(&[inline])?;
        Ok(())
    });
    update(&editor, || {
        let paragraph = get_root().get_first_child().unwrap();
        let inline = paragraph.get_first_child().unwrap();
        assert!(is_test_inline(&inline));
        inline.get_first_child_or_throw()?.remove(false)
    });
    update(&editor, || {
        assert!(get_root().get_first_child().unwrap().is_empty());
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NormalizeInlineElements.test.ts::NormalizeInlineElements > should not to remove empty inline elements if extension is disabled
#[test]
fn normalize_inline_elements_disabled_keeps_them() {
    let editor = inline_editor(vec![config_extension(
        &normalize_inline_elements_extension(),
        [NormalizeInlineElementsConfigOverrides {
            disabled: Some(true),
        }],
    )]);
    update(&editor, || {
        let paragraph = get_root().get_first_child().unwrap();
        assert_eq!(paragraph.get_children_size(), 2);
        assert!(is_test_inline(&paragraph.get_first_child_or_throw()?));
        assert!(is_test_inline(&paragraph.get_last_child_or_throw()?));
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/NestedEditorExtension.test.ts::NestedEditorExtension > inheritEditableFromParent works when configured true
#[test]
fn inherit_editable_from_parent_configured_true() {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        editable: Some(false),
        ..LexicalExtension::<(), (), ()>::new("parent", ())
    })
    .into()])
    .unwrap();
    let child = editor
        .read(|| {
            build_editor_from_extensions([define_extension(LexicalExtension {
                dependencies: vec![config_extension(
                    &nested_editor_extension(),
                    [NestedEditorConfigOverrides {
                        inherit_editable_from_parent: Some(true),
                        ..Default::default()
                    }],
                )],
                editable: Some(false),
                ..LexicalExtension::<(), (), ()>::new("child", ())
            })
            .into()])
        })
        .unwrap()
        .unwrap();
    assert!(!editor.is_editable());
    assert!(!child.is_editable());
    editor.set_editable(true);
    assert!(editor.is_editable());
    assert!(child.is_editable());
    child.dispose();
    editor.dispose();
}

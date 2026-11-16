//! Ports of `packages/lexical-list/src/__tests__/unit/ListExtension.test.ts`.

use super::support::{discrete, list, text_item};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::list::{
    ListConfigOverrides, ListType, check_list_extension, create_list_item_node, is_list_item_node,
    is_list_node, list_extension,
};
use crate::rich_text::rich_text_extension;
use crate::{
    AnyLexicalExtensionArgument, INSERT_PARAGRAPH_COMMAND, InitialEditorStateType,
    LexicalExtension, UpdateOptions, config_extension, create_text_node, define_extension,
    get_root,
};
use std::rc::Rc;

fn build(
    name: &str,
    dependencies: Vec<AnyLexicalExtensionArgument>,
    initial: impl Fn() -> Result<(), crate::LexicalError> + 'static,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

fn checked_item(checked: bool, text: &str) -> crate::NodeRef {
    let item = create_list_item_node(Some(checked));
    item.append(&[create_text_node(text)]).unwrap();
    item
}

// upstream: lexical-list/src/__tests__/unit/ListExtension.test.ts::ListExtension > Creates the list
#[test]
fn list_extension_creates_the_list() {
    let editor = build(
        "[root]",
        vec![list_extension().into(), rich_text_extension().into()],
        || {
            get_root().append(&[list(
                ListType::Number,
                &[text_item("item 1"), text_item("item 2")],
            )])
        },
    );
    editor
        .update(
            || {
                let ol = get_root().get_first_child_or_throw()?;
                assert!(is_list_node(&ol));
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/ListExtension.test.ts::CheckListExtension > Preserves numbering when configured via extension
#[test]
fn preserves_numbering_when_configured_via_extension() {
    let configured_list_extension = config_extension(
        &list_extension(),
        [ListConfigOverrides {
            has_strict_indent: Some(false),
            // The override
            should_preserve_numbering: Some(true),
        }],
    );
    let editor = build(
        "[root-configured]",
        vec![configured_list_extension, rich_text_extension().into()],
        || {
            let list = list(
                ListType::Number,
                &[text_item("A"), create_list_item_node(None), text_item("B")],
            );
            get_root().append(&[list])
        },
    );
    editor
        .update(
            || {
                let list = get_root().get_first_child_or_throw()?;
                if is_list_node(&list) {
                    let empty_item = list.get_child_at_index(1).unwrap();
                    assert!(
                        is_list_item_node(&empty_item),
                        "Expected second child to be a list item"
                    );
                    empty_item.select(None, None)?;
                }
                Ok(())
            },
            UpdateOptions::default(),
        )
        .unwrap();
    editor
        .dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())
        .unwrap();
    editor
        .update(
            || {
                let children = get_root().get_children();
                // We expect 3 children now: List, Paragraph, List
                assert_eq!(children.len(), 3);
                let second_list = &children[2];
                assert!(
                    is_list_node(second_list),
                    "Expected third child to be a list"
                );
                // Since we split at Item 1 (index 0), the next list should
                // start at 2. Item 1 stays in List 1. Item 2 moves to List 2.
                assert_eq!(second_list.get_list_start(), Some(2.0));
                Ok(())
            },
            UpdateOptions::default(),
        )
        .unwrap();
    editor.dispose();
}

// upstream: lexical-list/src/__tests__/unit/ListExtension.test.ts::CheckListExtension > Creates the list
#[test]
fn check_list_extension_creates_the_list() {
    let editor = build(
        "[root]",
        vec![check_list_extension().into(), rich_text_extension().into()],
        || {
            get_root().append(&[list(
                ListType::Check,
                &[
                    checked_item(true, "checked"),
                    checked_item(false, "unchecked"),
                ],
            )])
        },
    );
    editor
        .update(
            || {
                let ul = get_root().get_first_child_or_throw()?;
                assert!(is_list_node(&ul));
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    editor.dispose();
}

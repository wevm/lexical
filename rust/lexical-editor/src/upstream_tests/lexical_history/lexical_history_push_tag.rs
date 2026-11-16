//! Ports of `packages/lexical-history/src/__tests__/unit/LexicalHistoryPushTag.test.ts`.

use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::history::{HistoryConfigOverrides, HistoryNow, history_extension};
use crate::nodes::element::ElementData;
use crate::{
    Editor, HISTORY_MERGE_TAG, HISTORY_PUSH_TAG, LexicalError, LexicalExtension, LexicalNode,
    NodeBase, NodeClass, NodeRef, REDO_COMMAND, UNDO_COMMAND, UpdateOptions, config_extension,
    create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

/// `InlineEmptyElementNode` (`inline-empty`). Empty inline elements change
/// the adjacent TextNode's sibling pointer, without adding another dirty
/// leaf or changing the text's serialized value.
#[derive(Clone, Debug, Default)]
struct InlineEmptyElementNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for InlineEmptyElementNode {
    fn get_type(&self) -> &'static str {
        "inline-empty"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
    fn is_inline(&self) -> bool {
        true
    }
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("span"))
    }
}

/// `$getParagraph()`.
fn get_paragraph() -> NodeRef {
    let paragraph = get_root().get_first_child().unwrap();
    assert_eq!(paragraph.get_type(), "paragraph");
    paragraph
}

/// `readChildren(editor)`.
fn read_children(editor: &Editor) -> Vec<(String, String)> {
    editor
        .read(|| {
            get_paragraph()
                .get_children()
                .iter()
                .map(|node| (node.get_type().to_owned(), node.get_text_content()))
                .collect()
        })
        .unwrap()
}

fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static, tags: &[&str]) {
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                tags: tags.iter().map(|t| (*t).to_owned()).collect(),
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

/// `createEditor()`.
fn create_editor() -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            // Keep every update within the merge window without relying on wall time.
            config_extension(
                &history_extension(),
                [HistoryConfigOverrides {
                    delay: Some(1000.0),
                    disabled: Some(false),
                    now: Some(HistoryNow(Rc::new(|| 0.0))),
                    ..Default::default()
                }],
            ),
        ],
        nodes: vec![NodeClass::of::<InlineEmptyElementNode>()],
        ..LexicalExtension::<(), (), ()>::new("test", ())
    })
    .into()])
    .unwrap();
    update(
        &editor,
        || get_root().append(&[create_paragraph_node()]),
        &[],
    );
    update(
        &editor,
        || {
            let paragraph = get_paragraph();
            paragraph.append(&[create_text_node("ab")])?;
            paragraph.select_end()?;
            Ok(())
        },
        &[],
    );
    editor
}

fn undo_or_redo(editor: &Editor, undo: bool) {
    let inner = editor.clone();
    update(
        editor,
        move || {
            let command = if undo { &UNDO_COMMAND } else { &REDO_COMMAND };
            inner.dispatch_command(command, ())?;
            Ok(())
        },
        &[],
    );
}

fn children(of: &[(&str, &str)]) -> Vec<(String, String)> {
    of.iter()
        .map(|(t, c)| ((*t).to_owned(), (*c).to_owned()))
        .collect()
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistoryPushTag.test.ts::HISTORY_PUSH_TAG with an unchanged dirty TextNode (#9217) > isolates an element insertion with tags [ 'history-push' ]
// upstream: lexical-history/src/__tests__/unit/LexicalHistoryPushTag.test.ts::HISTORY_PUSH_TAG with an unchanged dirty TextNode (#9217) > isolates an element insertion with tags [ 'history-push', 'history-merge' ]
// upstream: lexical-history/src/__tests__/unit/LexicalHistoryPushTag.test.ts::HISTORY_PUSH_TAG with an unchanged dirty TextNode (#9217) > isolates an element insertion with tags [ 'history-merge', 'history-push' ]
#[test]
fn isolates_an_element_insertion_with_tags() {
    let text_children = children(&[("text", "ab")]);
    let inserted_children = children(&[("text", "ab"), ("inline-empty", "")]);
    for tag in [
        &[HISTORY_PUSH_TAG][..],
        &[HISTORY_PUSH_TAG, HISTORY_MERGE_TAG],
        &[HISTORY_MERGE_TAG, HISTORY_PUSH_TAG],
    ] {
        let editor = create_editor();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let heard = calls.clone();
        let unregister = editor
            .register_update_listener(move |payload| heard.borrow_mut().push(payload.clone()));
        update(
            &editor,
            || {
                get_root().get_all_text_nodes()[0]
                    .insert_after(&crate::create::<InlineEmptyElementNode>(), true)?;
                Ok(())
            },
            tag,
        );
        unregister();

        // Verify the regression's dirty-leaf shape so the fixture cannot silently
        // turn into a different insertion that already worked without the fix.
        let calls = calls.take();
        assert_eq!(calls.len(), 1, "{tag:?}");
        let payload = &calls[0];
        let text_key = editor
            .read(|| get_root().get_all_text_nodes()[0].get_key().to_owned())
            .unwrap();
        assert_eq!(payload.dirty_leaves, [text_key], "{tag:?}");
        let export = |state: &crate::EditorState| {
            state.read(|| {
                crate::json::Value::Object(get_root().get_all_text_nodes()[0].export_json(false))
            })
        };
        assert_eq!(
            export(&payload.editor_state),
            export(&payload.prev_editor_state),
            "{tag:?}"
        );
        assert_eq!(read_children(&editor), inserted_children, "{tag:?}");

        // Undo the insertion and earlier text separately, then redo both. Check
        // content and structure: a stack-length assertion alone is insufficient.
        undo_or_redo(&editor, true);
        assert_eq!(read_children(&editor), text_children, "{tag:?}");
        undo_or_redo(&editor, true);
        assert_eq!(read_children(&editor), [], "{tag:?}");
        undo_or_redo(&editor, false);
        assert_eq!(read_children(&editor), text_children, "{tag:?}");
        undo_or_redo(&editor, false);
        assert_eq!(read_children(&editor), inserted_children, "{tag:?}");
    }
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistoryPushTag.test.ts::HISTORY_PUSH_TAG with an unchanged dirty TextNode (#9217) > still merges an untagged TextNode marked dirty without a change
#[test]
fn still_merges_an_untagged_text_node_marked_dirty_without_a_change() {
    let editor = create_editor();
    update(
        &editor,
        || {
            get_root().get_all_text_nodes()[0].mark_dirty();
            Ok(())
        },
        &[],
    );
    let history_state = get_extension_dependency_from_editor(&editor, &history_extension())
        .unwrap()
        .output
        .unwrap()
        .history_state
        .peek();
    assert_eq!(history_state.0.borrow().undo_stack.len(), 1);
    undo_or_redo(&editor, true);
    assert_eq!(read_children(&editor), []);
}

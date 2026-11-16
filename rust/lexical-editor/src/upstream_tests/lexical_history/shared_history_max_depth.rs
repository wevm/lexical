//! Ports of `packages/lexical-history/src/__tests__/unit/SharedHistoryMaxDepth.test.ts`.

use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::history::{
    HistoryConfigOverrides, HistoryExtensionOutput, history_extension, shared_history_extension,
};
use crate::{
    AnyLexicalExtensionArgument, Editor, LexicalExtension, config_extension, define_extension,
};
use std::rc::Rc;

const PARENT_MAX_DEPTH: usize = 7;
const PARENT_DELAY: f64 = 123.0;

fn build(name: &str, dependencies: Vec<AnyLexicalExtensionArgument>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `buildParent()`.
fn build_parent() -> LexicalEditorWithDispose {
    build(
        "shared-max-depth-parent",
        vec![config_extension(
            &history_extension(),
            [HistoryConfigOverrides {
                delay: Some(PARENT_DELAY),
                max_depth: Some(Some(PARENT_MAX_DEPTH)),
                ..Default::default()
            }],
        )],
    )
}

/// `buildChild()`.
fn build_child() -> LexicalEditorWithDispose {
    build(
        "shared-max-depth-child",
        vec![shared_history_extension().into()],
    )
}

/// The child shares the parent's history: `shared.output.parentEditor.value
/// = parent`.
fn link(parent: &Editor, child: &Editor) {
    get_extension_dependency_from_editor(child, &shared_history_extension())
        .unwrap()
        .output
        .unwrap()
        .parent_editor
        .set_value(Some(parent.clone()));
}

fn history_output(editor: &Editor) -> Rc<HistoryExtensionOutput> {
    get_extension_dependency_from_editor(editor, &history_extension())
        .unwrap()
        .output
        .unwrap()
}

// upstream: lexical-history/src/__tests__/unit/SharedHistoryMaxDepth.test.ts::SharedHistoryExtension forwards the parent history signals > the child adopts the parent maxDepth
#[test]
fn the_child_adopts_the_parent_max_depth() {
    let parent = build_parent();
    let child = build_child();
    link(&parent, &child);

    let parent_history = history_output(&parent);
    let child_history = history_output(&child);

    // Premise: the parent really carries the configured cap, and the child's
    // own default differs from it.
    assert_eq!(parent_history.max_depth.peek(), Some(PARENT_MAX_DEPTH));

    // The child pushes onto the parent's shared undoStack, so it has to apply
    // the parent's cap rather than its own default.
    assert_eq!(child_history.max_depth.peek(), Some(PARENT_MAX_DEPTH));
}

// upstream: lexical-history/src/__tests__/unit/SharedHistoryMaxDepth.test.ts::SharedHistoryExtension forwards the parent history signals > the already-forwarded signals still arrive
#[test]
fn the_already_forwarded_signals_still_arrive() {
    let parent = build_parent();
    let child = build_child();
    link(&parent, &child);

    let parent_history = history_output(&parent);
    let child_history = history_output(&child);
    // `delay`, `historyState`, `now`, `maxDepth` and `disabled`.
    assert_eq!(child_history.delay.peek(), parent_history.delay.peek());
    assert!(child_history.history_state.peek() == parent_history.history_state.peek());
    assert!(child_history.now.peek() == parent_history.now.peek());
    assert_eq!(
        child_history.max_depth.peek(),
        parent_history.max_depth.peek()
    );
    assert_eq!(
        child_history.disabled.peek(),
        parent_history.disabled.peek()
    );
}

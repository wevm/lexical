//! Helpers shared by the `@lexical/rich-text` ports.

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::rich_text::rich_text_extension;
use crate::upstream_tests::support::TestBlockableDecoratorNode;
use crate::{
    InitialEditorStateType, KeyboardEvent, LexicalExtension, NodeClass, NodeRef,
    create_node_selection, define_extension, set_selection,
};
use std::rc::Rc;

/// `buildEditorFromExtensions({$initialEditorState, dependencies:
/// [RichTextExtension], name: 'test', nodes: [TestDecoratorNode]})`.
pub(super) fn build_with_decorators(
    initial: impl Fn() -> Result<(), crate::LexicalError> + 'static,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
        dependencies: vec![rich_text_extension().into()],
        nodes: vec![NodeClass::of::<TestBlockableDecoratorNode>()],
        ..LexicalExtension::<(), (), ()>::new("test", ())
    })
    .into()])
    .unwrap()
}

/// A `NodeSelection` of `nodes`, set as the selection.
pub(super) fn select_nodes(nodes: &[NodeRef]) {
    let selection = create_node_selection();
    for node in nodes {
        selection.add(node.get_key());
    }
    set_selection(Some(selection));
}

/// `new KeyboardEvent('keydown', {cancelable: true, key, shiftKey})`.
pub(super) fn key_event(key: &str, shift_key: bool) -> Rc<KeyboardEvent> {
    Rc::new(KeyboardEvent {
        key: key.to_owned(),
        shift_key,
        ..KeyboardEvent::default()
    })
}

/// `editor.dispatchCommand(command, new KeyboardEvent('keydown', {ctrlKey:
/// true, key, shiftKey}))`.
pub(super) fn dispatch_ctrl_key(
    editor: &crate::Editor,
    command: &crate::LexicalCommand<Rc<KeyboardEvent>>,
    key: &str,
    shift_key: bool,
) {
    let event = Rc::new(KeyboardEvent {
        key: key.to_owned(),
        ctrl_key: true,
        shift_key,
        ..KeyboardEvent::default()
    });
    editor.dispatch_command(command, event).unwrap();
}

/// `snapshotSelection(editor)`: the range selection's points, if any.
pub(super) fn snapshot_selection(editor: &crate::Editor) -> Option<(crate::Point, crate::Point)> {
    editor
        .read(|| {
            crate::get_selection()
                .and_then(|s| s.range())
                .map(|s| (s.anchor, s.focus))
        })
        .unwrap()
}

/// A paragraph holding `children`.
pub(super) fn paragraph_with(children: &[NodeRef]) -> Result<NodeRef, crate::LexicalError> {
    let paragraph = crate::create_paragraph_node();
    paragraph.append(children)?;
    Ok(paragraph)
}

/// `$getRoot().clear().append(...children)`.
pub(super) fn set_root_children(children: &[NodeRef]) -> Result<(), crate::LexicalError> {
    let root = crate::get_root();
    root.clear()?;
    root.append(children)
}

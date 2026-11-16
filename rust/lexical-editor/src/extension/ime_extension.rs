//! `IMEExtension.ts`: the input method's composition as signals.

use super::signals::{Signal, effect, signal};
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::{
    COMMAND_PRIORITY_BEFORE_EDITOR, COMPOSITION_END_COMMAND, COMPOSITION_START_COMMAND,
    COMPOSITION_START_TAG, Editor, EditorReadMode, NodeKey, NodeRef, get_node_by_key,
    get_selection, merge_register,
};
use std::rc::Rc;

/// The output of `IMEExtension`. Both signals are kept up to date for the
/// editor's lifetime, whether or not anything watches them.
#[derive(Clone)]
pub struct ImeOutput {
    /// `compositionKey`: the key of the selection's anchor when a
    /// composition started (what Lexical's own `$handleCompositionStart`
    /// marks as composing), which may be an element's; `None` when no
    /// composition is in progress.
    pub composition_key: Signal<Option<NodeKey>>,
    /// `composingTextNode`: the text node being composed, once there is
    /// one. A composition that starts on an element point gets its text
    /// node from the update tagged [`COMPOSITION_START_TAG`]. JS compares
    /// node objects by identity; this compares the nodes' keys.
    pub composing_text_node: Signal<Option<NodeRef>>,
}

/// `IMEExtension`.
pub type ImeExtensionType = LexicalExtension<(), ImeOutput, ()>;

fn make() -> Rc<ImeExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, _: &(), _: &ExtensionBuildState<()>| {
                Ok(ImeOutput {
                    composition_key: signal(None),
                    composing_text_node: signal(None),
                })
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor, _: &(), state: &ExtensionRegisterState<(), ImeOutput>| {
                let output = state.get_output().expect("IMEExtension has a build");
                let composition_key = output.composition_key.clone();
                let remove_start_command = editor.register_command(
                    &COMPOSITION_START_COMMAND,
                    move |_, _| {
                        // `BEFORE_EDITOR` runs just before Lexical's own
                        // `$handleCompositionStart`, which marks the same
                        // anchor key as composing.
                        if let Some(range) = get_selection().and_then(|s| s.range()) {
                            composition_key.set_value(Some(range.anchor.key));
                        }
                        Ok(false)
                    },
                    COMMAND_PRIORITY_BEFORE_EDITOR,
                );

                // Stage 1: resolve the key to a text node when it is one. An
                // element point start resolves to `None` until stage 2.
                let (composition_key, composing_text_node) = (
                    output.composition_key.clone(),
                    output.composing_text_node.clone(),
                );
                let weak = editor.weak();
                let stop_key_effect = effect(move || {
                    let Some(key) = composition_key.value() else {
                        composing_text_node.set_value(None);
                        return None;
                    };
                    let editor = weak.upgrade().map(Editor)?;
                    let node = editor
                        .read_with_mode(EditorReadMode::Latest, || {
                            get_node_by_key(&key).filter(NodeRef::is_text)
                        })
                        .ok()
                        .flatten();
                    composing_text_node.set_value(node);
                    None
                });

                // Stage 2: the update tagged COMPOSITION_START_TAG has the
                // selection in the composing text node.
                let composing_text_node = output.composing_text_node.clone();
                let remove_update_listener = editor.register_update_listener(move |payload| {
                    if !payload.tags.iter().any(|t| t == COMPOSITION_START_TAG) {
                        return;
                    }
                    let node = payload.editor_state.read(|| {
                        get_selection()
                            .and_then(|s| s.range())
                            .and_then(|range| range.anchor.get_node().ok())
                            .filter(NodeRef::is_text)
                    });
                    if let Some(node) = node {
                        composing_text_node.set_value(Some(node));
                    }
                });

                // JS clears the key on the root element's `compositionend`
                // event; the app reports that as COMPOSITION_END_COMMAND.
                let composition_key = output.composition_key.clone();
                let remove_end_command = editor.register_command(
                    &COMPOSITION_END_COMMAND,
                    move |_, _| {
                        composition_key.set_value(None);
                        Ok(false)
                    },
                    COMMAND_PRIORITY_BEFORE_EDITOR,
                );

                let composition_key = output.composition_key.clone();
                let remove_root_listener = editor.register_root_listener(move |root_element, _| {
                    if root_element.is_none() {
                        composition_key.set_value(None);
                    }
                    None
                });

                Ok(merge_register(vec![
                    remove_start_command,
                    stop_key_effect,
                    remove_update_listener,
                    remove_end_command,
                    remove_root_listener,
                ]))
            },
        )),
        ..LexicalExtension::new("@lexical/extension/IME", ())
    })
}

thread_local! {
    static IME_EXTENSION: Rc<ImeExtensionType> = make();
}

/// `IMEExtension`: the input method's composition in progress as two
/// signals ([`ImeOutput`]), so extensions that follow it don't each listen
/// to the composition commands.
///
/// JS clears the composition on the root element's `compositionend` event,
/// which also dispatches `COMPOSITION_END_COMMAND`; here the app reports
/// the event as that command, so the extension clears it there, at
/// [`COMMAND_PRIORITY_BEFORE_EDITOR`], before Lexical's own handler, as
/// Swift's does. Removing the root element clears it too, as in JS.
pub fn ime_extension() -> Rc<ImeExtensionType> {
    IME_EXTENSION.with(Rc::clone)
}

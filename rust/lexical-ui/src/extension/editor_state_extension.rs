//! `EditorStateExtension.ts`.

use super::signals::Signal;
use super::watched_signal::watched_signal;
use crate::extension_core::{ExtensionBuildState, LexicalExtension, define_extension};
use crate::{Editor, EditorState};
use std::rc::Rc;

/// `EditorStateExtension`.
pub type EditorStateExtensionType = LexicalExtension<(), Signal<EditorState>, ()>;

fn make() -> Rc<EditorStateExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |editor: &Editor, _config: &(), _state: &ExtensionBuildState<()>| {
                // Weakly, as the editor keeps the signal.
                let (snapshot, listen) = (editor.weak(), editor.weak());
                Ok(watched_signal(
                    move || {
                        Editor(snapshot.upgrade().expect("the editor is alive")).get_editor_state()
                    },
                    move |editor_state_signal| {
                        let Some(editor) = listen.upgrade().map(Editor) else {
                            return Box::new(|| {});
                        };
                        let editor_state_signal = editor_state_signal.clone();
                        editor.register_update_listener(move |payload| {
                            editor_state_signal.set_value(payload.editor_state.clone());
                        })
                    },
                ))
            },
        )),
        ..LexicalExtension::new("@lexical/extension/EditorState", ())
    })
}

thread_local! {
    static EDITOR_STATE_EXTENSION: Rc<EditorStateExtensionType> = make();
}

/// `EditorStateExtension`: the editor's current state as a signal, which
/// follows the editor's updates while something watches it.
pub fn editor_state_extension() -> Rc<EditorStateExtensionType> {
    EDITOR_STATE_EXTENSION.with(Rc::clone)
}

//! `WatchEditableExtension.ts`.

use super::signals::Signal;
use super::watched_signal::watched_signal;
use crate::Editor;
use crate::extension_core::{ExtensionBuildState, LexicalExtension, define_extension};
use std::rc::Rc;

/// `WatchEditableExtension`.
pub type WatchEditableExtensionType = LexicalExtension<(), Signal<bool>, ()>;

fn make() -> Rc<WatchEditableExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |editor: &Editor, _config: &(), _state: &ExtensionBuildState<()>| {
                // Weakly, as the editor keeps the signal.
                let (snapshot, listen) = (editor.weak(), editor.weak());
                Ok(watched_signal(
                    move || Editor(snapshot.upgrade().expect("the editor is alive")).is_editable(),
                    move |editable_signal| {
                        let Some(editor) = listen.upgrade().map(Editor) else {
                            return Box::new(|| {});
                        };
                        let editable_signal = editable_signal.clone();
                        editor.register_editable_listener(move |editable| {
                            editable_signal.set_value(editable);
                        })
                    },
                ))
            },
        )),
        ..LexicalExtension::new("@lexical/extension/WatchEditable", ())
    })
}

thread_local! {
    static WATCH_EDITABLE_EXTENSION: Rc<WatchEditableExtensionType> = make();
}

/// `WatchEditableExtension`: whether the editor is editable
/// ([`Editor::is_editable`]) as a signal, which follows
/// [`Editor::set_editable`] while something watches it.
pub fn watch_editable_extension() -> Rc<WatchEditableExtensionType> {
    WATCH_EDITABLE_EXTENSION.with(Rc::clone)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::{
        build_editor_from_extensions, effect, get_extension_dependency_from_editor,
    };
    use std::cell::RefCell;

    #[test]
    fn follows_set_editable_while_watched() {
        let editor = build_editor_from_extensions([watch_editable_extension().into()]).unwrap();
        let editable = get_extension_dependency_from_editor(&editor, &watch_editable_extension())
            .unwrap()
            .output
            .unwrap();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        let signal = editable.clone();
        let stop = effect(move || {
            log.borrow_mut().push(signal.value());
            None
        });
        editor.set_editable(false);
        editor.set_editable(false);
        editor.set_editable(true);
        assert_eq!(*seen.borrow(), [true, false, true]);
        stop();
        // Unwatched, it no longer listens; watched again, it takes the
        // editor's value.
        editor.set_editable(false);
        assert!(editable.peek());
        let log = seen.clone();
        let signal = editable.clone();
        let stop = effect(move || {
            log.borrow_mut().push(signal.value());
            None
        });
        assert_eq!(seen.borrow().last(), Some(&false));
        stop();
        editor.dispose();
    }
}

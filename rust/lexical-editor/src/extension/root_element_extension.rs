//! `RootElementExtension.ts`.

use super::signals::Signal;
use super::watched_signal::watched_signal;
use crate::extension_core::{ExtensionBuildState, LexicalExtension, define_extension};
use crate::{Editor, RootElementRef};
use std::rc::Rc;

/// `RootElementExtension`.
pub type RootElementExtensionType = LexicalExtension<(), Signal<Option<RootElementRef>>, ()>;

fn make() -> Rc<RootElementExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |editor: &Editor, _config: &(), _state: &ExtensionBuildState<()>| {
                // Weakly, as the editor keeps the signal.
                let (snapshot, listen) = (editor.weak(), editor.weak());
                Ok(watched_signal(
                    move || {
                        Editor(snapshot.upgrade().expect("the editor is alive")).get_root_element()
                    },
                    move |root_element_signal| {
                        let Some(editor) = listen.upgrade().map(Editor) else {
                            return Box::new(|| {});
                        };
                        let root_element_signal = root_element_signal.clone();
                        editor.register_root_listener(move |root_element, _prev| {
                            root_element_signal.set_value(root_element.cloned());
                            None
                        })
                    },
                ))
            },
        )),
        ..LexicalExtension::new("@lexical/extension/RootElement", ())
    })
}

thread_local! {
    static ROOT_ELEMENT_EXTENSION: Rc<RootElementExtensionType> = make();
}

/// `RootElementExtension`: the editor's root element
/// ([`Editor::get_root_element`]) as a signal, which follows
/// [`Editor::set_root_element`] while something watches it.
pub fn root_element_extension() -> Rc<RootElementExtensionType> {
    ROOT_ELEMENT_EXTENSION.with(Rc::clone)
}

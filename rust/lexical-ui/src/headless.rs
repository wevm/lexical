//! `@lexical/headless` (`packages/lexical-headless/src`): an editor for use
//! without a view, such as on a server, which refuses a root element.

use crate::{CreateEditorArgs, Editor, LexicalError, create_editor};
use std::rc::Rc;

/// `createHeadlessEditor(editorConfig)`: [`create_editor`], marked
/// headless (`_headless`), whose [`set_root_element`](Editor::set_root_element)
/// fails with JS's message. JS also makes `registerDecoratorListener`,
/// `registerRootListener`, `registerMutationListener`, `getRootElement`,
/// `getElementByKey`, `focus` and `blur` throw; here, as in Swift, they
/// behave as on any editor without a root element, which a headless one
/// can't get.
pub fn create_headless_editor(editor_config: CreateEditorArgs) -> Editor {
    let editor = create_editor(editor_config);
    editor.set_headless(Rc::new(|_| {
        Err(LexicalError::new(
            "setRootElement is not supported in headless mode",
        ))
    }));
    editor
}

/// `HeadlessExtension.ts` (the `extension` feature).
#[cfg(feature = "extension")]
mod headless_extension {
    use crate::Editor;
    use crate::error::invariant;
    use crate::extension_core::{ExtensionBuildState, LexicalExtension, define_extension};
    use std::rc::Rc;

    /// `HeadlessExtension`.
    pub type HeadlessExtensionType = LexicalExtension<(), (), ()>;

    fn make() -> Rc<HeadlessExtensionType> {
        define_extension(LexicalExtension {
            build: Some(Rc::new(
                |editor: &Editor, _config: &(), _state: &ExtensionBuildState<()>| {
                    // `markEditorHeadless(editor)`.
                    editor.set_headless(Rc::new(|next_root_element| {
                        invariant!(
                            next_root_element.is_none(),
                            "HeadlessExtension: setRootElement is not supported in headless mode"
                        );
                        Ok(())
                    }));
                    Ok(())
                },
            )),
            ..LexicalExtension::new("@lexical/headless/Headless", ())
        })
    }

    thread_local! {
        static HEADLESS_EXTENSION: Rc<HeadlessExtensionType> = make();
    }

    /// `HeadlessExtension`: its `build` marks the editor headless
    /// (`markEditorHeadless`), after which setting a root element fails,
    /// though setting none is allowed.
    pub fn headless_extension() -> Rc<HeadlessExtensionType> {
        HEADLESS_EXTENSION.with(Rc::clone)
    }
}

#[cfg(feature = "extension")]
pub use headless_extension::{HeadlessExtensionType, headless_extension};

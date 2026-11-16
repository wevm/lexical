//! `AutoFocusExtension.ts`.

use super::named_signals::named_signals_config;
use super::signals::effect;
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::{Editor, EditorFocusOptions};
use std::rc::Rc;

named_signals_config! {
    /// `AutoFocusConfig`.
    pub struct AutoFocusConfig;
    /// `Partial<AutoFocusConfig>`.
    pub struct AutoFocusConfigOverrides;
    /// `namedSignals(AutoFocusConfig)`: the extension's output.
    pub struct AutoFocusOutput {
        /// `defaultSelection`: where the selection goes when the editor is
        /// focused without one, the root's end (`'rootEnd'`) by default.
        default_selection: EditorFocusOptions,
        /// `disabled`.
        disabled: bool,
    }
}

/// `AutoFocusExtension`.
pub type AutoFocusExtensionType = LexicalExtension<AutoFocusConfig, AutoFocusOutput, ()>;

fn make() -> Rc<AutoFocusExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &AutoFocusConfig, _: &ExtensionBuildState<()>| {
                Ok(AutoFocusOutput::of(config))
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _: &AutoFocusConfig,
             state: &ExtensionRegisterState<(), AutoFocusOutput>| {
                let stores = state.get_output().expect("AutoFocusExtension has a build");
                let weak = editor.weak();
                Ok(effect(move || {
                    if stores.disabled.value() {
                        return None;
                    }
                    let editor = weak.upgrade().map(Editor)?;
                    let default_selection = stores.default_selection.clone();
                    let focused = editor.weak();
                    Some(editor.register_root_listener(move |root_element, _| {
                        let editor = focused.upgrade().map(Editor)?;
                        let root_element = root_element.cloned();
                        // If the selection moves to the same point, the view
                        // isn't focused again, so it is focused here.
                        let callback = Box::new(move || {
                            if let Some(root) = root_element
                                && !root.0.has_focus()
                            {
                                root.0.focus();
                            }
                        });
                        // JS's root listener would throw an error of the
                        // update; a root listener can't fail, and a failed
                        // update has already been discarded.
                        let _ = editor.focus(Some(callback), default_selection.peek());
                        None
                    }))
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/extension/AutoFocus",
            AutoFocusConfig {
                default_selection: EditorFocusOptions::RootEnd,
                disabled: false,
            },
        )
    })
}

thread_local! {
    static AUTO_FOCUS_EXTENSION: Rc<AutoFocusExtensionType> = make();
}

/// `AutoFocusExtension`: focuses the editor ([`Editor::focus`]) when a
/// root element is set, typically once, when the editor is first shown.
///
/// JS asks the DOM whether the active element is inside the root, and
/// focuses the root when it isn't; here the root element answers
/// ([`RootElement::has_focus`](crate::RootElement::has_focus)) and is
/// focused ([`RootElement::focus`](crate::RootElement::focus)), as in
/// Swift.
pub fn auto_focus_extension() -> Rc<AutoFocusExtensionType> {
    AUTO_FOCUS_EXTENSION.with(Rc::clone)
}

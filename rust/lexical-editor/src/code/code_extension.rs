//! `CodeExtension.ts`: what `CodeExtension` registers.

use super::code_node::is_code_node;
use crate::{Editor, Unregister};

/// `CodeExtension`'s `register(editor)`, without the extension: a transform
/// that clears the format of a tab in a code block. Its other handler, of
/// `KEY_ENTER_COMMAND` (`$exitCodeNodeOnEnter`), reads a `KeyboardEvent`
/// and waits for an input layer; without the extension `CodeNode`'s
/// `insertNewAfter` exits the block itself, as in JS. Returns what
/// unregisters it.
// KEY_ENTER_COMMAND: `$exitCodeNodeOnEnter(selection)` at
// COMMAND_PRIORITY_LOW goes here, before the transform, once the core has
// the command.
pub fn register_code(editor: &Editor) -> Unregister {
    editor.register_node_transform("tab", |node| {
        if node.text_data().get_format() != 0.0
            && node.get_parent().is_some_and(|p| is_code_node(&p))
        {
            node.set_format(0.0);
        }
        Ok(())
    })
}

#[cfg(feature = "extension")]
pub use extension::*;

#[cfg(feature = "extension")]
mod extension {
    use super::super::code_indentation::register_code_indentation;
    use super::super::code_nodes;
    use super::register_code;
    use crate::extension::named_signals_config;
    use crate::extension::signals::effect;
    use crate::extension_core::{
        ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
    };
    use crate::{Editor, Unregister};
    use std::rc::Rc;

    /// `CodeExtension`.
    pub type CodeExtensionType = LexicalExtension;

    fn make_code_extension() -> Rc<CodeExtensionType> {
        define_extension(LexicalExtension {
            // JS's dependencies, `CoreImportExtension` and
            // `DOMImportExtension` with `CodeImportRules` and the VS Code
            // paste overlay, are the HTML import, which comes with `html`.
            nodes: code_nodes(),
            register: Some(Rc::new(
                |editor: &Editor, _: &(), _: &ExtensionRegisterState<(), ()>| {
                    Ok(register_code(editor) as Unregister)
                },
            )),
            ..LexicalExtension::new("@lexical/code", ())
        })
    }

    named_signals_config! {
        /// `CodeIndentConfig`.
        pub struct CodeIndentConfig;
        /// `Partial<CodeIndentConfig>`.
        pub struct CodeIndentConfigOverrides;
        /// `namedSignals(CodeIndentConfig)`: `CodeIndentExtension`'s output.
        pub struct CodeIndentOutput {
            /// `disabled`: the indent commands aren't registered.
            disabled: bool,
            /// `tabSize`: how many leading spaces of a line outdenting
            /// removes as one level; `None` (the default) removes only
            /// tabs.
            tab_size: Option<usize>,
            /// `escapeWithArrows`: the arrow keys leave a code block with
            /// nothing around it. Only the arrow key handlers read it, and
            /// they wait for an input layer.
            escape_with_arrows: bool,
        }
    }

    /// `CodeIndentExtension`.
    pub type CodeIndentExtensionType = LexicalExtension<CodeIndentConfig, CodeIndentOutput, ()>;

    fn make_code_indent_extension() -> Rc<CodeIndentExtensionType> {
        define_extension(LexicalExtension {
            build: Some(Rc::new(
                |_: &Editor, config: &CodeIndentConfig, _: &ExtensionBuildState<()>| {
                    Ok(CodeIndentOutput::of(config))
                },
            )),
            dependencies: vec![code_extension().into()],
            register: Some(Rc::new(
                |editor: &Editor,
                 _: &CodeIndentConfig,
                 state: &ExtensionRegisterState<(), CodeIndentOutput>| {
                    let stores = state.get_output().expect("CodeIndentExtension has a build");
                    let editor = editor.weak();
                    Ok(effect(move || {
                        if stores.disabled.value() {
                            return None;
                        }
                        let tab_size = stores.tab_size.value();
                        // `registerCodeIndentation(editor, tabSize,
                        // escapeWithArrows)`: JS reads `escapeWithArrows`
                        // here too, for the arrow key handlers.
                        let _ = stores.escape_with_arrows.value();
                        let editor = editor.upgrade().map(Editor)?;
                        Some(register_code_indentation(&editor, tab_size))
                    }))
                },
            )),
            ..LexicalExtension::new(
                "@lexical/code-indent",
                CodeIndentConfig {
                    disabled: false,
                    tab_size: None,
                    escape_with_arrows: false,
                },
            )
        })
    }

    thread_local! {
        static CODE_EXTENSION: Rc<CodeExtensionType> = make_code_extension();
        static CODE_INDENT_EXTENSION: Rc<CodeIndentExtensionType> = make_code_indent_extension();
    }

    /// `CodeExtension`: `CodeNode` and `CodeHighlightNode`, and
    /// [`register_code`]. In an editor built with it, Enter
    /// (`INSERT_PARAGRAPH_COMMAND`) in a code block always adds a line; JS
    /// exits the block on `KEY_ENTER_COMMAND` instead.
    pub fn code_extension() -> Rc<CodeExtensionType> {
        CODE_EXTENSION.with(Rc::clone)
    }

    /// `CodeIndentExtension`: [`code_extension`] and
    /// [`register_code_indentation`] with its `tab_size`, unless `disabled`.
    pub fn code_indent_extension() -> Rc<CodeIndentExtensionType> {
        CODE_INDENT_EXTENSION.with(Rc::clone)
    }
}

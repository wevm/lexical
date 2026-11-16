//! `CodeExtension.ts`: what `CodeExtension` registers.

use super::code_node::{exit_code_node_on_enter, is_code_node};
use crate::{COMMAND_PRIORITY_LOW, Editor, KEY_ENTER_COMMAND, Unregister, get_selection};

/// `CodeExtension`'s `register(editor)`, without the extension: a
/// `KEY_ENTER_COMMAND` handler that leaves a code block after two line
/// breaks at its end (`$exitCodeNodeOnEnter`), and a transform that clears
/// the format of a tab in a code block. Returns what unregisters them.
pub fn register_code(editor: &Editor) -> Unregister {
    let unregister_enter = editor.register_command(
        &KEY_ENTER_COMMAND,
        |event, _| match get_selection().filter(|s| s.is_range()) {
            Some(selection) if exit_code_node_on_enter(&selection)?.is_some() => {
                if let Some(event) = event {
                    event.prevent_default();
                }
                Ok(true)
            }
            _ => Ok(false),
        },
        COMMAND_PRIORITY_LOW,
    );
    let unregister_tab = editor.register_node_transform("tab", |node| {
        if node.text_data().get_format() != 0.0
            && node.get_parent().is_some_and(|p| is_code_node(&p))
        {
            node.set_format(0.0);
        }
        Ok(())
    });
    Box::new(move || {
        unregister_enter();
        unregister_tab();
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
            // `CoreImportExtension` and `DOMImportExtension` with
            // `CodeImportRules` and the VS Code paste overlay: the HTML
            // import, with `html`.
            #[cfg(feature = "html")]
            dependencies: crate::html::import::package_import_dependencies(
                crate::code::code_import_rules(),
                vec![crate::code::install_vscode_code_paste_overlay()],
            ),
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

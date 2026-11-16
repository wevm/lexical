//! `NormalizeInlineElementsExtension.ts`: the transform that removes an
//! inline element once it is empty.

use super::named_signals::named_signals_config;
use super::signals::effect;
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::{Editor, LexicalError, NodeRef, Unregister};
use std::rc::Rc;

/// `deleteEmptyInline(node)`: removes `node` when it is an inline element
/// with no children. JS's `__DEV__` warning for a node whose `canBeEmpty()`
/// is true has no console here.
fn delete_empty_inline(node: &NodeRef) -> Result<(), LexicalError> {
    if node.is_element() && node.is_inline() && node.is_empty() {
        node.remove(false)?;
        if node.can_be_empty() {
            crate::warnings::console_warn(&format!(
                "Empty inline elements are removed from the EditorState, so returning 'true' from {}.canBeEmpty() is not allowed",
                node.get_type()
            ));
        }
    }
    Ok(())
}

/// `NormalizeInlineElementsExtension`'s `register(editor)` with
/// `disabled: false`, without the extension: adds `deleteEmptyInline` to the
/// transforms of every registered `ElementNode` class with an `isInline` of
/// its own (`LinkNode` and `AutoLinkNode` among the ported ones). As in JS
/// it goes straight into each class's transforms, without marking that
/// class's nodes dirty as `registerNodeTransform` does. Returns what
/// removes it again.
pub fn register_normalize_inline_elements(editor: &Editor) -> Unregister {
    let unregisters: Vec<Unregister> = editor
        .registered_nodes()
        .values()
        .filter(|class| (class.is_inline_element)())
        .map(|class| editor.add_node_transform(class.node_type, delete_empty_inline))
        .collect();
    Box::new(move || {
        for unregister in unregisters {
            unregister();
        }
    })
}

named_signals_config! {
    /// `NormalizeInlineElementsConfig`.
    pub struct NormalizeInlineElementsConfig;
    /// `Partial<NormalizeInlineElementsConfig>`.
    pub struct NormalizeInlineElementsConfigOverrides;
    /// `namedSignals(NormalizeInlineElementsConfig)`: the extension's output.
    pub struct NormalizeInlineElementsOutput {
        /// `disabled`: whether the transform is left out.
        disabled: bool,
    }
}

/// `NormalizeInlineElementsExtension`.
pub type NormalizeInlineElementsExtensionType =
    LexicalExtension<NormalizeInlineElementsConfig, NormalizeInlineElementsOutput, ()>;

fn make() -> Rc<NormalizeInlineElementsExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &NormalizeInlineElementsConfig, _: &ExtensionBuildState<()>| {
                Ok(NormalizeInlineElementsOutput::of(config))
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _: &NormalizeInlineElementsConfig,
             state: &ExtensionRegisterState<(), NormalizeInlineElementsOutput>| {
                let stores = state
                    .get_output()
                    .expect("NormalizeInlineElementsExtension has a build");
                let editor = editor.weak();
                Ok(effect(move || {
                    if stores.disabled.value() {
                        return None;
                    }
                    let editor = editor.upgrade().map(Editor)?;
                    Some(register_normalize_inline_elements(&editor))
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/NormalizeInlineElements",
            NormalizeInlineElementsConfig { disabled: false },
        )
    })
}

thread_local! {
    static NORMALIZE_INLINE_ELEMENTS_EXTENSION: Rc<NormalizeInlineElementsExtensionType> = make();
}

/// `NormalizeInlineElementsExtension`: removes inline elements once they
/// are empty, unless `disabled` (a signal of its output).
pub fn normalize_inline_elements_extension() -> Rc<NormalizeInlineElementsExtensionType> {
    NORMALIZE_INLINE_ELEMENTS_EXTENSION.with(Rc::clone)
}

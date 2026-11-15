//! `NodeStructureExtension.ts` (`@experimental node-structure`):
//! `registerNodeStructure` and `NodeStructureExtension`, which turn on the
//! core's node structure normalization (`LexicalStructure.ts`, the crate's
//! [`internal_register_node_structure`]).

use super::named_signals::named_signals_config;
use super::signals::effect;
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::{Editor, Unregister, internal_register_node_structure};
use std::rc::Rc;

named_signals_config! {
    /// `NodeStructureConfig`.
    pub struct NodeStructureConfig;
    /// `Partial<NodeStructureConfig>`.
    pub struct NodeStructureConfigOverrides;
    /// `namedSignals(NodeStructureConfig)`: the extension's output.
    pub struct NodeStructureOutput {
        /// `disabled`: `true` stops enforcing the declarations (they are
        /// then only read). Turning it back to `false` normalizes the whole
        /// document.
        disabled: bool,
    }
}

/// `registerNodeStructure(editor)`: enforces on `editor` the structure node
/// classes declare, as [`node_structure_extension`] does, for code that
/// builds an editor without extensions. The current document is normalized
/// too. Registering more than once is harmless; the normalization stops
/// when every returned function has been called.
pub fn register_node_structure(editor: &Editor) -> Unregister {
    internal_register_node_structure(editor)
}

/// `NodeStructureExtension`.
pub type NodeStructureExtensionType =
    LexicalExtension<NodeStructureConfig, NodeStructureOutput, ()>;

fn make() -> Rc<NodeStructureExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &NodeStructureConfig, _: &ExtensionBuildState<()>| {
                Ok(NodeStructureOutput::of(config))
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _: &NodeStructureConfig,
             state: &ExtensionRegisterState<(), NodeStructureOutput>| {
                let stores = state
                    .get_output()
                    .expect("NodeStructureExtension has a build");
                let editor = editor.weak();
                Ok(effect(move || {
                    if stores.disabled.value() {
                        return None;
                    }
                    let editor = editor.upgrade().map(Editor)?;
                    Some(register_node_structure(&editor))
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/NodeStructure",
            NodeStructureConfig { disabled: false },
        )
    })
}

thread_local! {
    static NODE_STRUCTURE_EXTENSION: Rc<NodeStructureExtensionType> = make();
}

/// `NodeStructureExtension` (`@experimental node-structure`): enforces the
/// structure node classes declare (`children`, `mergeAdjacent`) in the same
/// update as each change: a block in a node that holds inline content is
/// lifted out and the node split around it (a plain paragraph becomes lines
/// of the node instead), inline content in a node that holds blocks is
/// wrapped in a paragraph, a child a list of classes does not allow is
/// wrapped in the first listed class, and equal adjacent siblings are
/// merged. A class that declares `children: 'block'` is also a block
/// container for the structural commands ([`crate::is_block_container`]).
/// Opt-in: without it, the declarations change nothing at runtime.
pub fn node_structure_extension() -> Rc<NodeStructureExtensionType> {
    NODE_STRUCTURE_EXTENSION.with(Rc::clone)
}

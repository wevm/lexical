//! `config.ts`: the node types and classes of an editor configuration.

use crate::NodeClass;
use crate::extension_core::InitialEditorConfig;

/// `KnownTypesAndNodes`: the node types and classes of a configuration, in
/// the order they were added.
#[derive(Clone, Default)]
pub struct KnownTypesAndNodes {
    pub types: Vec<&'static str>,
    pub nodes: Vec<NodeClass>,
}

/// `getKnownTypesAndNodes(config)`: the node types and classes registered
/// in an [`InitialEditorConfig`], for an extension's `init` to register
/// optional behavior when some node is present.
pub fn get_known_types_and_nodes(config: &InitialEditorConfig) -> KnownTypesAndNodes {
    let mut known = KnownTypesAndNodes::default();
    for klass in get_node_config(config) {
        if !known.types.contains(&klass.node_type()) {
            known.types.push(klass.node_type());
            known.nodes.push(*klass);
        }
    }
    known
}

/// `getNodeConfig(config)`: a configuration's node classes.
pub fn get_node_config(config: &InitialEditorConfig) -> &[NodeClass] {
    &config.nodes
}

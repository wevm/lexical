//! `extension-core/defineExtension.ts`: `defineExtension`,
//! `configExtension` and `declarePeerDependency`.

use super::types::{
    AnyLexicalExtensionArgument, Erased, ExtensionConfigBase, LexicalExtension,
    NormalizedPeerDependency,
};
use std::rc::Rc;

/// `defineExtension(extension)`: the extension, behind the `Rc` whose
/// identity the builder tells extensions apart by (JS returns the object
/// itself, compared with `===`). Define each extension once and share the
/// `Rc`: two definitions with one name can't be built together.
pub fn define_extension<C: ExtensionConfigBase, O: 'static, I: 'static>(
    extension: LexicalExtension<C, O, I>,
) -> Rc<LexicalExtension<C, O, I>> {
    Rc::new(extension)
}

/// `configExtension(extension, ...configs)`: the extension with config
/// overrides, for another extension's `dependencies` or for
/// [`build_editor_from_extensions`](crate::extension::build_editor_from_extensions).
/// The builder merges them with `mergeConfig`, or with
/// [`shallow_merge_config`](crate::shallow_merge_config).
pub fn config_extension<C: ExtensionConfigBase, O: 'static, I: 'static>(
    extension: &Rc<LexicalExtension<C, O, I>>,
    configs: impl IntoIterator<Item = C::Partial>,
) -> AnyLexicalExtensionArgument {
    AnyLexicalExtensionArgument {
        extension: extension.clone(),
        configs: configs
            .into_iter()
            .map(|config| Rc::new(config) as Erased)
            .collect(),
    }
}

/// `declarePeerDependency(name, config?)`: a peer dependency on the
/// extension named `name`, whose config is a `C` (JS gives the extension's
/// type as a type argument), optionally with config overrides.
pub fn declare_peer_dependency<C: ExtensionConfigBase>(
    name: &str,
    config: Option<C::Partial>,
) -> NormalizedPeerDependency {
    NormalizedPeerDependency {
        name: name.to_owned(),
        config: config.map(|config| Rc::new(config) as Erased),
    }
}

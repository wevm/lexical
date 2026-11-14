//! `packages/lexical/src/extension-core` (the `extension` feature): the
//! types of the extension API and the helpers that define extensions. JS
//! keeps them in `lexical`, whose packages define their extensions with
//! them; the builder that uses them is `@lexical/extension`'s
//! ([`crate::extension`]). They are re-exported at the crate root, as JS
//! exports them from `lexical`.

mod define_extension;
mod safe_cast;
mod shallow_merge_config;
mod types;

pub use define_extension::{config_extension, declare_peer_dependency, define_extension};
pub use safe_cast::safe_cast;
pub use shallow_merge_config::shallow_merge_config;
pub use types::{
    AbortController, AbortSignal, AnyLexicalExtension, AnyLexicalExtensionArgument,
    ExtensionBuildFn, ExtensionBuildState, ExtensionConfigBase, ExtensionInitFn,
    ExtensionInitState, ExtensionMergeConfigFn, ExtensionRegisterFn, ExtensionRegisterState,
    InitialEditorConfig, InitialEditorStateFn, InitialEditorStateType, LexicalExtension,
    LexicalExtensionDependency, NormalizedPeerDependency,
};
pub(crate) use types::{AnyExtensionDependency, Erased, ExtensionRepAccess};

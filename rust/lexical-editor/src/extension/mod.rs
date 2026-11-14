//! `@lexical/extension`'s decorators (the `extension` feature), from
//! `packages/lexical-extension/src`: `HorizontalRuleNode`, a block
//! decorator (`HorizontalRuleExtension.ts`), and `DecoratorTextNode`, an
//! inline one that carries a text format (`DecoratorTextExtension.ts`),
//! with their `$create`/`$is` functions. Neither has a schema of its own,
//! so their JSON is `LexicalNode`'s, with `DecoratorTextNode`'s `format` as
//! a flat NodeState. Also `NormalizeInlineElementsExtension`'s register
//! ([`register_normalize_inline_elements`]), which `RichTextExtension`
//! depends on.
//!
//!
//! The extension API: [`build_editor_from_extensions`] and the builder
//! (`LexicalBuilder.ts`, `internal/ExtensionRep.ts`), the dependency
//! lookups (`getExtensionDependencyFromEditor`,
//! `getPeerDependencyFromEditor` and their `$` forms), `namedSignals`,
//! `watchedSignal` and the signals it is built on (`signals`, a port of
//! `@preact/signals-core`), `getKnownTypesAndNodes`, and the model-level
//! extensions `InitialStateExtension`, `ClearEditorExtension`,
//! `EditorStateExtension` and `DecoratorTextExtension`. The types it builds
//! on are `lexical`'s `extension-core`, at the crate root
//! ([`LexicalExtension`](crate::LexicalExtension),
//! [`define_extension`](crate::define_extension)...).
//!
//! Not ported: the decorators' `createDOM`, `updateDOM`, `exportDOM` and
//! `importDOM`, which come with `html`; `applyFormatFromStyle`, which reads
//! a CSS style declaration; `HorizontalRuleExtension`, which needs
//! `NodeSelectionExtension`, clicks and the DOM (its `nodes` are in
//! [`extension_nodes`]); and the package's other extensions (rust/PARITY.md).

mod clear_editor_extension;
mod config;
mod decorator_text_extension;
mod editor_state_extension;
mod extension_rep;
mod get_extension_dependency;
mod get_extension_dependency_from_editor;
mod get_peer_dependency_from_editor;
mod horizontal_rule_extension;
mod initial_state_extension;
mod lexical_builder;
mod named_signals;
mod normalize_inline_elements_extension;
pub mod signals;
mod watched_signal;

pub use clear_editor_extension::{
    ClearEditorConfig, ClearEditorConfigOverrides, ClearEditorExtensionType, ClearEditorOutput,
    OnClear, clear_editor_extension, register_clear_editor,
};
pub use config::{KnownTypesAndNodes, get_known_types_and_nodes, get_node_config};
pub use decorator_text_extension::{
    DecoratorTextNode, create_decorator_text_node, decorator_text_extension,
    decorator_text_format_state, is_decorator_text_node,
};
pub use editor_state_extension::{EditorStateExtensionType, editor_state_extension};
pub use get_extension_dependency::{
    get_extension_dependency, get_extension_output, get_peer_dependency,
};
pub use get_extension_dependency_from_editor::get_extension_dependency_from_editor;
pub use get_peer_dependency_from_editor::{
    get_peer_dependency_from_editor, get_peer_dependency_from_editor_or_throw,
};
pub use horizontal_rule_extension::{
    HorizontalRuleNode, create_horizontal_rule_node, is_horizontal_rule_node,
};
pub use initial_state_extension::{
    EditorSetOptions, InitialStateConfig, InitialStateConfigOverrides, InitialStateExtensionType,
    InitialStateInit, initial_state_extension,
};
pub use lexical_builder::{LexicalBuilder, LexicalEditorWithDispose, build_editor_from_extensions};
pub use named_signals::{NamedSignalsOutput, named_signals};
pub use normalize_inline_elements_extension::register_normalize_inline_elements;
pub use signals::{
    ReadonlySignal, Signal, SignalCallback, SignalOptions, SignalValue, batch, computed, effect,
    signal, signal_with_options, untracked,
};
pub use watched_signal::watched_signal;

use crate::NodeClass;

/// The decorator node classes of this package, the `nodes` of
/// `HorizontalRuleExtension` and `DecoratorTextExtension`:
/// `HorizontalRuleNode` and `DecoratorTextNode`.
pub fn extension_nodes() -> Vec<NodeClass> {
    vec![
        NodeClass::of::<HorizontalRuleNode>(),
        NodeClass::of::<DecoratorTextNode>(),
    ]
}

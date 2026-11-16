//! `packages/lexical-html/src/import`: `$inlineStylesFromStyleSheets` and,
//! with the `extension` feature too, `DOMImportExtension`, HTML import through rules that
//! extensions contribute, compiled into a dispatcher when the editor is
//! built (`@experimental` upstream). Swift's port (`swift/Sources/LexicalHTML`)
//! is the model for the parts JS leaves to the browser: `<style>` sheets
//! are parsed by [`crate::dom::parse_style_sheet`], and JS's `RegExp`
//! matchers are functions ([`sel::RegexMatcher`]).

#[cfg(feature = "extension")]
mod compile_import_rules;
#[cfg(feature = "extension")]
mod core_import_rules;
#[cfg(feature = "extension")]
mod define_import_rule;
#[cfg(feature = "extension")]
mod dom_import_extension;
#[cfg(feature = "extension")]
mod import_context;
mod inline_styles_from_style_sheets;
#[cfg(feature = "extension")]
mod parse_css;
#[cfg(feature = "extension")]
mod run_import;
#[cfg(feature = "extension")]
mod schemas;
#[cfg(feature = "extension")]
pub mod sel;

#[cfg(feature = "extension")]
pub use crate::html::internal::context_record::{
    ContextConfig, ContextPairOrUpdater, ContextRecord, context_updater, context_value,
};
#[cfg(feature = "extension")]
pub use compile_import_rules::{CompiledOverlayRules, DomImportRuleEntry, define_overlay_rules};
#[cfg(feature = "extension")]
pub use core_import_rules::{core_import_rules, is_alignment_value};
#[cfg(feature = "extension")]
pub use define_import_rule::{DomImportFn, DomImportRule, ImportNext, define_import_rule};
#[cfg(all(
    feature = "extension",
    any(
        feature = "rich-text",
        feature = "list",
        feature = "link",
        feature = "code"
    )
))]
pub(crate) use dom_import_extension::package_import_dependencies;
#[cfg(feature = "extension")]
pub use dom_import_extension::{
    DomImportConfig, DomImportConfigOverrides, DomImportExtensionOutput, DomImportExtensionType,
    DomPreprocessContext, DomPreprocessFn, GenerateNodesFromDomOptions, PreprocessNext,
    core_import_extension, default_hoist_rule, dom_import_extension,
    generate_nodes_from_dom_via_extension, horizontal_rule_import_rules,
};
#[cfg(feature = "extension")]
pub use import_context::{
    DomNodePredicate, ImportContextPairOrUpdater, ImportSession, ImportSourceKind,
    ImportStateConfig, SharedDataTransfer, WhitespaceImportConfig, create_import_state,
    default_is_inline, default_preserves_whitespace, get_import_context_value, import_overlays,
    import_source, import_source_data_transfer, import_text_format, import_text_style,
    import_whitespace_config, with_import_context,
};
#[cfg(feature = "extension")]
pub use inline_styles_from_style_sheets::inline_styles_from_style_sheets;
pub use inline_styles_from_style_sheets::inline_styles_from_style_sheets_dom;
#[cfg(feature = "extension")]
pub use parse_css::parse_selector;
#[cfg(feature = "extension")]
pub use run_import::{AfterFn, DomImportContext, ImportChildrenOpts, OnChildFn};
#[cfg(feature = "extension")]
pub use schemas::{
    AcceptsFn, ChildSchema, FinalizeFn, OnReject, PackageRunFn, block_schema,
    distribute_inline_wrapper, inline_schema, is_block_level, nested_block_schema,
    propagate_text_align_to_block_children, root_schema,
};

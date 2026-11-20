//! `import/DOMImportExtension.ts`, `import/CoreImportExtension.ts` and
//! `import/HorizontalRuleImportExtension.ts`: HTML import through rules
//! compiled into a dispatcher (`@experimental` upstream). The legacy
//! [`generate_nodes_from_dom`](crate::html::generate_nodes_from_dom) and
//! the nodes' `importDOM` keep working alongside it.

use super::compile_import_rules::{
    CompiledDispatch, DomImportRuleEntry, compile_import_rules, flatten_rule_entries,
};
use super::core_import_rules::{core_import_rules, horizontal_rule_rule};
use super::define_import_rule::{DomImportRule, define_import_rule};
use super::import_context::{ImportContextPairOrUpdater, ImportSession};
use super::inline_styles_from_style_sheets::inline_styles_from_style_sheets;
use super::run_import::{ImportChildrenOpts, run_import};
use super::sel;
use crate::dom::DomNode;
use crate::extension::get_extension_output;
use crate::extension_core::{
    ExtensionBuildState, ExtensionConfigBase, LexicalExtension, config_extension, define_extension,
};
use crate::html::internal::constants::DOM_IMPORT_EXTENSION_NAME;
use crate::html::internal::context_record::{
    ContextKind, ContextRecord, context_from_pairs, get_context_record, with_full_context,
};
use crate::node::NodeRef;
use crate::{Editor, LexicalError};
use std::cell::Cell;
use std::rc::Rc;

/// `DOMPreprocessContext`.
pub struct DomPreprocessContext {
    pub session: ImportSession,
}

/// A preprocessor's `$next`: runs the preprocessors below it.
pub type PreprocessNext<'a> = &'a dyn Fn() -> Result<(), LexicalError>;

/// `DOMPreprocessFn`: runs before the walk. It may change the DOM, write
/// to the session, and call `next` to run the preprocessors below it.
pub type DomPreprocessFn =
    Rc<dyn Fn(&DomNode, &DomPreprocessContext, PreprocessNext<'_>) -> Result<(), LexicalError>>;

/// `GenerateNodesFromDOMOptions`.
#[derive(Clone, Default)]
pub struct GenerateNodesFromDomOptions {
    /// Context over the editor's defaults for this import.
    pub context: Option<Vec<ImportContextPairOrUpdater>>,
    /// Run before (and around) the configured preprocessors; the last runs
    /// first.
    pub preprocess: Option<Vec<DomPreprocessFn>>,
}

/// `DOMImportConfig`.
#[derive(Clone)]
pub struct DomImportConfig {
    /// The rules, tried front to back. A contribution is put in front of
    /// the rules so far, so an extension's rules outrank its dependencies'.
    pub rules: Vec<DomImportRuleEntry>,
    /// Context for every import. Contributions are appended.
    pub context_defaults: Vec<ImportContextPairOrUpdater>,
    /// Run before the walk, from the last entry. Contributions are
    /// appended.
    pub preprocess: Vec<DomPreprocessFn>,
}

/// `Partial<DOMImportConfig>`.
#[derive(Clone, Default)]
pub struct DomImportConfigOverrides {
    pub rules: Option<Vec<DomImportRuleEntry>>,
    pub context_defaults: Option<Vec<ImportContextPairOrUpdater>>,
    pub preprocess: Option<Vec<DomPreprocessFn>>,
}

impl ExtensionConfigBase for DomImportConfig {
    type Partial = DomImportConfigOverrides;

    fn shallow_merge(&self, overrides: &DomImportConfigOverrides) -> Self {
        DomImportConfig {
            rules: overrides
                .rules
                .clone()
                .unwrap_or_else(|| self.rules.clone()),
            context_defaults: overrides
                .context_defaults
                .clone()
                .unwrap_or_else(|| self.context_defaults.clone()),
            preprocess: overrides
                .preprocess
                .clone()
                .unwrap_or_else(|| self.preprocess.clone()),
        }
    }
}

type GenerateFn = Rc<
    dyn Fn(&DomNode, Option<&GenerateNodesFromDomOptions>) -> Result<Vec<NodeRef>, LexicalError>,
>;

/// `DOMImportExtensionOutput`.
pub struct DomImportExtensionOutput {
    generate: GenerateFn,
    /// The record of `contextDefaults` (`@internal` upstream).
    pub defaults: Option<Rc<ContextRecord>>,
}

impl DomImportExtensionOutput {
    /// `$generateNodesFromDOM(dom, options)`: the Lexical nodes for a
    /// document (its body) or a parent node. Call it inside an update.
    pub fn generate_nodes_from_dom(
        &self,
        dom: &DomNode,
        options: Option<&GenerateNodesFromDomOptions>,
    ) -> Result<Vec<NodeRef>, LexicalError> {
        (self.generate)(dom, options)
    }
}

/// `DOMImportExtension`.
pub type DomImportExtensionType = LexicalExtension<DomImportConfig, DomImportExtensionOutput, ()>;

/// `$runPreprocessStack(stack, dom, ctx)`: the last entry first, each
/// deferring to the one below with `next`.
fn run_preprocess_stack(
    stack: &[DomPreprocessFn],
    dom: &DomNode,
    ctx: &DomPreprocessContext,
) -> Result<(), LexicalError> {
    struct Stack<'a> {
        stack: &'a [DomPreprocessFn],
        dom: &'a DomNode,
        ctx: &'a DomPreprocessContext,
        i: Cell<usize>,
    }
    impl Stack<'_> {
        fn next(&self) -> Result<(), LexicalError> {
            let i = self.i.get();
            if i == 0 {
                return Ok(());
            }
            self.i.set(i - 1);
            (self.stack[i - 1])(self.dom, self.ctx, &|| self.next())
        }
    }
    Stack {
        stack,
        dom,
        ctx,
        i: Cell::new(stack.len()),
    }
    .next()
}

/// `DefaultHoistRule`: the last rule tried. An element no other rule takes
/// is replaced by its imported children.
pub fn default_hoist_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/html/default-hoist"),
        sel::any(),
        |ctx, el, _| ctx.import_children(el, &ImportChildrenOpts::default()),
    )
}

fn make_dom_import() -> Rc<DomImportExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |editor: &Editor, config: &DomImportConfig, _: &ExtensionBuildState<()>| {
                let dispatch: Rc<CompiledDispatch> =
                    Rc::new(compile_import_rules(&flatten_rule_entries(&config.rules)));
                let defaults = context_from_pairs(&config.context_defaults, None);
                let config_preprocess = config.preprocess.clone();
                // Weakly, as the editor keeps the output through its builder.
                let weak = editor.weak();
                let session_defaults = defaults.clone();
                let generate: GenerateFn = Rc::new(move |dom, options| {
                    let editor = weak
                        .upgrade()
                        .map(Editor)
                        .ok_or_else(|| LexicalError::new("the editor was dropped"))?;
                    // The session record is the walk's root layer: a fresh
                    // child of the ambient import context (or the editor's
                    // defaults), so session writes never leak outward.
                    let parent_record = get_context_record(ContextKind::Import, &editor)
                        .or_else(|| session_defaults.clone());
                    let from_opts = match options.and_then(|o| o.context.as_ref()) {
                        Some(context) => context_from_pairs(context, parent_record.clone()),
                        None => parent_record.clone(),
                    };
                    let session_record = match (&from_opts, &parent_record) {
                        (Some(from), Some(parent)) if !Rc::ptr_eq(from, parent) => from.clone(),
                        (Some(from), None) => from.clone(),
                        _ => ContextRecord::child_of(parent_record.clone()),
                    };
                    let session = ImportSession {
                        record: session_record.clone(),
                    };
                    let mut stack = config_preprocess.clone();
                    if let Some(preprocess) = options.and_then(|o| o.preprocess.as_ref()) {
                        stack.extend(preprocess.iter().cloned());
                    }
                    run_preprocess_stack(
                        &stack,
                        dom,
                        &DomPreprocessContext {
                            session: session.clone(),
                        },
                    )?;
                    with_full_context(ContextKind::Import, session_record, &editor, || {
                        run_import(dispatch.clone(), &editor, dom, session)
                    })
                });
                Ok(DomImportExtensionOutput { generate, defaults })
            },
        )),
        // `contextDefaults` and `preprocess` append (the preprocess stack
        // runs from the end) while `rules` prepends (dispatch reads front
        // to back). Configs merge in dependency order, so in every case
        // the more dependent contributor wins.
        merge_config: Some(Rc::new(
            |config: &DomImportConfig, partial: &DomImportConfigOverrides| {
                let mut merged = config.shallow_merge(partial);
                if let Some(context_defaults) = &partial.context_defaults {
                    merged.context_defaults =
                        [&config.context_defaults[..], context_defaults].concat();
                }
                if let Some(preprocess) = &partial.preprocess {
                    merged.preprocess = [&config.preprocess[..], preprocess].concat();
                }
                if let Some(rules) = &partial.rules {
                    merged.rules = [&rules[..], &config.rules[..]].concat();
                }
                merged
            },
        )),
        ..LexicalExtension::new(
            DOM_IMPORT_EXTENSION_NAME,
            DomImportConfig {
                rules: vec![default_hoist_rule().into()],
                context_defaults: Vec::new(),
                preprocess: vec![Rc::new(inline_styles_from_style_sheets) as DomPreprocessFn],
            },
        )
    })
}

fn make_core_import() -> Rc<LexicalExtension> {
    define_extension(LexicalExtension {
        dependencies: vec![config_extension(
            &dom_import_extension(),
            [DomImportConfigOverrides {
                rules: Some(core_import_rules().into_iter().map(Into::into).collect()),
                ..DomImportConfigOverrides::default()
            }],
        )],
        ..LexicalExtension::new("@lexical/html/CoreImport", ())
    })
}

thread_local! {
    static DOM_IMPORT_EXTENSION: Rc<DomImportExtensionType> = make_dom_import();
    static CORE_IMPORT_EXTENSION: Rc<LexicalExtension> = make_core_import();
}

/// `DOMImportExtension`: rules contributed through its config
/// ([`DomImportConfig::rules`]) compiled into a dispatcher when the editor
/// is built, run by its output's
/// [`generate_nodes_from_dom`](DomImportExtensionOutput::generate_nodes_from_dom).
pub fn dom_import_extension() -> Rc<DomImportExtensionType> {
    DOM_IMPORT_EXTENSION.with(Rc::clone)
}

/// `CoreImportExtension`: [`core_import_rules`] in a `DOMImportExtension`,
/// the equivalent of the core nodes' `importDOM`. Node extensions that
/// contribute rules depend on it.
pub fn core_import_extension() -> Rc<LexicalExtension> {
    CORE_IMPORT_EXTENSION.with(Rc::clone)
}

/// `HorizontalRuleImportRules`: the `<hr>` rule, which
/// [`core_import_rules`] already includes.
pub fn horizontal_rule_import_rules() -> Vec<DomImportRule> {
    vec![horizontal_rule_rule()]
}

/// `$generateNodesFromDOMViaExtension(dom, options)`: the import of the
/// active editor's `DOMImportExtension`. An error when the editor wasn't
/// built with it.
pub fn generate_nodes_from_dom_via_extension(
    dom: &DomNode,
    options: Option<&GenerateNodesFromDomOptions>,
) -> Result<Vec<NodeRef>, LexicalError> {
    get_extension_output(&dom_import_extension())?
        .ok_or_else(|| LexicalError::new("DOMImportExtension has a build"))?
        .generate_nodes_from_dom(dom, options)
}

/// The dependencies a node package's extension gives itself for HTML
/// import: `CoreImportExtension`, and `DOMImportExtension` with the
/// package's `rules` and `preprocess`.
#[cfg(any(
    feature = "rich-text",
    feature = "list",
    feature = "link",
    feature = "code",
    feature = "table"
))]
pub(crate) fn package_import_dependencies(
    rules: Vec<DomImportRule>,
    preprocess: Vec<DomPreprocessFn>,
) -> Vec<crate::extension_core::AnyLexicalExtensionArgument> {
    vec![
        core_import_extension().into(),
        config_extension(
            &dom_import_extension(),
            [DomImportConfigOverrides {
                rules: (!rules.is_empty()).then(|| rules.into_iter().map(Into::into).collect()),
                preprocess: (!preprocess.is_empty()).then_some(preprocess),
                ..DomImportConfigOverrides::default()
            }],
        ),
    ]
}

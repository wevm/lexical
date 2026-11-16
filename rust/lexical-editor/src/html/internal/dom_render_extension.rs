//! `internal/DOMRenderExtension.ts` and `internal/domOverride.ts`:
//! overrides of how nodes render to DOM and export (`@experimental`
//! upstream).

use super::compile_dom_render_config_overrides::compile_dom_render_config_overrides;
use super::constants::DOM_RENDER_EXTENSION_NAME;
use super::context_record::ContextRecord;
use super::dom_render_runtime::{
    DomRenderRuntime, create_editor_context_record, filter_editor_installed,
};
use super::types::{
    DomOverrideOptions, DomRenderConfig, DomRenderConfigOverrides, DomRenderMatch,
    DomRenderMatchConfig, NodeMatch,
};
use crate::Editor;
use crate::extension_core::{
    ExtensionBuildState, ExtensionConfigBase, ExtensionInitState, InitialEditorConfig,
    LexicalExtension, define_extension,
};
use crate::lexical_dom::{DomExportOutput, ExportOverride, HtmlConfig, get_document};
use std::rc::Rc;

/// `domOverride(nodes, config, options)`: an override of the nodes `nodes`
/// matches, or of every node for `None` (JS `'*'`).
pub fn dom_override(
    nodes: Option<Vec<NodeMatch>>,
    config: DomRenderMatchConfig,
    options: DomOverrideOptions,
) -> Rc<DomRenderMatch> {
    Rc::new(DomRenderMatch {
        nodes,
        config,
        options,
    })
}

impl ExtensionConfigBase for DomRenderConfig {
    type Partial = DomRenderConfigOverrides;

    fn shallow_merge(&self, overrides: &DomRenderConfigOverrides) -> Self {
        DomRenderConfig {
            overrides: overrides
                .overrides
                .clone()
                .unwrap_or_else(|| self.overrides.clone()),
            context_defaults: overrides
                .context_defaults
                .clone()
                .unwrap_or_else(|| self.context_defaults.clone()),
        }
    }
}

/// What `init` keeps: the `nodes` and base `dom` from before it replaced
/// `dom`, which the runtime recompiles from.
pub struct DomRenderInitResult {
    initial_editor_config: InitialEditorConfig,
}

/// `DOMRenderExtensionOutput`.
pub struct DomRenderExtensionOutput {
    /// The editor-level render context (`@internal` upstream).
    pub defaults: Rc<ContextRecord>,
    /// The runtime (`@internal` upstream).
    pub runtime: DomRenderRuntime,
}

/// `DOMRenderExtension`.
pub type DomRenderExtensionType =
    LexicalExtension<DomRenderConfig, DomRenderExtensionOutput, DomRenderInitResult>;

fn make_dom_render() -> Rc<DomRenderExtensionType> {
    // A RootNode export for `$generateDOMFromRoot`.
    let root_export: ExportOverride = Rc::new(|_, _, _| {
        let element = get_document().create_element("div");
        element.set_attribute("role", "textbox");
        Ok(DomExportOutput::element(element))
    });
    define_extension(LexicalExtension {
        init: Some(Rc::new(
            |editor_config: &mut InitialEditorConfig,
             config: &DomRenderConfig,
             _: &ExtensionInitState| {
                // Keep the base `dom` and `nodes` before replacing `dom`, so
                // the runtime can recompile when overrides toggle.
                let initial_editor_config = InitialEditorConfig {
                    nodes: editor_config.nodes.clone(),
                    dom: editor_config.dom.clone(),
                    ..InitialEditorConfig::default()
                };
                let editor_context = create_editor_context_record(&config.context_defaults);
                let installed = filter_editor_installed(&config.overrides, &editor_context);
                editor_config.dom = Some(compile_dom_render_config_overrides(
                    &initial_editor_config,
                    &installed,
                )?);
                Ok(DomRenderInitResult {
                    initial_editor_config,
                })
            },
        )),
        build: Some(Rc::new(
            |editor: &Editor,
             config: &DomRenderConfig,
             state: &ExtensionBuildState<DomRenderInitResult>| {
                let init = state.get_init_result()?;
                let editor_context = create_editor_context_record(&config.context_defaults);
                let runtime = DomRenderRuntime::new(
                    editor,
                    init.initial_editor_config.clone(),
                    config.overrides.clone(),
                    editor_context.clone(),
                );
                Ok(DomRenderExtensionOutput {
                    defaults: editor_context,
                    runtime,
                })
            },
        )),
        merge_config: Some(Rc::new(
            |config: &DomRenderConfig, partial: &DomRenderConfigOverrides| {
                let mut merged = config.shallow_merge(partial);
                if let Some(overrides) = &partial.overrides {
                    merged.overrides = [&config.overrides[..], overrides].concat();
                }
                if let Some(context_defaults) = &partial.context_defaults {
                    merged.context_defaults =
                        [&config.context_defaults[..], context_defaults].concat();
                }
                merged
            },
        )),
        html: HtmlConfig {
            export: vec![("root", root_export)],
            import: Vec::new(),
        },
        ..LexicalExtension::new(DOM_RENDER_EXTENSION_NAME, DomRenderConfig::default())
    })
}

thread_local! {
    static DOM_RENDER_EXTENSION: Rc<DomRenderExtensionType> = make_dom_render();
}

/// `DOMRenderExtension`: overrides ([`DomRenderConfig::overrides`], made
/// with [`dom_override`]) of how nodes render to DOM and export, layered
/// over the editor's render config when it is built, and installed or not
/// by the render context.
pub fn dom_render_extension() -> Rc<DomRenderExtensionType> {
    DOM_RENDER_EXTENSION.with(Rc::clone)
}

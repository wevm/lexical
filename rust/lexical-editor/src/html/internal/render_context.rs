//! `internal/RenderContext.ts`: the render context, which overrides read
//! and gate on.

use super::constants::DOM_RENDER_EXTENSION_NAME;
use super::context_record::{
    ContextKind, ContextPairOrUpdater, ContextRecord, create_context_state, get_context_record,
    get_context_value, with_context,
};
use super::dom_render_extension::{DomRenderExtensionOutput, DomRenderInitResult};
use super::types::{DomRenderConfig, RenderStateConfig};
use crate::extension::get_peer_dependency_from_editor;
use crate::lexical_dom::{EditorDomRenderConfig, get_editor_dom_render_config};
use crate::{Editor, LexicalError};
use std::rc::Rc;

/// `createRenderState(name, getDefaultValue)` (`@experimental`): a typed
/// key of the render context. The default is made once.
pub fn create_render_state<V: Clone + PartialEq + 'static>(
    name: &str,
    get_default_value: impl FnOnce() -> V,
) -> RenderStateConfig<V> {
    create_context_state(ContextKind::Render, name, get_default_value)
}

thread_local! {
    static ROOT: RenderStateConfig<bool> = create_render_state("root", || false);
    static EXPORT: RenderStateConfig<bool> = create_render_state("isExport", || false);
}

/// `RenderContextRoot`: true while exporting from the root itself
/// ([`generate_dom_from_root`](crate::html::generate_dom_from_root)).
pub fn render_context_root() -> RenderStateConfig<bool> {
    ROOT.with(Clone::clone)
}

/// `RenderContextExport`: true while exporting
/// ([`generate_dom_from_nodes`](crate::html::generate_dom_from_nodes) and
/// what calls it).
pub fn render_context_export() -> RenderStateConfig<bool> {
    EXPORT.with(Clone::clone)
}

fn get_output(editor: &Editor) -> Option<Rc<DomRenderExtensionOutput>> {
    get_peer_dependency_from_editor::<DomRenderConfig, DomRenderExtensionOutput, DomRenderInitResult>(
        editor,
        DOM_RENDER_EXTENSION_NAME,
    )
    .ok()
    .flatten()
    .and_then(|dep| dep.output)
}

/// `getDefaultRenderContext(editor)`: the editor-level record of the
/// editor's `DOMRenderExtension`.
fn get_default_render_context(editor: &Editor) -> Option<Rc<ContextRecord>> {
    get_output(editor).map(|output| output.defaults.clone())
}

/// `$getRenderContextValue(cfg, editor)`: the value in the current export's
/// context, or the editor's.
pub fn get_render_context_value<V: Clone + PartialEq + 'static>(
    cfg: &RenderStateConfig<V>,
    editor: &Editor,
) -> V {
    let record = get_context_record(ContextKind::Render, editor)
        .or_else(|| get_default_render_context(editor));
    get_context_value(record.as_ref(), cfg)
}

/// `$setRenderContextValue(cfg, value, editor)`: sets a value in the
/// editor's persistent render context, which reinstalls the overrides
/// whose `disabledForEditor` it flips. Nothing without `DOMRenderExtension`.
pub fn set_render_context_value<V: Clone + PartialEq + 'static>(
    cfg: &RenderStateConfig<V>,
    value: V,
    editor: &Editor,
) -> Result<(), LexicalError> {
    match get_output(editor) {
        Some(output) => output.runtime.set_context_value(cfg, value),
        None => Ok(()),
    }
}

/// `$updateRenderContextValue(cfg, updater, editor)`: as
/// [`set_render_context_value`], with a function of the current value.
pub fn update_render_context_value<V: Clone + PartialEq + 'static>(
    cfg: &RenderStateConfig<V>,
    updater: impl FnOnce(V) -> V,
    editor: &Editor,
) -> Result<(), LexicalError> {
    match get_output(editor) {
        Some(output) => {
            let runtime = &output.runtime;
            let value = updater(get_context_value(Some(&runtime.editor_context), cfg));
            runtime.set_context_value(cfg, value)
        }
        None => Ok(()),
    }
}

/// `$getSessionDOMRenderConfig(editor)`: the render config for the current
/// export, without the overrides its context disables, or the editor's
/// without `DOMRenderExtension`.
pub fn get_session_dom_render_config(
    editor: &Editor,
) -> Result<EditorDomRenderConfig, LexicalError> {
    match get_output(editor) {
        Some(output) => output.runtime.get_session_config(),
        None => Ok(get_editor_dom_render_config(editor)),
    }
}

/// `$withRenderContext(cfg, editor)(f)`: runs `f` with the pairs set over
/// the current render context (or the editor's).
pub fn with_render_context<T>(
    cfg: &[ContextPairOrUpdater],
    editor: &Editor,
    f: impl FnOnce() -> Result<T, LexicalError>,
) -> Result<T, LexicalError> {
    with_context(
        ContextKind::Render,
        get_default_render_context,
        cfg,
        editor,
        f,
    )
}

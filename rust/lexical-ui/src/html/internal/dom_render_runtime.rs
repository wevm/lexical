//! `internal/DOMRenderRuntime.ts`: the editor's render context and the
//! overrides it installs.

use super::compile_dom_render_config_overrides::compile_dom_render_config_overrides;
use super::context_record::{
    ContextKind, ContextPairOrUpdater, ContextRecord, context_from_pairs, get_context_record,
};
use super::types::{DomRenderMatch, RenderContextReader, RenderStateConfig};
use crate::editor::EditorInner;
use crate::extension_core::InitialEditorConfig;
use crate::lexical_dom::{
    EditorDomRenderConfig, get_editor_dom_render_config, set_editor_dom_render_config,
};
use crate::{Editor, LexicalError};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// `createEditorContextRecord(contextDefaults)`: the writable editor-level
/// record, with the defaults set.
pub(crate) fn create_editor_context_record(
    context_defaults: &[ContextPairOrUpdater],
) -> Rc<ContextRecord> {
    let parent = ContextRecord::child_of(None);
    context_from_pairs(context_defaults, Some(parent.clone())).unwrap_or(parent)
}

/// `filterEditorInstalled(overrides, record)`: the overrides whose
/// `disabledForEditor` doesn't hold for the editor's context.
pub(crate) fn filter_editor_installed(
    overrides: &[Rc<DomRenderMatch>],
    record: &Rc<ContextRecord>,
) -> Vec<Rc<DomRenderMatch>> {
    let reader = RenderContextReader {
        record: Some(record.clone()),
    };
    overrides
        .iter()
        .filter(|o| {
            !o.options
                .disabled_for_editor
                .as_ref()
                .is_some_and(|f| f(&reader))
        })
        .cloned()
        .collect()
}

fn same_overrides(a: &[Rc<DomRenderMatch>], b: &[Rc<DomRenderMatch>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| Rc::ptr_eq(a, b))
}

/// `DOMRenderRuntime`: the overrides `DOMRenderExtension` installs, which
/// the editor's render context decides, and that context.
pub struct DomRenderRuntime {
    /// Weakly, as the editor keeps the runtime through its builder.
    editor: Weak<EditorInner>,
    /// The `nodes` and base `dom` from before `init` replaced `dom`: what
    /// every recompile starts from.
    initial_editor_config: InitialEditorConfig,
    overrides: Vec<Rc<DomRenderMatch>>,
    /// The writable editor-level record. Exports read through to it, and
    /// `disabledForEditor` reads it.
    pub editor_context: Rc<ContextRecord>,
    has_session_gates: bool,
    installed: RefCell<Vec<Rc<DomRenderMatch>>>,
    /// The session configs, by the indices of the overrides they leave out.
    session_cache: RefCell<HashMap<String, EditorDomRenderConfig>>,
}

impl DomRenderRuntime {
    pub(crate) fn new(
        editor: &Editor,
        initial_editor_config: InitialEditorConfig,
        overrides: Vec<Rc<DomRenderMatch>>,
        editor_context: Rc<ContextRecord>,
    ) -> Self {
        let installed = filter_editor_installed(&overrides, &editor_context);
        let has_session_gates = overrides
            .iter()
            .any(|o| o.options.disabled_for_session.is_some());
        Self {
            editor: editor.weak(),
            initial_editor_config,
            overrides,
            editor_context,
            has_session_gates,
            installed: RefCell::new(installed),
            session_cache: RefCell::default(),
        }
    }

    fn editor(&self) -> Result<Editor, LexicalError> {
        self.editor
            .upgrade()
            .map(Editor)
            .ok_or_else(|| LexicalError::new("the editor was dropped"))
    }

    /// `setContextValue(cfg, value)`: sets an editor-level value, and
    /// recompiles the editor's render config when that changes which
    /// overrides are installed. JS then recreates the DOM of the nodes the
    /// change affects with a full reconcile; with no DOM to reconcile, the
    /// next export reads the new config.
    pub fn set_context_value<V: Clone + PartialEq + 'static>(
        &self,
        cfg: &RenderStateConfig<V>,
        value: V,
    ) -> Result<(), LexicalError> {
        self.editor_context.set(cfg, value);
        let next = filter_editor_installed(&self.overrides, &self.editor_context);
        if same_overrides(&self.installed.borrow(), &next) {
            return Ok(());
        }
        let dom = compile_dom_render_config_overrides(&self.initial_editor_config, &next)?;
        *self.installed.borrow_mut() = next;
        self.session_cache.borrow_mut().clear();
        set_editor_dom_render_config(&self.editor()?, dom);
        Ok(())
    }

    /// `getSessionConfig()`: the editor's render config without the
    /// overrides the current render context disables for this export.
    pub fn get_session_config(&self) -> Result<EditorDomRenderConfig, LexicalError> {
        let editor = self.editor()?;
        let resident = get_editor_dom_render_config(&editor);
        if !self.has_session_gates {
            return Ok(resident);
        }
        let reader = RenderContextReader {
            record: Some(
                get_context_record(ContextKind::Render, &editor)
                    .unwrap_or_else(|| self.editor_context.clone()),
            ),
        };
        let mut disabled_keys = Vec::new();
        let mut session_set = Vec::new();
        for (i, o) in self.installed.borrow().iter().enumerate() {
            if o.options
                .disabled_for_session
                .as_ref()
                .is_some_and(|f| f(&reader))
            {
                disabled_keys.push(i.to_string());
            } else {
                session_set.push(o.clone());
            }
        }
        if disabled_keys.is_empty() {
            return Ok(resident);
        }
        let key = disabled_keys.join(",");
        if let Some(cfg) = self.session_cache.borrow().get(&key) {
            return Ok(cfg.clone());
        }
        let cfg = compile_dom_render_config_overrides(&self.initial_editor_config, &session_set)?;
        self.session_cache.borrow_mut().insert(key, cfg.clone());
        Ok(cfg)
    }
}

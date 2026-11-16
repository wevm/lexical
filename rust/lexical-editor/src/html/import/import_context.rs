//! `import/ImportContext.ts`: the import context's states, the session the
//! walk writes to, and reading and layering the context.

use super::compile_import_rules::CompiledOverlayRules;
use super::dom_import_extension::{DomImportConfig, DomImportExtensionOutput};
use crate::dom::DomNode;
use crate::events::DataTransfer;
use crate::extension::get_peer_dependency_from_editor;
use crate::html::internal::constants::DOM_IMPORT_EXTENSION_NAME;
use crate::html::internal::context_record::{
    ContextConfig, ContextKind, ContextPairOrUpdater, ContextRecord, create_context_state,
    get_context_record, get_context_value, with_context,
};
use crate::lexical_dom::{is_block_dom_node, is_inline_dom_node};
use crate::set_dom_style::StyleObject;
use crate::{Editor, LexicalError};
use std::rc::Rc;

/// `ImportStateConfig<V>`: a typed key of the import context.
pub type ImportStateConfig<V> = ContextConfig<V>;

/// `ImportContextPairOrUpdater`.
pub type ImportContextPairOrUpdater = ContextPairOrUpdater;

/// `createImportState(name, getDefaultValue)`. The default is made once
/// and shared between every session that reads the state without writing
/// it.
pub fn create_import_state<V: Clone + PartialEq + 'static>(
    name: &str,
    get_default_value: impl FnOnce() -> V,
) -> ImportStateConfig<V> {
    create_context_state(ContextKind::Import, name, get_default_value)
}

/// `ImportSourceKind`: what started the import.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportSourceKind {
    Paste,
    Unknown,
}

/// `IsPreserveWhitespaceDom` and `IsInlineForWhitespace`.
pub type DomNodePredicate = Rc<dyn Fn(&DomNode) -> bool>;

/// `WhitespaceImportConfig`: how text whitespace collapses. Configs
/// compare by the identity of their functions.
#[derive(Clone)]
pub struct WhitespaceImportConfig {
    pub preserves_whitespace: DomNodePredicate,
    pub is_inline: DomNodePredicate,
}

impl PartialEq for WhitespaceImportConfig {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.preserves_whitespace, &other.preserves_whitespace)
            && Rc::ptr_eq(&self.is_inline, &other.is_inline)
    }
}

/// `defaultPreservesWhitespace(node)`: `<pre>`, or `white-space: pre*`.
pub fn default_preserves_whitespace(node: &DomNode) -> bool {
    if !node.is_element() {
        return false;
    }
    if node.node_name() == "PRE" {
        return true;
    }
    node.style()
        .get_property_value("white-space")
        .starts_with("pre")
}

/// `defaultIsInline(node)`: text, or an element with an inline `display`,
/// or else an inline tag that isn't a block tag.
pub fn default_is_inline(node: &DomNode) -> bool {
    if node.is_text() {
        return true;
    }
    if !node.is_element() {
        return false;
    }
    let display = node.style().get_property_value("display");
    if !display.is_empty() {
        return display.starts_with("inline");
    }
    if is_block_dom_node(node) {
        return false;
    }
    is_inline_dom_node(node)
}

struct States {
    source: ImportStateConfig<ImportSourceKind>,
    source_data_transfer: ImportStateConfig<Option<SharedDataTransfer>>,
    text_format: ImportStateConfig<i32>,
    text_style: ImportStateConfig<StyleObject>,
    whitespace: ImportStateConfig<WhitespaceImportConfig>,
    overlays: ImportStateConfig<Vec<Rc<CompiledOverlayRules>>>,
}

thread_local! {
    static STATES: States = States {
        source: create_import_state("importSource", || ImportSourceKind::Unknown),
        source_data_transfer: create_import_state("importSourceDataTransfer", || None),
        text_format: create_import_state("textFormat", || 0),
        text_style: create_import_state("textStyle", StyleObject::new),
        whitespace: create_import_state("whitespaceConfig", || WhitespaceImportConfig {
            is_inline: Rc::new(default_is_inline),
            preserves_whitespace: Rc::new(default_preserves_whitespace),
        }),
        overlays: create_import_state("importOverlays", Vec::new),
    };
}

/// `ImportSource`: how this import was started (`Unknown` by default).
pub fn import_source() -> ImportStateConfig<ImportSourceKind> {
    STATES.with(|s| s.source.clone())
}

/// A `DataTransfer` shared with the import context, equal only to itself,
/// as JS compares objects.
#[derive(Clone, Debug)]
pub struct SharedDataTransfer(pub Rc<DataTransfer>);

impl PartialEq for SharedDataTransfer {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// `ImportSourceDataTransfer` (`@experimental` upstream): the
/// `DataTransfer` a paste came from (`None` by default), for rules and
/// preprocessors that look at its other MIME types.
/// `ClipboardDOMImportExtension` sets it.
pub fn import_source_data_transfer() -> ImportStateConfig<Option<SharedDataTransfer>> {
    STATES.with(|s| s.source_data_transfer.clone())
}

/// `ImportTextFormat`: the text format bits for text nodes made in the
/// current subtree.
pub fn import_text_format() -> ImportStateConfig<i32> {
    STATES.with(|s| s.text_format.clone())
}

/// `ImportTextStyle`: CSS properties for text nodes made in the current
/// subtree (`getStyleObjectFromCSS`'s shape).
pub fn import_text_style() -> ImportStateConfig<StyleObject> {
    STATES.with(|s| s.text_style.clone())
}

/// `ImportWhitespaceConfig`.
pub fn import_whitespace_config() -> ImportStateConfig<WhitespaceImportConfig> {
    STATES.with(|s| s.whitespace.clone())
}

/// `ImportOverlays`: overlays in effect for the whole walk; later entries
/// are higher priority.
pub fn import_overlays() -> ImportStateConfig<Vec<Rc<CompiledOverlayRules>>> {
    STATES.with(|s| s.overlays.clone())
}

/// `ImportSession`: the walk's store, the root layer of its context record.
/// Reads fall through to the editor's `contextDefaults`; writes go to this
/// layer.
#[derive(Clone)]
pub struct ImportSession {
    pub(crate) record: Rc<ContextRecord>,
}

impl ImportSession {
    pub fn get<V: Clone + PartialEq + 'static>(&self, cfg: &ImportStateConfig<V>) -> V {
        get_context_value(Some(&self.record), cfg)
    }

    pub fn set<V: Clone + PartialEq + 'static>(&self, cfg: &ImportStateConfig<V>, value: V) {
        self.record.set(cfg, value);
    }

    pub fn update<V: Clone + PartialEq + 'static>(
        &self,
        cfg: &ImportStateConfig<V>,
        updater: impl FnOnce(V) -> V,
    ) {
        let value = updater(self.get(cfg));
        self.record.set(cfg, value);
    }

    /// Whether this session wrote the state.
    pub fn has<V: Clone + PartialEq + 'static>(&self, cfg: &ImportStateConfig<V>) -> bool {
        self.record.has_own(cfg)
    }
}

/// `getDefaultImportContext(editor)`: the `contextDefaults` record of the
/// editor's `DOMImportExtension`.
fn get_default_import_context(editor: &Editor) -> Option<Rc<ContextRecord>> {
    get_peer_dependency_from_editor::<DomImportConfig, DomImportExtensionOutput, ()>(
        editor,
        DOM_IMPORT_EXTENSION_NAME,
    )
    .ok()
    .flatten()
    .and_then(|dep| dep.output)
    .and_then(|output| output.defaults.clone())
}

/// `$getImportContextValue(cfg, editor)`.
pub fn get_import_context_value<V: Clone + PartialEq + 'static>(
    cfg: &ImportStateConfig<V>,
    editor: &Editor,
) -> V {
    let record = get_context_record(ContextKind::Import, editor)
        .or_else(|| get_default_import_context(editor));
    get_context_value(record.as_ref(), cfg)
}

/// `$withImportContext(cfg, editor)(f)`: runs `f` with the pairs set over
/// the editor's current import context.
pub fn with_import_context<T>(
    cfg: &[ImportContextPairOrUpdater],
    editor: &Editor,
    f: impl FnOnce() -> Result<T, LexicalError>,
) -> Result<T, LexicalError> {
    with_context(
        ContextKind::Import,
        get_default_import_context,
        cfg,
        editor,
        f,
    )
}

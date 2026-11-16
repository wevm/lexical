//! `BlockIdHTMLExtension.ts` (`@experimental` upstream): opt-in HTML
//! support for [`block_id_extension`]. Exported HTML carries each
//! identified block's id in a `data-block-id` attribute (or, with
//! `handles`, its handle in `data-block-handle`), and HTML imported through
//! `DOMImportExtension` reads the id back, through the uniqueness rule of
//! `BlockIdExtension`, so ids in pasted HTML never clash with ids in the
//! document. The legacy [`generate_nodes_from_dom`](super::generate_nodes_from_dom)
//! (`importDOM`) does not read the attribute.

use super::import::CompiledOverlayRules;
use super::import::{
    DomImportConfigOverrides, DomImportContext, DomImportRule, DomPreprocessContext,
    DomPreprocessFn, ImportNext, ImportStateConfig, PreprocessNext, create_import_state,
    define_import_rule, define_overlay_rules, dom_import_extension, import_overlays,
    sel::{self, AttrMatch},
};
use super::internal::dom_render_extension::{dom_override, dom_render_extension};
use super::internal::types::{
    DomOverrideOptions, DomRenderConfigOverrides, DomRenderMatch, DomRenderMatchConfig,
};
use crate::dom::DomNode;
use crate::extension::{
    BlockIdExtensionType, Signal, block_id_extension, block_id_state, get_block_handle,
    get_block_id, get_extension_dependency, get_extension_output, signal,
};
use crate::extension_core::{
    ExtensionBuildState, ExtensionConfigBase, ExtensionInitState, ExtensionRegisterState,
    InitialEditorConfig, LexicalExtension, config_extension, define_extension,
};
use crate::json::Value;
use crate::lexical_dom::DomExportOutput;
use crate::lexical_utils::{get_node_by_key, is_root_or_shadow_root};
use crate::node::NodeRef;
use crate::node_state::set_state;
use crate::{Editor, LexicalError};
use std::cell::RefCell;
use std::rc::Rc;

/// `BLOCK_ID_ATTRIBUTE`: the attribute that carries a block id in HTML.
pub const BLOCK_ID_ATTRIBUTE: &str = "data-block-id";

/// `BLOCK_HANDLE_ATTRIBUTE`: the attribute that carries a block handle
/// ([`get_block_handle`]) in HTML exported with `handles`. Import never
/// reads it.
pub const BLOCK_HANDLE_ATTRIBUTE: &str = "data-block-handle";

/// `BLOCK_WRAPPER_ID_ATTRIBUTE`: the attribute that carries the id of a
/// block whose export removes its own element and hands its children to
/// another (a list item that only wraps a nested list is exported as that
/// list, inside the previous item's element). The id goes on the first
/// element the block's element held, and import puts it back on the block
/// that ends up holding only what that element produced, where the import
/// of that element, its parent or its grandparent makes it.
pub const BLOCK_WRAPPER_ID_ATTRIBUTE: &str = "data-block-wrapper-id";

/// `BlockIdHTMLConfig`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockIdHtmlConfig {
    /// Write [`BLOCK_ID_ATTRIBUTE`] on the exported element of every
    /// identified block. Default: `true`.
    pub export: bool,
    /// With `export`, write each block's handle in
    /// [`BLOCK_HANDLE_ATTRIBUTE`] instead of its id, for HTML shown to a
    /// model. Default: `false`.
    pub handles: bool,
    /// Read [`BLOCK_ID_ATTRIBUTE`] back in `DOMImportExtension`'s import,
    /// onto the block the element produced. An invalid id is ignored, a
    /// valid one goes through `BlockIdExtension`'s uniqueness rule, and an
    /// id is dropped from a block that isn't identified once inserted.
    /// Default: `true`.
    pub import: bool,
}

impl Default for BlockIdHtmlConfig {
    fn default() -> Self {
        Self {
            export: true,
            handles: false,
            import: true,
        }
    }
}

/// `Partial<BlockIdHTMLConfig>`.
#[derive(Clone, Debug, Default)]
pub struct BlockIdHtmlConfigOverrides {
    pub export: Option<bool>,
    pub handles: Option<bool>,
    pub import: Option<bool>,
}

impl ExtensionConfigBase for BlockIdHtmlConfig {
    type Partial = BlockIdHtmlConfigOverrides;

    fn shallow_merge(&self, overrides: &BlockIdHtmlConfigOverrides) -> Self {
        Self {
            export: overrides.export.unwrap_or(self.export),
            handles: overrides.handles.unwrap_or(self.handles),
            import: overrides.import.unwrap_or(self.import),
        }
    }
}

/// `namedSignals(config)`: the config as signals, to change at runtime.
pub struct BlockIdHtmlOutput {
    pub export: Signal<bool>,
    pub handles: Signal<bool>,
    pub import: Signal<bool>,
}

/// The init result: the keys of the blocks that got an id from HTML in
/// this update, in the order they got it (a JS `Set`). Opaque outside this
/// module (`unknown` upstream).
#[derive(Default)]
pub struct BlockIdHtmlRuntime {
    imported: RefCell<Vec<String>>,
    /// How many waiting wrapper ids the resolver has looked at, the work
    /// upstream's tests count with spies on `getParent` and
    /// `getChildrenSize`.
    wrapper_id_lookups: std::cell::Cell<usize>,
}

impl BlockIdHtmlRuntime {
    /// How many waiting wrapper ids the resolver has looked at.
    #[cfg(test)]
    pub(crate) fn wrapper_id_lookups(&self) -> usize {
        self.wrapper_id_lookups.get()
    }

    fn add(&self, key: String) {
        let mut imported = self.imported.borrow_mut();
        if !imported.contains(&key) {
            imported.push(key);
        }
    }
}

/// `BlockIdHTMLExtension`.
pub type BlockIdHtmlExtensionType =
    LexicalExtension<BlockIdHtmlConfig, BlockIdHtmlOutput, BlockIdHtmlRuntime>;

/// `$isBlockForId(node)`: an element or decorator that isn't inline.
fn is_block_for_id(node: &NodeRef) -> bool {
    (node.is_element() || node.is_decorator()) && !node.is_inline()
}

/// `blockIdState.parse(attribute)`: the id, or `None` when it is missing or
/// invalid.
fn parse_attribute(value: Option<String>) -> Option<String> {
    block_id_state().parse(&value.map_or(Value::Null, Value::String))
}

fn html_dependency() -> Result<(Rc<BlockIdHtmlOutput>, Rc<BlockIdHtmlRuntime>), LexicalError> {
    let dep = get_extension_dependency(&block_id_html_extension())?;
    let missing = || LexicalError::new("BlockIdHTMLExtension has a build and an init");
    Ok((
        dep.output.ok_or_else(missing)?,
        dep.init.ok_or_else(missing)?,
    ))
}

fn is_identified_block(node: &NodeRef) -> Result<bool, LexicalError> {
    let output = get_extension_output::<_, _, _>(&block_id_extension())?
        .ok_or_else(|| LexicalError::new("BlockIdExtension has a build"))?;
    Ok((output.is_identified_block.0)(node))
}

/// `BlockIdImportRule`: after the element's import, the first block it
/// produced (it may also produce inline siblings or hoisted children after
/// it) takes the element's id, unless it has one.
fn block_id_import_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/html/block-id"),
        sel::any().attr(BLOCK_ID_ATTRIBUTE, AttrMatch::Present),
        |_: &DomImportContext, el: &DomNode, next: ImportNext<'_>| {
            let nodes = next()?;
            let (output, runtime) = html_dependency()?;
            if !output.import.peek() {
                return Ok(nodes);
            }
            if let Some(id) = parse_attribute(el.get_attribute(BLOCK_ID_ATTRIBUTE))
                && let Some(node) = nodes.iter().find(|node| is_block_for_id(node))
                && get_block_id(node)?.is_none()
            {
                set_state(node, &block_id_state(), Some(id))?;
                runtime.add(node.get_key().to_owned());
            }
            Ok(nodes)
        },
    )
}

/// The ids waiting in one import session (`PendingWrapperIds`' value).
#[derive(Default)]
struct PendingWrapperIdsState {
    /// The waiting ids, by the key of the node they wait for (a JS `Map`).
    pending: Vec<(String, String)>,
    /// The ids still looked for, in the order they were found: the node's
    /// key and the element that carried the id. An id is looked for by the
    /// elements around that element up to its grandparent, and dropped
    /// after.
    active: Vec<(String, DomNode)>,
}

impl PendingWrapperIdsState {
    fn get(&self, key: &str) -> Option<&String> {
        self.pending
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, id)| id)
    }

    fn set(&mut self, key: String, id: String) {
        match self.pending.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = id,
            None => self.pending.push((key, id)),
        }
    }

    fn delete(&mut self, key: &str) {
        self.pending.retain(|(k, _)| k != key);
    }
}

/// A session's [`PendingWrapperIdsState`], shared by the rules of its walk
/// and equal only to itself.
#[derive(Clone)]
struct PendingWrapperIds(Rc<RefCell<PendingWrapperIdsState>>);

impl PartialEq for PendingWrapperIds {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// `PendingWrapperIds`: per import session (an import run inside another
/// has its own); `None` where the HTML carries no wrapper id.
fn pending_wrapper_ids() -> ImportStateConfig<Option<PendingWrapperIds>> {
    thread_local! {
        static STATE: ImportStateConfig<Option<PendingWrapperIds>> =
            create_import_state("blockWrapperIds", || None);
    }
    STATE.with(Clone::clone)
}

/// `BlockWrapperIdImportRule`: after the element's import, the first block
/// it produced waits for the element's wrapper id.
fn block_wrapper_id_import_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/html/block-wrapper-id"),
        sel::any().attr(BLOCK_WRAPPER_ID_ATTRIBUTE, AttrMatch::Present),
        |ctx: &DomImportContext, el: &DomNode, next: ImportNext<'_>| {
            let nodes = next()?;
            let (output, _) = html_dependency()?;
            if !output.import.peek() {
                return Ok(nodes);
            }
            if let Some(id) = parse_attribute(el.get_attribute(BLOCK_WRAPPER_ID_ATTRIBUTE))
                && let Some(waiting) = ctx.session.get(&pending_wrapper_ids())
                && let Some(node) = nodes.iter().find(|node| is_block_for_id(node))
            {
                let mut waiting = waiting.0.borrow_mut();
                waiting.set(node.get_key().to_owned(), id);
                waiting.active.push((node.get_key().to_owned(), el.clone()));
            }
            Ok(nodes)
        },
    )
}

/// `BlockWrapperIdResolveRule`: around each element's import, of the
/// wrapper ids found in its children or grandchildren, each one's node, if
/// its parent now holds it alone and is a block without an id (the item
/// the list importer wraps a nested list in), gives that parent the id. An
/// id is looked at by three elements at most, so the cost is linear in the
/// ids.
fn block_wrapper_id_resolve_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/html/block-wrapper-id-resolve"),
        sel::any(),
        |ctx: &DomImportContext, el: &DomNode, next: ImportNext<'_>| {
            let waiting = ctx.session.get(&pending_wrapper_ids());
            let from = waiting.as_ref().map_or(0, |w| w.0.borrow().active.len());
            let nodes = next()?;
            let Some(waiting) = waiting else {
                return Ok(nodes);
            };
            let mut waiting = waiting.0.borrow_mut();
            if waiting.active.len() == from {
                return Ok(nodes);
            }
            let (_, runtime) = html_dependency()?;
            let mut kept = from;
            for k in from..waiting.active.len() {
                runtime
                    .wrapper_id_lookups
                    .set(runtime.wrapper_id_lookups.get() + 1);
                let (key, entry_el) = waiting.active[k].clone();
                let Some(id) = waiting.get(&key).cloned() else {
                    continue;
                };
                let holder = get_node_by_key(&key).and_then(|node| node.get_parent());
                if let Some(holder) = holder.filter(|holder| {
                    holder.get_children_size() == 1
                        && !is_root_or_shadow_root(holder)
                        && is_block_for_id(holder)
                }) && get_block_id(&holder)?.is_none()
                {
                    set_state(&holder, &block_id_state(), Some(id))?;
                    runtime.add(holder.get_key().to_owned());
                    waiting.delete(&key);
                } else if entry_el == *el || entry_el.parent_element().as_ref() == Some(el) {
                    // Its parent, or its grandparent, looks again.
                    waiting.active[kept] = (key, entry_el);
                    kept += 1;
                } else {
                    waiting.delete(&key);
                }
            }
            waiting.active.truncate(kept);
            Ok(nodes)
        },
    )
}

thread_local! {
    /// `BlockIdImportOverlay` and `BlockWrapperIdResolveOverlay`, made once:
    /// overlays compare by identity.
    static OVERLAYS: (Rc<CompiledOverlayRules>, Rc<CompiledOverlayRules>) = (
        define_overlay_rules(&[
            block_id_import_rule().into(),
            block_wrapper_id_import_rule().into(),
        ]),
        define_overlay_rules(&[block_wrapper_id_resolve_rule().into()]),
    );
}

/// Whether a node below `node` carries [`BLOCK_WRAPPER_ID_ATTRIBUTE`]
/// (`node.querySelector(...) !== null`).
fn contains_wrapper_id(node: &DomNode) -> bool {
    node.child_nodes().iter().any(|child| {
        (child.is_element() && child.has_attribute(BLOCK_WRAPPER_ID_ATTRIBUTE))
            || contains_wrapper_id(child)
    })
}

/// `$installBlockIdImportOverlay`: installs the import rules as a
/// walk-wide overlay, so they wrap whichever rule converts the element,
/// whatever the order of the extensions that contribute rules. The
/// resolver runs for every element, so only where the HTML carries a
/// wrapper id.
fn install_block_id_import_overlay(
    dom: &DomNode,
    ctx: &DomPreprocessContext,
    next: PreprocessNext<'_>,
) -> Result<(), LexicalError> {
    let wrapped = contains_wrapper_id(dom);
    if wrapped {
        ctx.session.set(
            &pending_wrapper_ids(),
            Some(PendingWrapperIds(Rc::default())),
        );
    }
    let (import, resolve) = OVERLAYS.with(Clone::clone);
    ctx.session.update(&import_overlays(), |mut prev| {
        prev.push(import);
        if wrapped {
            prev.push(resolve);
        }
        prev
    });
    next()
}

/// The `'*'` export override: the id (or handle) on the element of every
/// identified block, and the wrapper id where the export hands the block's
/// children to another element.
fn block_id_export_override() -> Rc<DomRenderMatch> {
    dom_override(
        None,
        DomRenderMatchConfig {
            export_dom: Some(Rc::new(|_, node, next, _| {
                let mut output: DomExportOutput = next()?;
                let (html, _) = html_dependency()?;
                let Some(element) = output.element.clone().filter(DomNode::is_element) else {
                    return Ok(output);
                };
                if !html.export.peek() || !is_identified_block(node)? {
                    return Ok(output);
                }
                if html.handles.peek() {
                    if let Some(handle) = get_block_handle(node)? {
                        element.set_attribute(BLOCK_HANDLE_ATTRIBUTE, &handle);
                    }
                } else if let Some(id) = get_block_id(node)? {
                    element.set_attribute(BLOCK_ID_ATTRIBUTE, &id);
                    if let Some(after) = output.after.take() {
                        // Where the export removes the block's element and
                        // hands its children to another element (a list item
                        // that only wraps a nested list), the id goes on the
                        // first element it held.
                        output.after = Some(Box::new(move |generated: &DomNode| {
                            let first = generated
                                .is_element()
                                .then(|| generated.children().first().cloned())
                                .flatten();
                            let result = after(generated)?;
                            if generated.is_element()
                                && generated.parent_node().is_none()
                                && result.as_ref() == Some(generated)
                                && let Some(first) = first
                                && first.parent_node().as_ref() != Some(generated)
                                && first.parent_node().is_some()
                            {
                                first.set_attribute(BLOCK_WRAPPER_ID_ATTRIBUTE, &id);
                            }
                            Ok(result)
                        }));
                    }
                }
                Ok(output)
            })),
            ..DomRenderMatchConfig::default()
        },
        DomOverrideOptions {
            // Left out of export sessions while `export` is off, so that
            // nothing runs for each exported element.
            disabled_for_session: Some(Rc::new(|_| {
                !get_extension_output(&block_id_html_extension())
                    .ok()
                    .flatten()
                    .is_some_and(|html| html.export.peek())
            })),
            ..DomOverrideOptions::default()
        },
    )
}

fn make() -> Rc<BlockIdHtmlExtensionType> {
    let block_id: Rc<BlockIdExtensionType> = block_id_extension();
    define_extension(LexicalExtension {
        dependencies: vec![
            block_id.into(),
            config_extension(
                &dom_render_extension(),
                [DomRenderConfigOverrides {
                    overrides: Some(vec![block_id_export_override()]),
                    ..DomRenderConfigOverrides::default()
                }],
            ),
            config_extension(
                &dom_import_extension(),
                [DomImportConfigOverrides {
                    preprocess: Some(vec![
                        Rc::new(install_block_id_import_overlay) as DomPreprocessFn
                    ]),
                    ..DomImportConfigOverrides::default()
                }],
            ),
        ],
        init: Some(Rc::new(
            |_: &mut InitialEditorConfig, _: &BlockIdHtmlConfig, _: &ExtensionInitState| {
                Ok(BlockIdHtmlRuntime::default())
            },
        )),
        build: Some(Rc::new(
            |_: &Editor,
             config: &BlockIdHtmlConfig,
             _: &ExtensionBuildState<BlockIdHtmlRuntime>| {
                Ok(BlockIdHtmlOutput {
                    export: signal(config.export),
                    handles: signal(config.handles),
                    import: signal(config.import),
                })
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _: &BlockIdHtmlConfig,
             state: &ExtensionRegisterState<BlockIdHtmlRuntime, BlockIdHtmlOutput>| {
                let runtime = state.get_init_result()?;
                // Root transforms run after the other transforms of a pass,
                // once the imported blocks are inserted, so the predicate can
                // see where they are.
                Ok(editor.register_node_transform("root", move |_| {
                    let imported = std::mem::take(&mut *runtime.imported.borrow_mut());
                    for key in imported {
                        if let Some(node) = get_node_by_key(&key)
                            && node.is_attached()
                            && !is_identified_block(&node)?
                        {
                            set_state(&node, &block_id_state(), None)?;
                        }
                    }
                    Ok(())
                }))
            },
        )),
        ..LexicalExtension::new("@lexical/html/BlockIdHTML", BlockIdHtmlConfig::default())
    })
}

thread_local! {
    static BLOCK_ID_HTML_EXTENSION: Rc<BlockIdHtmlExtensionType> = make();
}

/// `BlockIdHTMLExtension` (`@experimental` upstream). See the module
/// documentation. It lives in `html` rather than in `BlockIdExtension` as
/// `@lexical/html` depends on `@lexical/extension` upstream.
pub fn block_id_html_extension() -> Rc<BlockIdHtmlExtensionType> {
    BLOCK_ID_HTML_EXTENSION.with(Rc::clone)
}

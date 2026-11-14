//! `import/runImport.ts`: the walk. Each DOM node is dispatched to the
//! rules whose selectors match it (the overlays first, the most recently
//! installed first, then the configured rules), and a rule's `$next` goes
//! on to the next one. With no rule left, the node's children take its
//! place. With `DOMImportContext` and `ImportChildrenOpts` from
//! `import/types.ts`.

use super::compile_import_rules::{CompiledDispatch, CompiledOverlayRules, get_dispatch_indices};
use super::import_context::{
    ImportContextPairOrUpdater, ImportSession, ImportStateConfig, get_import_context_value,
    import_overlays, with_import_context,
};
use super::schemas::{ChildSchema, apply_schema, root_schema};
use super::sel::ImportCaptures;
use crate::dom::DomNode;
use crate::node::NodeRef;
use crate::{Editor, LexicalError};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A function of each child a traversal produces: a substitute, or `None`
/// to drop it.
pub type OnChildFn = Rc<dyn Fn(NodeRef) -> Result<Option<NodeRef>, LexicalError>>;

/// A function of all the children a traversal produced.
pub type AfterFn = Rc<dyn Fn(Vec<NodeRef>) -> Result<Vec<NodeRef>, LexicalError>>;

/// `ImportChildrenOpts`.
#[derive(Clone, Default)]
pub struct ImportChildrenOpts {
    /// Validates and repackages the produced children.
    pub schema: Option<ChildSchema>,
    /// `$onChild`.
    pub on_child: Option<OnChildFn>,
    /// `$after`: runs on all the children before the schema.
    pub after: Option<AfterFn>,
    /// Context for the children traversal.
    pub context: Option<Vec<ImportContextPairOrUpdater>>,
    /// Rules tried before every other for this traversal.
    pub rules: Option<Rc<CompiledOverlayRules>>,
}

/// The walk's state (`Runtime`).
pub(crate) struct Runtime {
    dispatch: Rc<CompiledDispatch>,
    editor: Editor,
    session: ImportSession,
    /// The overlays `$importChildren({rules})` installed; the last is tried
    /// first.
    overlays: RefCell<Vec<Rc<CompiledDispatch>>>,
}

/// `DOMImportContext`: what a rule's `$import` receives.
pub struct DomImportContext {
    /// The captures of this rule's selector.
    pub captures: ImportCaptures,
    /// The walk's shared store.
    pub session: ImportSession,
    runtime: Rc<Runtime>,
}

impl DomImportContext {
    /// `get(cfg)`: a typed context value.
    pub fn get<V: Clone + PartialEq + 'static>(&self, cfg: &ImportStateConfig<V>) -> V {
        get_import_context_value(cfg, &self.runtime.editor)
    }

    /// The editor being imported into.
    pub fn editor(&self) -> &Editor {
        &self.runtime.editor
    }

    /// `$importChildren(parent, opts)`.
    pub fn import_children(
        &self,
        parent: &DomNode,
        opts: &ImportChildrenOpts,
    ) -> Result<Vec<NodeRef>, LexicalError> {
        import_children_internal(&self.runtime, parent, opts)
    }

    /// `$importOne(node, {context})`.
    pub fn import_one(
        &self,
        node: &DomNode,
        context: Option<&[ImportContextPairOrUpdater]>,
    ) -> Result<Vec<NodeRef>, LexicalError> {
        import_one_internal(&self.runtime, node, context)
    }
}

fn import_children_internal(
    runtime: &Rc<Runtime>,
    parent: &DomNode,
    opts: &ImportChildrenOpts,
) -> Result<Vec<NodeRef>, LexicalError> {
    let overlay = opts.rules.as_ref().map(|r| r.dispatch.clone());
    if let Some(overlay) = &overlay {
        runtime.overlays.borrow_mut().push(overlay.clone());
    }
    let run = || import_children_run(runtime, parent, opts);
    let result = match &opts.context {
        Some(context) => with_import_context(context, &runtime.editor, run),
        None => run(),
    };
    if overlay.is_some() {
        runtime.overlays.borrow_mut().pop();
    }
    result
}

fn import_children_run(
    runtime: &Rc<Runtime>,
    parent: &DomNode,
    opts: &ImportChildrenOpts,
) -> Result<Vec<NodeRef>, LexicalError> {
    let mut collected = Vec::new();
    for child in parent.child_nodes() {
        for lex in import_one_internal(runtime, &child, None)? {
            match &opts.on_child {
                Some(on_child) => collected.extend(on_child(lex)?),
                None => collected.push(lex),
            }
        }
    }
    let after_applied = match &opts.after {
        Some(after) => after(collected)?,
        None => collected,
    };
    match &opts.schema {
        Some(schema) => apply_schema(schema, after_applied, None, Some(parent)),
        None => Ok(after_applied),
    }
}

fn import_one_internal(
    runtime: &Rc<Runtime>,
    node: &DomNode,
    context: Option<&[ImportContextPairOrUpdater]>,
) -> Result<Vec<NodeRef>, LexicalError> {
    match context {
        Some(context) => with_import_context(context, &runtime.editor, || dispatch(runtime, node)),
        None => dispatch(runtime, node),
    }
}

/// The rules to try for a node: its candidates in each overlay (top of the
/// stack first) and then in the configured rules.
struct Chain<'a> {
    runtime: &'a Rc<Runtime>,
    node: &'a DomNode,
    candidates: Vec<(Rc<CompiledDispatch>, Vec<usize>)>,
    group: Cell<usize>,
    rule: Cell<usize>,
}

impl Chain<'_> {
    /// `$next()`.
    fn next(&self) -> Result<Vec<NodeRef>, LexicalError> {
        while self.group.get() < self.candidates.len() {
            let (dispatch, indices) = &self.candidates[self.group.get()];
            while self.rule.get() < indices.len() {
                let rule = &dispatch.rules[indices[self.rule.get()]];
                self.rule.set(self.rule.get() + 1);
                let mut captures = ImportCaptures::new();
                if (rule.predicate)(self.node, &mut captures) {
                    let ctx = DomImportContext {
                        captures,
                        session: self.runtime.session.clone(),
                        runtime: self.runtime.clone(),
                    };
                    // JS also logs the rule's name with `console.error` in
                    // development builds; the error propagates the same way.
                    return (rule.import)(&ctx, self.node, &|| self.next());
                }
            }
            self.group.set(self.group.get() + 1);
            self.rule.set(0);
        }
        hoist_children_of(self.runtime, self.node)
    }
}

/// `$dispatch(runtime, node)`.
fn dispatch(runtime: &Rc<Runtime>, node: &DomNode) -> Result<Vec<NodeRef>, LexicalError> {
    let mut candidates = Vec::new();
    for d in runtime.overlays.borrow().iter().rev() {
        let idx = get_dispatch_indices(d, node);
        if !idx.is_empty() {
            candidates.push((d.clone(), idx.to_vec()));
        }
    }
    let main_idx = get_dispatch_indices(&runtime.dispatch, node);
    if !main_idx.is_empty() {
        candidates.push((runtime.dispatch.clone(), main_idx.to_vec()));
    }
    if candidates.is_empty() {
        return hoist_children_of(runtime, node);
    }
    Chain {
        runtime,
        node,
        candidates,
        group: Cell::new(0),
        rule: Cell::new(0),
    }
    .next()
}

/// `$hoistChildrenOf(runtime, node)`: with no rule left, the node's
/// children take its place.
fn hoist_children_of(runtime: &Rc<Runtime>, node: &DomNode) -> Result<Vec<NodeRef>, LexicalError> {
    let mut collected = Vec::new();
    for child in node.child_nodes() {
        collected.extend(import_one_internal(runtime, &child, None)?);
    }
    Ok(collected)
}

/// `$runImport(dispatch, editor, dom, session)`: imports the children of
/// `dom` (a document's body) through `RootSchema`.
pub(crate) fn run_import(
    dispatch: Rc<CompiledDispatch>,
    editor: &Editor,
    dom: &DomNode,
    session: ImportSession,
) -> Result<Vec<NodeRef>, LexicalError> {
    let installed = session.get(&import_overlays());
    let runtime = Rc::new(Runtime {
        dispatch,
        editor: editor.clone(),
        session,
        overlays: RefCell::new(installed.iter().map(|o| o.dispatch.clone()).collect()),
    });
    let root_parent = if dom.is_document() {
        dom.body().unwrap_or_else(|| dom.clone())
    } else {
        dom.clone()
    };
    import_children_run(
        &runtime,
        &root_parent,
        &ImportChildrenOpts {
            schema: Some(root_schema()),
            ..ImportChildrenOpts::default()
        },
    )
}

//! `internal/ContextRecord.ts`: the layered context records the import and
//! render pipelines read typed values from.
//!
//! JS keys a record by each config's symbol and chains records through
//! prototypes; here a record holds its own values by config id, with a link
//! to its parent. JS compares a value with the parent's with `===` to tell
//! whether a pair changes anything; here values compare with `PartialEq`,
//! which for the function-holding values (whitespace config, overlays) is
//! identity.

use crate::Editor;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Which pipeline a record belongs to (JS `DOMImportContextSymbol` and
/// `DOMRenderContextSymbol`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContextKind {
    Import,
    Render,
}

thread_local! {
    static NEXT_CONFIG_ID: Cell<u64> = const { Cell::new(1) };
    /// `activeContext`.
    static ACTIVE_CONTEXT: RefCell<Option<EditorContext>> = const { RefCell::new(None) };
}

struct ConfigInner<V> {
    id: u64,
    name: String,
    default_value: V,
}

/// `ContextConfig<Sym, V>`: a typed key of a context record with its
/// default value, made by `createImportState` or `createRenderState`.
/// Configs are told apart by identity, as JS keys them by a fresh
/// `Symbol(name)`.
pub struct ContextConfig<V>(Rc<ConfigInner<V>>);

impl<V> Clone for ContextConfig<V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<V: Clone + PartialEq + 'static> ContextConfig<V> {
    /// The config's name (its symbol's description).
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// `defaultValue`.
    pub fn default_value(&self) -> V {
        self.0.default_value.clone()
    }
}

/// `createContextState(tag, name, getDefaultValue)`. The default is made
/// once and shared. JS tags a config with its pipeline only for the type
/// checker; so does this, in the signature.
pub(crate) fn create_context_state<V: Clone + PartialEq + 'static>(
    _kind: ContextKind,
    name: &str,
    get_default_value: impl FnOnce() -> V,
) -> ContextConfig<V> {
    let id = NEXT_CONFIG_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    ContextConfig(Rc::new(ConfigInner {
        id,
        name: name.to_owned(),
        default_value: get_default_value(),
    }))
}

/// `ContextRecord<Sym>`: one layer of context values over its parent's.
#[derive(Default)]
pub struct ContextRecord {
    parent: Option<Rc<ContextRecord>>,
    values: RefCell<HashMap<u64, Rc<dyn Any>>>,
}

impl ContextRecord {
    /// `Object.create(parent || null)`: an empty layer.
    pub(crate) fn child_of(parent: Option<Rc<ContextRecord>>) -> Rc<ContextRecord> {
        Rc::new(ContextRecord {
            parent,
            values: RefCell::default(),
        })
    }

    /// `record[key]`, through the parents.
    fn lookup(&self, id: u64) -> Option<Rc<dyn Any>> {
        if let Some(value) = self.values.borrow().get(&id) {
            return Some(value.clone());
        }
        self.parent.as_ref().and_then(|p| p.lookup(id))
    }

    /// `record[key] = value` on this layer.
    pub(crate) fn set<V: Clone + PartialEq + 'static>(&self, cfg: &ContextConfig<V>, value: V) {
        self.values.borrow_mut().insert(cfg.0.id, Rc::new(value));
    }

    /// `Object.hasOwn(record, key)`.
    pub(crate) fn has_own<V>(&self, cfg: &ContextConfig<V>) -> bool {
        self.values.borrow().contains_key(&cfg.0.id)
    }
}

/// `getContextValue(record, cfg)`: the value in `record` or its parents, or
/// the config's default.
pub fn get_context_value<V: Clone + PartialEq + 'static>(
    record: Option<&Rc<ContextRecord>>,
    cfg: &ContextConfig<V>,
) -> V {
    record
        .and_then(|r| r.lookup(cfg.0.id))
        .and_then(|v| v.downcast_ref::<V>().cloned())
        .unwrap_or_else(|| cfg.default_value())
}

type Resolve = Rc<dyn Fn(Option<&Rc<ContextRecord>>) -> Rc<dyn Any>>;
type SameAs = Rc<dyn Fn(Option<&Rc<ContextRecord>>, &dyn Any) -> bool>;

/// `ContextConfigPair` or `ContextConfigUpdater`: a value for a config, or
/// a function of its current value. Make one with [`context_value`] or
/// [`context_updater`].
#[derive(Clone)]
pub struct ContextPairOrUpdater {
    id: u64,
    resolve: Resolve,
    /// Whether a value equals the config's value in a record.
    same_as: SameAs,
}

fn pair_of<V: Clone + PartialEq + 'static>(
    cfg: &ContextConfig<V>,
    resolve: Resolve,
) -> ContextPairOrUpdater {
    let same = cfg.clone();
    ContextPairOrUpdater {
        id: cfg.0.id,
        resolve,
        same_as: Rc::new(move |record, value| {
            value.downcast_ref::<V>() == Some(&get_context_value(record, &same))
        }),
    }
}

/// `contextValue(cfg, value)` (`@experimental`).
pub fn context_value<V: Clone + PartialEq + 'static>(
    cfg: &ContextConfig<V>,
    value: V,
) -> ContextPairOrUpdater {
    pair_of(cfg, Rc::new(move |_| Rc::new(value.clone())))
}

/// `contextUpdater(cfg, updater)` (`@experimental`).
pub fn context_updater<V: Clone + PartialEq + 'static>(
    cfg: &ContextConfig<V>,
    updater: impl Fn(V) -> V + 'static,
) -> ContextPairOrUpdater {
    let cfg2 = cfg.clone();
    pair_of(
        cfg,
        Rc::new(move |record| Rc::new(updater(get_context_value(record, &cfg2)))),
    )
}

/// `contextFromPairs(pairs, parent)`: a child of `parent` with the pairs
/// set, or `parent` itself when none changes a value.
pub(crate) fn context_from_pairs(
    pairs: &[ContextPairOrUpdater],
    parent: Option<Rc<ContextRecord>>,
) -> Option<Rc<ContextRecord>> {
    let mut rval = parent.clone();
    for pair in pairs {
        let value = (pair.resolve)(rval.as_ref());
        let is_parent = ptr_eq(&rval, &parent);
        if is_parent && (pair.same_as)(rval.as_ref(), value.as_ref()) {
            continue;
        }
        // Branch away from `parent` on the first change, so the caller's
        // record is never mutated; later pairs go into the same child.
        let ctx = match &rval {
            Some(r) if !is_parent => r.clone(),
            _ => ContextRecord::child_of(parent.clone()),
        };
        ctx.values.borrow_mut().insert(pair.id, value);
        rval = Some(ctx);
    }
    rval
}

fn ptr_eq(a: &Option<Rc<ContextRecord>>, b: &Option<Rc<ContextRecord>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// `EditorContext`: the editor and its current record of each kind.
#[derive(Clone)]
struct EditorContext {
    editor: Editor,
    records: HashMap<ContextKind, Rc<ContextRecord>>,
}

fn get_editor_context(editor: &Editor) -> Option<EditorContext> {
    ACTIVE_CONTEXT.with(|active| {
        active
            .borrow()
            .as_ref()
            .filter(|c| Rc::ptr_eq(&c.editor.0, &editor.0))
            .cloned()
    })
}

/// `getContextRecord(sym, editor)`: the current record of `kind`, if
/// `editor` has an active context.
pub(crate) fn get_context_record(kind: ContextKind, editor: &Editor) -> Option<Rc<ContextRecord>> {
    get_editor_context(editor).and_then(|c| c.records.get(&kind).cloned())
}

/// `$withFullContext(sym, contextRecord, f, editor)`: runs `f` with
/// `record` as the editor's current record of `kind`.
pub(crate) fn with_full_context<T>(
    kind: ContextKind,
    record: Rc<ContextRecord>,
    editor: &Editor,
    f: impl FnOnce() -> T,
) -> T {
    let mut records = get_editor_context(editor)
        .map(|c| c.records)
        .unwrap_or_default();
    records.insert(kind, record);
    let prev = ACTIVE_CONTEXT.with(|active| {
        active.replace(Some(EditorContext {
            editor: editor.clone(),
            records,
        }))
    });
    struct Restore(Option<EditorContext>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let prev = self.0.take();
            ACTIVE_CONTEXT.with(|active| *active.borrow_mut() = prev);
        }
    }
    let _restore = Restore(prev);
    f()
}

/// `$withContext(sym, $defaults)(cfg, editor)(f)`: runs `f` with the pairs
/// set over the current record of `kind` (or the editor's defaults).
pub(crate) fn with_context<T>(
    kind: ContextKind,
    defaults: impl FnOnce(&Editor) -> Option<Rc<ContextRecord>>,
    cfg: &[ContextPairOrUpdater],
    editor: &Editor,
    f: impl FnOnce() -> T,
) -> T {
    let parent = get_context_record(kind, editor);
    let base = parent.clone().or_else(|| defaults(editor));
    match context_from_pairs(cfg, base) {
        Some(record) if !ptr_eq(&Some(record.clone()), &parent) => {
            with_full_context(kind, record, editor, f)
        }
        _ => f(),
    }
}

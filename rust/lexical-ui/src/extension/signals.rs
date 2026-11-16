//! `signals.ts`: the reactive core `@lexical/extension` re-exports from
//! `@preact/signals-core` 1.14.4 (MIT, Copyright (c) 2022-present Preact
//! Team): `signal`, `computed`, `effect`, `batch` and `untracked`, ported
//! without a dependency. The algorithm is preact's: versioned dependency
//! nodes, lazily recomputed computeds, batched effects, and the batch
//! snapshots that keep a value set back to what it was inside a batch from
//! rerunning anything. Its linked lists are vectors kept in the same order,
//! so targets are notified and effects run in preact's order. The state is
//! per thread, as signals are neither `Send` nor `Sync`.
//!
//! Where it differs, from Rust: a value is compared with `==`
//! ([`SignalValue`]), where JS's `!==` compares objects by identity (a type
//! whose `==` is `Rc::ptr_eq` compares as JS does); effect and computed bodies can't throw, so
//! preact's error paths (`HAS_ERROR`, rethrowing after a batch) have no
//! counterpart, and a cycle panics with preact's message. `name`, `action`,
//! `createModel` and `Symbol.dispose` aren't ported.

use crate::{EditorState, Unregister};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

const RUNNING: u8 = 1 << 0;
const NOTIFIED: u8 = 1 << 1;
const OUTDATED: u8 = 1 << 2;
const DISPOSED: u8 = 1 << 3;
const TRACKING: u8 = 1 << 5;

type Value = Rc<dyn Any>;
type SourceCallback = Option<Rc<dyn Fn(&Rc<Source>)>>;
type EffectFn = Rc<dyn Fn() -> Option<Unregister>>;
/// `SignalOptions`' `watched` and `unwatched`.
pub type SignalCallback<T> = Option<Rc<dyn Fn(&Signal<T>)>>;

/// A dependency edge (preact's `Node`): `target` read `source` at
/// `version`.
struct Node {
    source: Rc<Source>,
    target: Target,
    version: Cell<i64>,
    rollback_node: RefCell<Option<Rc<Node>>>,
}

/// What reads signals: a computed (weakly, as it owns its sources) or an
/// effect.
#[derive(Clone)]
enum Target {
    Computed(Weak<Source>),
    Effect(Rc<EffectInner>),
}

impl Target {
    fn is(&self, other: &Target) -> bool {
        match (self, other) {
            (Target::Computed(a), Target::Computed(b)) => a.ptr_eq(b),
            (Target::Effect(a), Target::Effect(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    fn flags(&self) -> u8 {
        match self {
            Target::Computed(c) => c
                .upgrade()
                .map_or(0, |c| c.computed.as_ref().expect("computed").flags.get()),
            Target::Effect(e) => e.flags.get(),
        }
    }

    fn with_sources<R>(&self, f: impl FnOnce(&RefCell<Vec<Rc<Node>>>) -> R) -> Option<R> {
        match self {
            Target::Computed(c) => {
                let c = c.upgrade()?;
                Some(f(&c.computed.as_ref().expect("computed").sources))
            }
            Target::Effect(e) => Some(f(&e.sources)),
        }
    }

    fn notify(&self) {
        match self {
            Target::Computed(c) => {
                if let Some(c) = c.upgrade() {
                    computed_notify(&c);
                }
            }
            Target::Effect(e) => effect_notify(e),
        }
    }
}

/// A signal or a computed (preact's `Signal`, which `Computed` extends).
struct Source {
    value: RefCell<Option<Value>>,
    version: Cell<i64>,
    node: RefCell<Option<Rc<Node>>>,
    /// `_targets`, head first: the newest subscriber is notified first.
    targets: RefCell<Vec<Rc<Node>>>,
    batch_snapshot_version: Cell<u64>,
    watched: SourceCallback,
    unwatched: SourceCallback,
    /// JS's `===` on two values of this source.
    same: fn(&dyn Any, &dyn Any) -> bool,
    computed: Option<ComputedData>,
}

struct ComputedData {
    func: Box<dyn Fn() -> Value>,
    sources: RefCell<Vec<Rc<Node>>>,
    global_version: Cell<u64>,
    flags: Cell<u8>,
}

struct EffectInner {
    func: RefCell<Option<EffectFn>>,
    cleanup: RefCell<Option<Unregister>>,
    sources: RefCell<Vec<Rc<Node>>>,
    flags: Cell<u8>,
}

struct BatchSnapshot {
    source: Rc<Source>,
    value: Option<Value>,
    version: i64,
}

/// signals-core's module-scope variables.
#[derive(Default)]
struct State {
    eval_context: RefCell<Option<Target>>,
    /// `batchedEffect`, in notification order; preact runs the newest first.
    batched_effects: RefCell<Vec<Rc<EffectInner>>>,
    batch_depth: Cell<u32>,
    batch_iteration: Cell<u32>,
    batch_snapshot_version: Cell<u64>,
    current_batch_snapshot_version: Cell<u64>,
    /// `batchSnapshots`, head first.
    batch_snapshots: RefCell<Vec<BatchSnapshot>>,
    global_version: Cell<u64>,
}

thread_local! {
    static STATE: State = State::default();
}

fn with_state<R>(f: impl FnOnce(&State) -> R) -> R {
    STATE.with(f)
}

/// A value a signal can hold: JS's `===` on it is [`is_same`](Self::is_same),
/// which is `==` for any `PartialEq` value. An [`EditorState`] is the same
/// only as itself, as JS compares the objects.
pub trait SignalValue: Clone + 'static {
    fn is_same(&self, other: &Self) -> bool;
}

impl<T: Clone + PartialEq + 'static> SignalValue for T {
    fn is_same(&self, other: &Self) -> bool {
        self == other
    }
}

impl SignalValue for EditorState {
    fn is_same(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

fn same_value<T: SignalValue>(a: &dyn Any, b: &dyn Any) -> bool {
    match (a.downcast_ref::<T>(), b.downcast_ref::<T>()) {
        (Some(a), Some(b)) => a.is_same(b),
        _ => false,
    }
}

fn values_same(source: &Source, a: &Option<Value>, b: &Option<Value>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (source.same)(a.as_ref(), b.as_ref()),
        (None, None) => true,
        _ => false,
    }
}

fn start_batch() {
    with_state(|s| s.batch_depth.set(s.batch_depth.get() + 1));
}

fn end_batch() {
    if with_state(|s| {
        let depth = s.batch_depth.get();
        if depth > 1 {
            s.batch_depth.set(depth - 1);
        }
        depth > 1
    }) {
        return;
    }
    reconcile_batch_snapshots();
    loop {
        let effects = with_state(|s| std::mem::take(&mut *s.batched_effects.borrow_mut()));
        if effects.is_empty() {
            break;
        }
        with_state(|s| s.batch_iteration.set(s.batch_iteration.get() + 1));
        for effect in effects.iter().rev() {
            effect.flags.set(effect.flags.get() & !NOTIFIED);
            if effect.flags.get() & DISPOSED == 0
                && needs_to_recompute(&Target::Effect(effect.clone()))
            {
                effect_callback(effect);
            }
        }
    }
    with_state(|s| {
        s.batch_iteration.set(0);
        s.batch_depth.set(s.batch_depth.get() - 1);
    });
}

/// `batch(fn)`: runs `fn`, and the effects the signals it sets trigger once,
/// when it returns (or when the outermost batch does).
pub fn batch<R>(f: impl FnOnce() -> R) -> R {
    if with_state(|s| s.batch_depth.get() > 0) {
        return f();
    }
    with_state(|s| {
        let version = s.batch_snapshot_version.get() + 1;
        s.batch_snapshot_version.set(version);
        s.current_batch_snapshot_version.set(version);
    });
    start_batch();
    let result = f();
    end_batch();
    result
}

/// Restores the evaluation context when dropped.
struct ContextGuard(Option<Target>);

impl Drop for ContextGuard {
    fn drop(&mut self) {
        let prev = self.0.take();
        with_state(|s| *s.eval_context.borrow_mut() = prev);
    }
}

fn set_eval_context(target: Option<Target>) -> Option<Target> {
    with_state(|s| std::mem::replace(&mut *s.eval_context.borrow_mut(), target))
}

/// `untracked(fn)`: runs `fn` without subscribing the running computed or
/// effect to the signals it reads.
pub fn untracked<R>(f: impl FnOnce() -> R) -> R {
    let _guard = ContextGuard(set_eval_context(None));
    f()
}

fn record_batch_snapshot(source: &Rc<Source>) {
    with_state(|s| {
        if s.batch_depth.get() == 0 || s.batch_iteration.get() != 0 {
            return;
        }
        let current = s.current_batch_snapshot_version.get();
        if source.batch_snapshot_version.get() != current {
            source.batch_snapshot_version.set(current);
            s.batch_snapshots.borrow_mut().insert(
                0,
                BatchSnapshot {
                    source: source.clone(),
                    value: source.value.borrow().clone(),
                    version: source.version.get(),
                },
            );
        }
    });
}

fn reconcile_batch_snapshots() {
    let snapshots = with_state(|s| std::mem::take(&mut *s.batch_snapshots.borrow_mut()));
    for snapshot in snapshots {
        let source = &snapshot.source;
        if values_same(source, &source.value.borrow(), &snapshot.value) {
            for node in source.targets.borrow().iter() {
                if node.version.get() == snapshot.version {
                    node.version.set(source.version.get());
                }
            }
        }
    }
}

fn add_dependency(source: &Rc<Source>) -> Option<Rc<Node>> {
    let context = with_state(|s| s.eval_context.borrow().clone())?;
    let existing = source.node.borrow().clone();
    match existing {
        Some(node) if node.target.is(&context) => {
            if node.version.get() != -1 {
                return None;
            }
            node.version.set(0);
            // Moves the node to the end of the sources, so they stay in the
            // order they were first read.
            context.with_sources(|sources| {
                let mut sources = sources.borrow_mut();
                if let Some(i) = sources.iter().position(|n| Rc::ptr_eq(n, &node))
                    && i != sources.len() - 1
                {
                    let node = sources.remove(i);
                    sources.push(node);
                }
            });
            Some(node)
        }
        existing => {
            let node = Rc::new(Node {
                source: source.clone(),
                target: context.clone(),
                version: Cell::new(0),
                rollback_node: RefCell::new(existing),
            });
            context.with_sources(|sources| sources.borrow_mut().push(node.clone()));
            *source.node.borrow_mut() = Some(node.clone());
            if context.flags() & TRACKING != 0 {
                subscribe(source, &node);
            }
            Some(node)
        }
    }
}

fn subscribe(source: &Rc<Source>, node: &Rc<Node>) {
    if let Some(computed) = &source.computed
        && source.targets.borrow().is_empty()
    {
        computed
            .flags
            .set(computed.flags.get() | OUTDATED | TRACKING);
        let sources = computed.sources.borrow().clone();
        for n in &sources {
            subscribe(&n.source, n);
        }
    }
    let was_empty = {
        let mut targets = source.targets.borrow_mut();
        if targets.iter().any(|n| Rc::ptr_eq(n, node)) {
            return;
        }
        let was_empty = targets.is_empty();
        targets.insert(0, node.clone());
        was_empty
    };
    if was_empty && let Some(watched) = source.watched.clone() {
        untracked(|| watched(source));
    }
}

fn unsubscribe(source: &Rc<Source>, node: &Rc<Node>) {
    if source.targets.borrow().is_empty() {
        return;
    }
    let removed_last = {
        let mut targets = source.targets.borrow_mut();
        match targets.iter().position(|n| Rc::ptr_eq(n, node)) {
            Some(i) => {
                targets.remove(i);
                targets.is_empty()
            }
            None => false,
        }
    };
    if removed_last && let Some(unwatched) = source.unwatched.clone() {
        untracked(|| unwatched(source));
    }
    // `Computed._unsubscribe`: a computed nothing reads stops tracking its
    // sources.
    if let Some(computed) = &source.computed
        && source.targets.borrow().is_empty()
    {
        computed.flags.set(computed.flags.get() & !TRACKING);
        let sources = computed.sources.borrow().clone();
        for n in &sources {
            unsubscribe(&n.source, n);
        }
    }
}

fn refresh(source: &Rc<Source>) -> bool {
    match &source.computed {
        None => true,
        Some(_) => computed_refresh(source),
    }
}

fn needs_to_recompute(target: &Target) -> bool {
    let sources = target
        .with_sources(|s| s.borrow().clone())
        .unwrap_or_default();
    for node in &sources {
        if node.source.version.get() != node.version.get()
            || !refresh(&node.source)
            || node.source.version.get() != node.version.get()
        {
            return true;
        }
    }
    false
}

fn prepare_sources(target: &Target) {
    target.with_sources(|sources| {
        for node in sources.borrow().iter() {
            if let Some(rollback) = node.source.node.borrow().clone() {
                *node.rollback_node.borrow_mut() = Some(rollback);
            }
            *node.source.node.borrow_mut() = Some(node.clone());
            node.version.set(-1);
        }
    });
}

fn cleanup_sources(target: &Target) {
    let sources = target
        .with_sources(|s| s.borrow().clone())
        .unwrap_or_default();
    let mut kept = Vec::new();
    for node in sources.iter().rev() {
        if node.version.get() == -1 {
            unsubscribe(&node.source, node);
        } else {
            kept.push(node.clone());
        }
        let rollback = node.rollback_node.borrow_mut().take();
        *node.source.node.borrow_mut() = rollback;
    }
    kept.reverse();
    target.with_sources(|s| *s.borrow_mut() = kept);
}

fn computed_refresh(source: &Rc<Source>) -> bool {
    let computed = source.computed.as_ref().expect("computed");
    let flags = &computed.flags;
    flags.set(flags.get() & !NOTIFIED);
    if flags.get() & RUNNING != 0 {
        return false;
    }
    if flags.get() & (OUTDATED | TRACKING) == TRACKING {
        return true;
    }
    flags.set(flags.get() & !OUTDATED);
    let global_version = with_state(|s| s.global_version.get());
    if computed.global_version.get() == global_version {
        return true;
    }
    computed.global_version.set(global_version);
    flags.set(flags.get() | RUNNING);
    let target = Target::Computed(Rc::downgrade(source));
    if source.version.get() > 0 && !needs_to_recompute(&target) {
        flags.set(flags.get() & !RUNNING);
        return true;
    }
    prepare_sources(&target);
    let prev = set_eval_context(Some(target.clone()));
    let value = Some((computed.func)());
    if source.version.get() == 0 || !values_same(source, &source.value.borrow(), &value) {
        *source.value.borrow_mut() = value;
        source.version.set(source.version.get() + 1);
    }
    set_eval_context(prev);
    cleanup_sources(&target);
    flags.set(flags.get() & !RUNNING);
    true
}

fn computed_notify(source: &Rc<Source>) {
    let computed = source.computed.as_ref().expect("computed");
    if computed.flags.get() & NOTIFIED == 0 {
        computed
            .flags
            .set(computed.flags.get() | OUTDATED | NOTIFIED);
        let targets = source.targets.borrow().clone();
        for node in &targets {
            node.target.notify();
        }
    }
}

fn effect_notify(effect: &Rc<EffectInner>) {
    if effect.flags.get() & NOTIFIED == 0 {
        effect.flags.set(effect.flags.get() | NOTIFIED);
        with_state(|s| s.batched_effects.borrow_mut().push(effect.clone()));
    }
}

fn effect_callback(effect: &Rc<EffectInner>) {
    let prev = effect_start(effect);
    if effect.flags.get() & DISPOSED == 0 {
        let func = effect.func.borrow().clone();
        if let Some(func) = func
            && let Some(cleanup) = func()
        {
            *effect.cleanup.borrow_mut() = Some(cleanup);
        }
    }
    effect_end(effect, prev);
}

fn effect_start(effect: &Rc<EffectInner>) -> Option<Target> {
    if effect.flags.get() & RUNNING != 0 {
        panic!("Cycle detected");
    }
    effect.flags.set((effect.flags.get() | RUNNING) & !DISPOSED);
    cleanup_effect(effect);
    let target = Target::Effect(effect.clone());
    prepare_sources(&target);
    start_batch();
    set_eval_context(Some(target))
}

fn effect_end(effect: &Rc<EffectInner>, prev: Option<Target>) {
    let target = Target::Effect(effect.clone());
    let current = with_state(|s| s.eval_context.borrow().clone());
    assert!(
        current.is_some_and(|c| c.is(&target)),
        "Out-of-order effect"
    );
    cleanup_sources(&target);
    set_eval_context(prev);
    effect.flags.set(effect.flags.get() & !RUNNING);
    if effect.flags.get() & DISPOSED != 0 {
        dispose_effect(effect);
    }
    end_batch();
}

fn cleanup_effect(effect: &Rc<EffectInner>) {
    let cleanup = effect.cleanup.borrow_mut().take();
    if let Some(cleanup) = cleanup {
        start_batch();
        let prev = set_eval_context(None);
        cleanup();
        set_eval_context(prev);
        end_batch();
    }
}

fn dispose_effect(effect: &Rc<EffectInner>) {
    let sources = std::mem::take(&mut *effect.sources.borrow_mut());
    for node in &sources {
        unsubscribe(&node.source, node);
    }
    *effect.func.borrow_mut() = None;
    cleanup_effect(effect);
}

fn effect_dispose(effect: &Rc<EffectInner>) {
    effect.flags.set(effect.flags.get() | DISPOSED);
    if effect.flags.get() & RUNNING == 0 {
        dispose_effect(effect);
    }
}

/// `SignalOptions`: callbacks for when the signal gains its first
/// subscriber (`watched`) and loses its last (`unwatched`).
pub struct SignalOptions<T> {
    pub watched: SignalCallback<T>,
    pub unwatched: SignalCallback<T>,
}

impl<T> Default for SignalOptions<T> {
    fn default() -> Self {
        Self {
            watched: None,
            unwatched: None,
        }
    }
}

/// `Signal`: a value that the computeds and effects reading it track.
/// Cloning it shares the signal.
pub struct Signal<T>(Rc<Source>, PhantomData<T>);

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Signal(self.0.clone(), PhantomData)
    }
}

/// `ReadonlySignal`: a signal that can only be read, such as a
/// [`computed`].
pub struct ReadonlySignal<T>(Rc<Source>, PhantomData<T>);

impl<T> Clone for ReadonlySignal<T> {
    fn clone(&self) -> Self {
        ReadonlySignal(self.0.clone(), PhantomData)
    }
}

fn read_value<T: Clone + 'static>(source: &Rc<Source>) -> T {
    source
        .value
        .borrow()
        .as_ref()
        .and_then(|v| v.downcast_ref::<T>())
        .cloned()
        .expect("a signal holds a value of its type")
}

fn source_value<T: Clone + 'static>(source: &Rc<Source>) -> T {
    if let Some(computed) = &source.computed {
        if computed.flags.get() & RUNNING != 0 {
            panic!("Cycle detected");
        }
        let node = add_dependency(source);
        computed_refresh(source);
        if let Some(node) = node {
            node.version.set(source.version.get());
        }
    } else if let Some(node) = add_dependency(source) {
        node.version.set(source.version.get());
    }
    read_value(source)
}

/// `signal(value)`.
pub fn signal<T: SignalValue>(value: T) -> Signal<T> {
    signal_with_options(value, SignalOptions::default())
}

/// `signal(value, options)`.
pub fn signal_with_options<T: SignalValue>(value: T, options: SignalOptions<T>) -> Signal<T> {
    let typed = |f: SignalCallback<T>| -> SourceCallback {
        f.map(|f| Rc::new(move |s: &Rc<Source>| f(&Signal(s.clone(), PhantomData))) as _)
    };
    Signal(
        Rc::new(Source {
            value: RefCell::new(Some(Rc::new(value))),
            version: Cell::new(0),
            node: RefCell::new(None),
            targets: RefCell::new(Vec::new()),
            batch_snapshot_version: Cell::new(0),
            watched: typed(options.watched),
            unwatched: typed(options.unwatched),
            same: same_value::<T>,
            computed: None,
        }),
        PhantomData,
    )
}

impl<T: SignalValue> Signal<T> {
    /// `signal.value`: the value, subscribing the running computed or effect
    /// to the signal.
    pub fn value(&self) -> T {
        source_value(&self.0)
    }

    /// `signal.value = value`: sets the value and, unless it is the same
    /// (`==`), runs the effects that depend on it, at the end of the batch
    /// if one is running.
    pub fn set_value(&self, value: T) {
        let source = &self.0;
        if source
            .value
            .borrow()
            .as_ref()
            .and_then(|v| v.downcast_ref::<T>())
            .is_some_and(|v| v.is_same(&value))
        {
            return;
        }
        if with_state(|s| s.batch_iteration.get()) > 100 {
            panic!("Cycle detected");
        }
        record_batch_snapshot(source);
        *source.value.borrow_mut() = Some(Rc::new(value));
        source.version.set(source.version.get() + 1);
        with_state(|s| s.global_version.set(s.global_version.get() + 1));
        start_batch();
        let targets = source.targets.borrow().clone();
        for node in &targets {
            node.target.notify();
        }
        end_batch();
    }

    /// `signal.peek()`: the value, without subscribing.
    pub fn peek(&self) -> T {
        untracked(|| self.value())
    }

    /// `signal.subscribe(fn)`: calls `fn` with the value now and after each
    /// change; returns the function that stops it.
    pub fn subscribe(&self, f: impl Fn(T) + 'static) -> Unregister {
        let this = self.clone();
        effect(move || {
            let value = this.value();
            untracked(|| f(value));
            None
        })
    }

    /// This signal, read-only.
    pub fn as_readonly(&self) -> ReadonlySignal<T> {
        ReadonlySignal(self.0.clone(), PhantomData)
    }
}

impl<T: Clone + 'static> ReadonlySignal<T> {
    /// `signal.value`.
    pub fn value(&self) -> T {
        source_value(&self.0)
    }

    /// `signal.peek()`.
    pub fn peek(&self) -> T {
        untracked(|| self.value())
    }
}

/// `computed(fn)`: a signal whose value `fn` computes from the signals it
/// reads, lazily, again only after one of them changed.
pub fn computed<T: SignalValue>(f: impl Fn() -> T + 'static) -> ReadonlySignal<T> {
    let global_version = with_state(|s| s.global_version.get());
    ReadonlySignal(
        Rc::new(Source {
            value: RefCell::new(None),
            version: Cell::new(0),
            node: RefCell::new(None),
            targets: RefCell::new(Vec::new()),
            batch_snapshot_version: Cell::new(0),
            watched: None,
            unwatched: None,
            same: same_value::<T>,
            computed: Some(ComputedData {
                func: Box::new(move || Rc::new(f()) as Value),
                sources: RefCell::new(Vec::new()),
                global_version: Cell::new(global_version.wrapping_sub(1)),
                flags: Cell::new(OUTDATED),
            }),
        }),
        PhantomData,
    )
}

/// `effect(fn)`: runs `fn` now and again after a signal it read changes.
/// The clean-up `fn` returns runs before its next run and when it is
/// disposed. Returns the function that disposes it.
pub fn effect(f: impl Fn() -> Option<Unregister> + 'static) -> Unregister {
    let effect = Rc::new(EffectInner {
        func: RefCell::new(Some(Rc::new(f))),
        cleanup: RefCell::new(None),
        sources: RefCell::new(Vec::new()),
        flags: Cell::new(TRACKING),
    });
    effect_callback(&effect);
    Box::new(move || effect_dispose(&effect))
}

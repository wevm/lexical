// A port of @preact/signals-core 1.14.4 (MIT, Copyright (c) 2022-present
// Preact Team), which @lexical/extension re-exports. The algorithm is the
// same: versioned dependency nodes, lazy computeds, batched effects. The
// linked lists become arrays kept in the same order, so effects run and
// targets are notified in the order preact uses.
//
// Two differences, both from Swift: values compare with `==` when they are
// `Equatable` and by identity when they are objects (JS `!==`), and
// property accessors can't throw, so an effect that throws while a
// `value` setter flushes is reported as an uncaught error (`try set(_:)`
// rethrows it instead), and a cycle is a fatal error.

import Lexical

private let RUNNING = 1 << 0
private let NOTIFIED = 1 << 1
private let OUTDATED = 1 << 2
private let DISPOSED = 1 << 3
private let HAS_ERROR = 1 << 4
private let TRACKING = 1 << 5

/// JS `!==` for signal values.
func signalValuesDiffer(_ a: Any?, _ b: Any?) -> Bool {
  switch (a, b) {
  case (nil, nil): return false
  case (nil, _), (_, nil): return true
  case (let a?, let b?):
    if type(of: a) is AnyClass, type(of: b) is AnyClass {
      return (a as AnyObject) !== (b as AnyObject)
    }
    if let a = a as? any Equatable {
      return !equals(a, b)
    }
    return true
  }
}

private func equals<E: Equatable>(_ a: E, _ b: Any) -> Bool {
  guard let b = b as? E else { return false }
  return a == b
}

final class SignalNode {
  let source: SignalBase
  let target: any SignalTarget
  var version: Int
  var rollbackNode: SignalNode?

  init(source: SignalBase, target: any SignalTarget, version: Int, rollbackNode: SignalNode?) {
    self.source = source
    self.target = target
    self.version = version
    self.rollbackNode = rollbackNode
  }
}

protocol SignalTarget: AnyObject {
  var _sources: [SignalNode] { get set }
  var _flags: Int { get set }
  func _notify()
}

private struct BatchSnapshot {
  let source: SignalBase
  let value: Any?
  let version: Int
}

/// The module-scope variables of signals-core, per thread.
final class SignalsState {
  var evalContext: (any SignalTarget)?
  var capturedEffects: [Effect]?
  /// `batchedEffect`, in notification order (preact runs it newest first).
  var batchedEffects: [Effect] = []
  var batchDepth = 0
  var batchIteration = 0
  var batchSnapshotVersion = 0
  var currentBatchSnapshotVersion = 0
  fileprivate var batchSnapshots: [BatchSnapshot] = []
  var globalVersion = 0

  static var current: SignalsState {
    let context = ThreadContext.current
    if let state = context.signalsState as? SignalsState { return state }
    let state = SignalsState()
    context.signalsState = state
    return state
  }
}

private func startBatch() {
  SignalsState.current.batchDepth += 1
}

private func endBatch() throws {
  let s = SignalsState.current
  if s.batchDepth > 1 {
    s.batchDepth -= 1
    return
  }
  var error: (any Error)?
  reconcileBatchSnapshots()
  while !s.batchedEffects.isEmpty {
    let effects = s.batchedEffects
    s.batchedEffects = []
    s.batchIteration += 1
    for effect in effects.reversed() {
      effect._flags &= ~NOTIFIED
      if effect._flags & DISPOSED == 0, needsToRecompute(effect) {
        do {
          try effect._callback()
        } catch let err {
          if error == nil { error = err }
        }
      }
    }
  }
  s.batchIteration = 0
  s.batchDepth -= 1
  if let error { throw error }
}

/// Ends a batch where nothing can rethrow: reports an effect's error.
private func endBatchReporting() {
  do { try endBatch() } catch { reportUncaughtError(error) }
}

/// `batch`: runs `fn`, running the effects it triggers once at the end.
public func batch<T>(_ fn: () throws -> T) throws -> T {
  let s = SignalsState.current
  if s.batchDepth > 0 {
    return try fn()
  }
  s.batchSnapshotVersion += 1
  s.currentBatchSnapshotVersion = s.batchSnapshotVersion
  startBatch()
  let result: T
  do {
    result = try fn()
  } catch {
    try endBatch()
    throw error
  }
  try endBatch()
  return result
}

/// `untracked`: runs `fn` without subscribing to the signals it reads.
public func untracked<T>(_ fn: () throws -> T) rethrows -> T {
  let s = SignalsState.current
  let prevContext = s.evalContext
  let prevCapturedEffects = s.capturedEffects
  s.evalContext = nil
  s.capturedEffects = nil
  defer {
    s.evalContext = prevContext
    s.capturedEffects = prevCapturedEffects
  }
  return try fn()
}

private func recordBatchSnapshot(_ source: SignalBase) {
  let s = SignalsState.current
  if s.batchDepth == 0 || s.batchIteration != 0 { return }
  if source._batchSnapshotVersion != s.currentBatchSnapshotVersion {
    source._batchSnapshotVersion = s.currentBatchSnapshotVersion
    s.batchSnapshots.insert(
      BatchSnapshot(source: source, value: source._value, version: source._version), at: 0)
  }
}

private func reconcileBatchSnapshots() {
  let s = SignalsState.current
  let snapshots = s.batchSnapshots
  s.batchSnapshots = []
  for snapshot in snapshots {
    let source = snapshot.source
    if !signalValuesDiffer(source._value, snapshot.value) {
      for node in source._targets where node.version == snapshot.version {
        node.version = source._version
      }
    }
  }
}

private func addDependency(_ signal: SignalBase) -> SignalNode? {
  let s = SignalsState.current
  guard let evalContext = s.evalContext else { return nil }
  let existing = signal._node
  if existing == nil || existing!.target !== evalContext {
    let node = SignalNode(source: signal, target: evalContext, version: 0, rollbackNode: existing)
    evalContext._sources.append(node)
    signal._node = node
    if evalContext._flags & TRACKING != 0 {
      signal._subscribe(node)
    }
    return node
  } else if let node = existing, node.version == -1 {
    node.version = 0
    // Move the node to the end of the sources, so they stay in use order.
    if let index = evalContext._sources.firstIndex(where: { $0 === node }),
      index != evalContext._sources.count - 1
    {
      evalContext._sources.remove(at: index)
      evalContext._sources.append(node)
    }
    return node
  }
  return nil
}

/// The part of a signal the dependency graph works on.
public class SignalBase {
  var _value: Any?
  var _version = 0
  var _node: SignalNode?
  /// `_targets`, head first: the newest subscriber is notified first.
  var _targets: [SignalNode] = []
  var _batchSnapshotVersion = 0
  var _watched: (() -> Void)?
  var _unwatched: (() -> Void)?
  public var name: String?

  init(value: Any?, name: String?) {
    _value = value
    self.name = name
  }

  func _refresh() -> Bool { true }

  func _subscribe(_ node: SignalNode) {
    if !_targets.contains(where: { $0 === node }) {
      let wasEmpty = _targets.isEmpty
      _targets.insert(node, at: 0)
      if wasEmpty, let watched = _watched {
        untracked { watched() }
      }
    }
  }

  func _unsubscribe(_ node: SignalNode) {
    guard !_targets.isEmpty, let index = _targets.firstIndex(where: { $0 === node }) else {
      return
    }
    _targets.remove(at: index)
    if _targets.isEmpty, let unwatched = _unwatched {
      untracked { unwatched() }
    }
  }
}

/// `ReadonlySignal`: a value that tracks its readers.
public class ReadonlySignal<T>: SignalBase {
  /// The value, subscribing the running computed or effect to it.
  public var value: T {
    let node = addDependency(self)
    if let node { node.version = _version }
    return _value as! T
  }

  /// The value, without subscribing.
  public func peek() -> T { untracked { value } }

  /// Calls `fn` with the value now and after each change; returns the
  /// unsubscribe function.
  public func subscribe(_ fn: @escaping (T) throws -> Void) throws -> () -> Void {
    try effect(name: "sub") { [unowned self] in
      let value = self.value
      try untracked { try fn(value) }
      return nil
    }
  }
}

/// `SignalOptions`.
public struct SignalOptions<T> {
  public var watched: ((Signal<T>) -> Void)?
  public var unwatched: ((Signal<T>) -> Void)?
  public var name: String?

  public init(
    watched: ((Signal<T>) -> Void)? = nil, unwatched: ((Signal<T>) -> Void)? = nil,
    name: String? = nil
  ) {
    self.watched = watched
    self.unwatched = unwatched
    self.name = name
  }
}

/// `Signal`: a writable value.
public final class Signal<T>: ReadonlySignal<T> {
  public init(_ value: T, options: SignalOptions<T> = SignalOptions()) {
    super.init(value: value, name: options.name)
    if let watched = options.watched {
      _watched = { [unowned self] in watched(self) }
    }
    if let unwatched = options.unwatched {
      _unwatched = { [unowned self] in unwatched(self) }
    }
  }

  public override var value: T {
    get { super.value }
    set { do { try set(newValue) } catch { reportUncaughtError(error) } }
  }

  /// Sets the value, running the effects that depend on it, and rethrows
  /// the first error one of them throws.
  public func set(_ value: T) throws {
    guard signalValuesDiffer(value, _value) else { return }
    let s = SignalsState.current
    if s.batchIteration > 100 {
      throw LexicalError("Cycle detected")
    }
    recordBatchSnapshot(self)
    _value = value
    _version += 1
    s.globalVersion += 1
    startBatch()
    for node in _targets {
      node.target._notify()
    }
    try endBatch()
  }
}

/// `signal(value, options)`.
public func signal<T>(_ value: T, _ options: SignalOptions<T> = SignalOptions()) -> Signal<T> {
  Signal(value, options: options)
}

private func needsToRecompute(_ target: any SignalTarget) -> Bool {
  for node in target._sources {
    if node.source._version != node.version || !node.source._refresh()
      || node.source._version != node.version
    {
      return true
    }
  }
  return false
}

private func prepareSources(_ target: any SignalTarget) {
  for node in target._sources {
    if let rollbackNode = node.source._node {
      node.rollbackNode = rollbackNode
    }
    node.source._node = node
    node.version = -1
  }
}

private func cleanupSources(_ target: any SignalTarget) {
  var kept: [SignalNode] = []
  for node in target._sources.reversed() {
    if node.version == -1 {
      node.source._unsubscribe(node)
    } else {
      kept.append(node)
    }
    node.source._node = node.rollbackNode
    node.rollbackNode = nil
  }
  target._sources = kept.reversed()
}

/// `Computed`: a lazily computed, cached value.
public final class Computed<T>: ReadonlySignal<T>, SignalTarget {
  let _fn: () -> T
  var _sources: [SignalNode] = []
  var _globalVersion: Int
  var _flags = OUTDATED

  init(_ fn: @escaping () -> T, name: String?) {
    _fn = fn
    _globalVersion = SignalsState.current.globalVersion - 1
    super.init(value: nil, name: name)
  }

  override func _refresh() -> Bool {
    _flags &= ~NOTIFIED
    if _flags & RUNNING != 0 { return false }
    if _flags & (OUTDATED | TRACKING) == TRACKING { return true }
    _flags &= ~OUTDATED
    let s = SignalsState.current
    if _globalVersion == s.globalVersion { return true }
    _globalVersion = s.globalVersion
    _flags |= RUNNING
    if _version > 0 && !needsToRecompute(self) {
      _flags &= ~RUNNING
      return true
    }
    let prevContext = s.evalContext
    prepareSources(self)
    s.evalContext = self
    let value = _fn()
    if _flags & HAS_ERROR != 0 || signalValuesDiffer(_value, value) || _version == 0 {
      _value = value
      _flags &= ~HAS_ERROR
      _version += 1
    }
    s.evalContext = prevContext
    cleanupSources(self)
    _flags &= ~RUNNING
    return true
  }

  override func _subscribe(_ node: SignalNode) {
    if _targets.isEmpty {
      _flags |= OUTDATED | TRACKING
      for source in _sources {
        source.source._subscribe(source)
      }
    }
    super._subscribe(node)
  }

  override func _unsubscribe(_ node: SignalNode) {
    if !_targets.isEmpty {
      super._unsubscribe(node)
      if _targets.isEmpty {
        _flags &= ~TRACKING
        for source in _sources {
          source.source._unsubscribe(source)
        }
      }
    }
  }

  func _notify() {
    if _flags & NOTIFIED == 0 {
      _flags |= OUTDATED | NOTIFIED
      for node in _targets {
        node.target._notify()
      }
    }
  }

  public override var value: T {
    if _flags & RUNNING != 0 {
      fatalError("Cycle detected")
    }
    let node = addDependency(self)
    _ = _refresh()
    if let node { node.version = _version }
    return _value as! T
  }
}

/// `computed(fn)`.
public func computed<T>(name: String? = nil, _ fn: @escaping () -> T) -> ReadonlySignal<T> {
  Computed(fn, name: name)
}

/// An effect's body: returns a clean-up to run before its next run and on
/// disposal, or `nil`.
public typealias EffectFn = () throws -> (() -> Void)?

final class Effect: SignalTarget {
  var _fn: EffectFn?
  var _cleanup: (() -> Void)?
  var _sources: [SignalNode] = []
  var _flags = TRACKING
  var name: String?

  init(_ fn: @escaping EffectFn, name: String?) {
    _fn = fn
    self.name = name
    let s = SignalsState.current
    if s.capturedEffects != nil {
      s.capturedEffects!.append(self)
    }
  }

  func _callback() throws {
    let prevContext = try _start()
    var bodyError: (any Error)?
    do {
      if _flags & DISPOSED == 0, let fn = _fn, let cleanup = try fn() {
        _cleanup = cleanup
      }
    } catch {
      bodyError = error
    }
    // JS ends the effect in `finally`, so its error replaces the body's.
    try endEffect(prevContext)
    if let bodyError { throw bodyError }
  }

  func _start() throws -> (any SignalTarget)? {
    if _flags & RUNNING != 0 {
      throw LexicalError("Cycle detected")
    }
    _flags |= RUNNING
    _flags &= ~DISPOSED
    try cleanupEffect(self)
    prepareSources(self)
    startBatch()
    let s = SignalsState.current
    let prevContext = s.evalContext
    s.evalContext = self
    return prevContext
  }

  private func endEffect(_ prevContext: (any SignalTarget)?) throws {
    let s = SignalsState.current
    precondition(s.evalContext === self, "Out-of-order effect")
    cleanupSources(self)
    s.evalContext = prevContext
    _flags &= ~RUNNING
    if _flags & DISPOSED != 0 {
      disposeEffect(self)
    }
    try endBatch()
  }

  func _notify() {
    if _flags & NOTIFIED == 0 {
      _flags |= NOTIFIED
      SignalsState.current.batchedEffects.append(self)
    }
  }

  func _dispose() {
    _flags |= DISPOSED
    if _flags & RUNNING == 0 {
      disposeEffect(self)
    }
  }
}

private func cleanupEffect(_ effect: Effect) throws {
  guard let cleanup = effect._cleanup else { return }
  effect._cleanup = nil
  startBatch()
  let s = SignalsState.current
  let prevContext = s.evalContext
  s.evalContext = nil
  cleanup()
  s.evalContext = prevContext
  try endBatch()
}

private func disposeEffect(_ effect: Effect) {
  for node in effect._sources {
    node.source._unsubscribe(node)
  }
  effect._fn = nil
  effect._sources = []
  do { try cleanupEffect(effect) } catch { reportUncaughtError(error) }
}

/// `effect(fn)`: runs `fn` now and again whenever a signal it read
/// changes. Returns the function that disposes it. A body that returns a
/// clean-up spells its type (`{ () -> (() -> Void)? in ... }`), as the
/// listener clean-ups do; a plain body resolves to the `Void` form.
@_disfavoredOverload
@discardableResult
public func effect(name: String? = nil, _ fn: @escaping EffectFn) throws -> () -> Void {
  let effect = Effect(fn, name: name)
  do {
    try effect._callback()
  } catch {
    effect._dispose()
    throw error
  }
  return { effect._dispose() }
}

/// `effect(fn)` for a body that returns nothing.
@discardableResult
public func effect(name: String? = nil, _ fn: @escaping () throws -> Void) throws -> () -> Void {
  try effect(
    name: name,
    {
      try fn()
      return nil
    } as EffectFn)
}

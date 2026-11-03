import Lexical

// Port of packages/lexical-html/src/internal/ContextRecord.ts: the layered
// context records the render and import pipelines read typed values from.
// JS chains records through prototypes; a Swift record holds its own values
// and a parent.

/// The pipeline a context record belongs to (JS `DOMRenderContextSymbol` or
/// `DOMImportContextSymbol`).
public enum ContextKind: Sendable {
  case render, `import`
}

/// The untyped half of a `ContextConfig`, so a record can store any value.
public class AnyContextConfig {
  public let name: String
  public let kind: ContextKind
  let isEqualAny: (Any, Any) -> Bool
  let defaultAny: Any

  init(name: String, kind: ContextKind, isEqualAny: @escaping (Any, Any) -> Bool, defaultAny: Any) {
    self.name = name
    self.kind = kind
    self.isEqualAny = isEqualAny
    self.defaultAny = defaultAny
  }
}

/// `ContextConfig<Sym, V>`: a typed key of a context record, with its
/// default value. Configs compare by identity, as JS keys them by a fresh
/// `Symbol(name)`.
public final class ContextConfig<V>: AnyContextConfig, @unchecked Sendable {
  public let defaultValue: V
  public let isEqual: (V, V) -> Bool

  init(name: String, kind: ContextKind, defaultValue: V, isEqual: ((V, V) -> Bool)?) {
    let isEqual = isEqual ?? { jsObjectIs($0, $1) }
    self.defaultValue = defaultValue
    self.isEqual = isEqual
    super.init(
      name: name, kind: kind, isEqualAny: { isEqual($0 as! V, $1 as! V) },
      defaultAny: defaultValue)
  }
}

/// `createContextState(tag, name, getDefaultValue, isEqual)`.
func createContextState<V>(
  _ kind: ContextKind, _ name: String, _ getDefaultValue: () -> V,
  _ isEqual: ((V, V) -> Bool)? = nil
) -> ContextConfig<V> {
  ContextConfig(name: name, kind: kind, defaultValue: getDefaultValue(), isEqual: isEqual)
}

/// `ContextRecord`: one layer of context values over its parent's.
public final class ContextRecord {
  let parent: ContextRecord?
  private var values: [ObjectIdentifier: Any] = [:]

  init(parent: ContextRecord?) {
    self.parent = parent
  }

  /// `key in record`: whether this layer or one below sets `config`.
  func lookup(_ config: AnyContextConfig) -> Any? {
    var record: ContextRecord? = self
    while let current = record {
      if let value = current.values[ObjectIdentifier(config)] {
        return value
      }
      record = current.parent
    }
    return nil
  }

  func set(_ config: AnyContextConfig, _ value: Any) {
    values[ObjectIdentifier(config)] = value
  }

  func deleteOwn(_ config: AnyContextConfig) {
    values[ObjectIdentifier(config)] = nil
  }
}

/// `getContextValue(record, cfg)`: the value, or the config's default.
func getContextValue<V>(_ record: ContextRecord?, _ cfg: ContextConfig<V>) -> V {
  if let value = record?.lookup(cfg) { return value as! V }
  return cfg.defaultValue
}

/// `getOwnContextValue(record, cfg)`: the value without a default.
func getOwnContextValue<V>(_ record: ContextRecord, _ cfg: ContextConfig<V>) -> V? {
  record.lookup(cfg).map { $0 as! V }
}

/// `popOwnContextValue(record, cfg)`: reads `cfg` and deletes it from this
/// layer.
func popOwnContextValue<V>(_ record: ContextRecord, _ cfg: ContextConfig<V>) -> V? {
  let value = getOwnContextValue(record, cfg)
  record.deleteOwn(cfg)
  return value
}

/// `ContextConfigPair` or `ContextConfigUpdater`: a value for a config, or
/// a function of its current value. Build with `contextValue` and
/// `contextUpdater`.
public struct ContextPairOrUpdater {
  let config: AnyContextConfig
  let resolve: (ContextRecord?) -> Any

  /// The config the pair or updater sets.
  public var cfg: AnyContextConfig { config }
}

/// `contextValue(cfg, value)` (`@experimental`).
public func contextValue<V>(_ cfg: ContextConfig<V>, _ value: V) -> ContextPairOrUpdater {
  ContextPairOrUpdater(config: cfg) { _ in value }
}

/// `contextUpdater(cfg, updater)` (`@experimental`).
public func contextUpdater<V>(
  _ cfg: ContextConfig<V>, _ updater: @escaping (V) -> V
)
  -> ContextPairOrUpdater
{
  ContextPairOrUpdater(config: cfg) { record in updater(getContextValue(record, cfg)) }
}

/// `contextFromPairs(pairs, parent)`: a child of `parent` with the pairs
/// set, or `parent` itself when none changes a value.
func contextFromPairs(
  _ pairs: [ContextPairOrUpdater], _ parent: ContextRecord?
)
  -> ContextRecord?
{
  var rval = parent
  for pair in pairs {
    let value = pair.resolve(rval)
    if rval === parent, jsObjectIs(rval?.lookup(pair.config) ?? pair.config.defaultAny, value) {
      continue
    }
    // Branch away from `parent` on the first change, so the caller's record
    // is never mutated; later pairs accumulate into the same child.
    let record = rval === parent || rval == nil ? ContextRecord(parent: parent) : rval!
    record.set(pair.config, value)
    rval = record
  }
  return rval
}

/// The active editor's context records (JS `activeContext`), one per kind,
/// kept per thread in `ThreadContext.htmlContext`.
private final class EditorContext {
  let editor: Editor
  var records: [ContextKind: ContextRecord]
  init(editor: Editor, records: [ContextKind: ContextRecord]) {
    self.editor = editor
    self.records = records
  }
}

private func editorContext(_ editor: Editor) -> EditorContext? {
  guard let context = ThreadContext.current.htmlContext as? EditorContext,
    context.editor === editor
  else { return nil }
  return context
}

/// `getContextRecord(sym, editor)`: the current record of `kind`, if
/// `editor` has an active context.
func getContextRecord(_ kind: ContextKind, _ editor: Editor) -> ContextRecord? {
  editorContext(editor)?.records[kind]
}

/// `$withFullContext(sym, record, f, editor)`.
func withFullContext<T>(
  _ kind: ContextKind, _ record: ContextRecord, _ editor: Editor, _ f: () throws -> T
) rethrows -> T {
  let thread = ThreadContext.current
  let prev = thread.htmlContext
  var records = editorContext(editor)?.records ?? [:]
  records[kind] = record
  thread.htmlContext = EditorContext(editor: editor, records: records)
  defer { thread.htmlContext = prev }
  return try f()
}

/// `$withContext(sym, $defaults)(cfg, editor)(f)`: runs `f` with the pairs
/// set over the current record of `kind` (or the editor's defaults).
func withContext<T>(
  _ kind: ContextKind, _ defaults: (Editor) throws -> ContextRecord?,
  _ cfg: [ContextPairOrUpdater], _ editor: Editor, _ f: () throws -> T
) throws -> T {
  let parentRecord = getContextRecord(kind, editor)
  guard let record = try contextFromPairs(cfg, parentRecord ?? defaults(editor)),
    record !== parentRecord
  else { return try f() }
  return try withFullContext(kind, record, editor, f)
}

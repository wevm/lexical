import Lexical
import LexicalExtension

// Port of packages/lexical-html/src/import: rules (defineImportRule.ts),
// their dispatch tables (compileImportRules.ts, defineOverlayRules.ts),
// the import context (ImportContext.ts), the walk (runImport.ts) and the
// child schemas (schemas.ts). All `@experimental` upstream.

// MARK: - Rules

/// `DOMImportFn`'s `$next`: the next matching rule's result.
public typealias ImportNext = () throws -> [LexicalNode]

/// `DOMImportRule`: converts the DOM nodes its `match` accepts into
/// Lexical nodes. `$import` may call `$next()` to delegate to the next
/// matching rule, and returns `[]` to drop the node.
public struct DOMImportRule {
  /// Shown in warnings and errors; `'@scope/package/rule-id'` by
  /// convention.
  public let name: String?
  let kind: SelectorKind
  let tags: Set<String>
  let predicate: SelectorPredicate
  let `import`: (DOMImportContext, DOMNode, @escaping ImportNext) throws -> [LexicalNode]
}

/// `defineImportRule({name, match, $import})`: a rule whose `$import`
/// receives the node type its selector matches.
public func defineImportRule<N: DOMNode>(
  name: String? = nil, match: CompiledSelector<N>,
  import: @escaping (DOMImportContext, N, @escaping ImportNext) throws -> [LexicalNode]
) -> DOMImportRule {
  DOMImportRule(
    name: name, kind: match.kind, tags: match.tags, predicate: match.predicate,
    import: { ctx, node, next in try `import`(ctx, node as! N, next) })
}

/// `DOMImportRuleEntry`: a rule, or a compiled overlay whose rules are
/// inlined at its position.
public enum DOMImportRuleEntry {
  case rule(DOMImportRule)
  case overlay(CompiledOverlayRules)
}

extension DOMImportRule {
  /// This rule as a `DOMImportRuleEntry`.
  public var entry: DOMImportRuleEntry { .rule(self) }
}

/// `CompiledDispatch`: the rules by registration order, and the indices to
/// try for each tag (with wildcard element rules interleaved), for
/// wildcards, text and comments.
struct CompiledDispatch {
  let rules: [(name: String, rule: DOMImportRule)]
  let byTag: [String: [Int]]
  let wildcardIndices: [Int]
  let textIndices: [Int]
  let commentIndices: [Int]
}

private func mergeSortedAsc(_ a: [Int], _ b: [Int]) -> [Int] {
  var out: [Int] = []
  var i = 0
  var j = 0
  while i < a.count && j < b.count {
    if a[i] <= b[j] {
      out.append(a[i])
      i += 1
    } else {
      out.append(b[j])
      j += 1
    }
  }
  return out + a[i...] + b[j...]
}

private func defaultRuleName(_ rule: DOMImportRule, _ index: Int) -> String {
  switch rule.kind {
  case .text: return "#text@\(index)"
  case .comment: return "#comment@\(index)"
  case .element:
    if rule.tags.isEmpty { return "*@\(index)" }
    return "\(rule.tags.sorted().joined(separator: ",").lowercased())@\(index)"
  }
}

/// `compileImportRules(rules)`: the dispatch tables. Index 0 is the
/// highest priority.
func compileImportRules(_ rules: [DOMImportRule]) -> CompiledDispatch {
  var compiled: [(name: String, rule: DOMImportRule)] = []
  var byTag: [String: [Int]] = [:]
  var wildcardIndices: [Int] = []
  var textIndices: [Int] = []
  var commentIndices: [Int] = []
  var seenNames: Set<String> = []
  for (i, rule) in rules.enumerated() {
    if let name = rule.name, !name.isEmpty {
      if seenNames.contains(name) {
        consoleWarn(
          "[lexical] duplicate DOMImportRule name \"\(name)\" — keep names unique to aid debugging."
        )
      }
      seenNames.insert(name)
    }
    let name = rule.name.flatMap { $0.isEmpty ? nil : $0 } ?? defaultRuleName(rule, i)
    compiled.append((name, rule))
    switch rule.kind {
    case .text: textIndices.append(i)
    case .comment: commentIndices.append(i)
    case .element:
      if rule.tags.isEmpty {
        wildcardIndices.append(i)
      } else {
        for tag in rule.tags { byTag[tag, default: []].append(i) }
      }
    }
  }
  if !wildcardIndices.isEmpty {
    byTag = byTag.mapValues { mergeSortedAsc($0, wildcardIndices) }
  }
  return CompiledDispatch(
    rules: compiled, byTag: byTag, wildcardIndices: wildcardIndices, textIndices: textIndices,
    commentIndices: commentIndices)
}

/// `getDispatchIndices(dispatch, node)`.
private func getDispatchIndices(_ dispatch: CompiledDispatch, _ node: DOMNode) -> [Int] {
  if node is DOMText { return dispatch.textIndices }
  if node is DOMComment { return dispatch.commentIndices }
  if node is DOMElement { return dispatch.byTag[node.nodeName] ?? dispatch.wildcardIndices }
  return []
}

/// `CompiledOverlayRules`: rules compiled once with `defineOverlayRules`,
/// for `importChildren(rules:)` or `ImportOverlays`.
public final class CompiledOverlayRules {
  let dispatch: CompiledDispatch
  let rules: [DOMImportRule]
  init(_ rules: [DOMImportRule]) {
    self.rules = rules
    self.dispatch = compileImportRules(rules)
  }
}

/// `flattenRuleEntries(entries)`.
func flattenRuleEntries(_ entries: [DOMImportRuleEntry]) -> [DOMImportRule] {
  entries.flatMap { entry -> [DOMImportRule] in
    switch entry {
    case .rule(let rule): [rule]
    case .overlay(let overlay): overlay.rules
    }
  }
}

/// `defineOverlayRules(entries)`: compiles rules (and other overlays,
/// inlined in place) into an overlay, tried front to back.
public func defineOverlayRules(_ entries: [DOMImportRuleEntry]) -> CompiledOverlayRules {
  CompiledOverlayRules(flattenRuleEntries(entries))
}

/// `defineOverlayRules` of rules only.
public func defineOverlayRules(_ rules: [DOMImportRule]) -> CompiledOverlayRules {
  CompiledOverlayRules(rules)
}

// MARK: - Context (ImportContext.ts)

/// `ImportStateConfig<V>`: a typed key of the import context.
public typealias ImportStateConfig<V> = ContextConfig<V>

/// `createImportState(name, getDefaultValue, isEqual)`. The default is
/// computed once and shared, so it must not be mutated.
public func createImportState<V>(
  _ name: String, _ getDefaultValue: () -> V, _ isEqual: ((V, V) -> Bool)? = nil
) -> ImportStateConfig<V> {
  createContextState(.import, name, getDefaultValue, isEqual)
}

/// `ImportSourceKind`: what started the import.
public enum ImportSourceKind: String, Sendable {
  case paste, unknown
}

/// `ImportSource`: how this import was started (`unknown` by default).
public let ImportSource = createImportState("importSource") { ImportSourceKind.unknown }

/// `ImportSourceDataTransfer`: the `DataTransfer` a paste or drop came
/// from, if any.
public let ImportSourceDataTransfer = createImportState("importSourceDataTransfer") {
  DataTransfer?.none
}

/// `ImportTextFormat`: the text format bits for text nodes made in the
/// current subtree.
public let ImportTextFormat = createImportState("textFormat") { 0 }

/// `ImportTextStyle`: CSS properties for text nodes made in the current
/// subtree (the `getStyleObjectFromCSS` shape).
public let ImportTextStyle = createImportState("textStyle") { StyleObject() }

/// `IsPreserveWhitespaceDom`.
public typealias IsPreserveWhitespaceDom = (DOMNode) -> Bool
/// `IsInlineForWhitespace`.
public typealias IsInlineForWhitespace = (DOMNode) -> Bool

/// `WhitespaceImportConfig`: how text whitespace collapses.
public struct WhitespaceImportConfig {
  public var preservesWhitespace: IsPreserveWhitespaceDom
  public var isInline: IsInlineForWhitespace
  public init(
    preservesWhitespace: @escaping IsPreserveWhitespaceDom = defaultPreservesWhitespace,
    isInline: @escaping IsInlineForWhitespace = defaultIsInline
  ) {
    self.preservesWhitespace = preservesWhitespace
    self.isInline = isInline
  }
}

/// `defaultPreservesWhitespace(node)`: `<pre>`, or `white-space: pre*`.
public func defaultPreservesWhitespace(_ node: DOMNode) -> Bool {
  guard let element = node as? DOMElement else { return false }
  if element.nodeName == "PRE" { return true }
  return element.style.getPropertyValue("white-space").hasPrefix("pre")
}

/// `defaultIsInline(node)`: text, or an element with an inline `display`,
/// or else an inline tag that is not a block tag.
public func defaultIsInline(_ node: DOMNode) -> Bool {
  if node is DOMText { return true }
  guard let element = node as? DOMElement else { return false }
  let display = element.style.getPropertyValue("display")
  if !display.isEmpty { return display.hasPrefix("inline") }
  if isBlockDomNode(element) { return false }
  return isInlineDomNode(element)
}

/// `ImportWhitespaceConfig`.
public let ImportWhitespaceConfig = createImportState("whitespaceConfig") {
  WhitespaceImportConfig()
}

/// `ImportOverlays`: overlays in effect for the whole walk; later entries
/// are higher priority.
public let ImportOverlays = createImportState("importOverlays") { [CompiledOverlayRules]() }

/// `ImportSession`: the walk's document-order store, the root layer of its
/// context record.
public final class ImportSession {
  let record: ContextRecord
  init(_ record: ContextRecord) { self.record = record }

  public func get<V>(_ cfg: ImportStateConfig<V>) -> V { getContextValue(record, cfg) }
  public func set<V>(_ cfg: ImportStateConfig<V>, _ value: V) { record.set(cfg, value) }
  public func update<V>(_ cfg: ImportStateConfig<V>, _ updater: (V) -> V) {
    record.set(cfg, updater(getContextValue(record, cfg)))
  }
  /// Whether the slot was written in this session.
  public func has<V>(_ cfg: ImportStateConfig<V>) -> Bool { record.hasOwn(cfg) }
}

private func getDefaultImportContext(_ editor: Editor) throws -> ContextRecord? {
  try getPeerDependencyFromEditor(editor, DOMImportExtensionName, as: type(of: DOMImportExtension))?
    .output.defaults
}

/// `$getImportContextValue(cfg, editor)`.
public func getImportContextValue<V>(
  _ cfg: ImportStateConfig<V>, _ editor: Editor? = nil
) throws
  -> V
{
  let editor = try editor ?? getEditor()
  return getContextValue(
    try getContextRecord(.import, editor) ?? getDefaultImportContext(editor), cfg)
}

/// `$withImportContext(cfg, editor)(f)`.
public func withImportContext<T>(
  _ cfg: [ContextPairOrUpdater], _ editor: Editor? = nil, _ f: () throws -> T
) throws -> T {
  let editor = try editor ?? getEditor()
  return try withContext(.import, getDefaultImportContext, cfg, editor, f)
}

// MARK: - Rule context and options (types.ts)

/// `ImportChildrenOpts`.
public struct ImportChildrenOpts {
  /// Validates and repackages the produced children.
  public var schema: ChildSchema?
  /// Substitutes or drops (`nil`) each produced child.
  public var onChild: ((LexicalNode) throws -> LexicalNode?)?
  /// Runs on all the children before the schema.
  public var after: (([LexicalNode]) throws -> [LexicalNode])?
  /// Context for the children traversal.
  public var context: [ContextPairOrUpdater]?
  /// Rules tried before every other for this traversal.
  public var rules: CompiledOverlayRules?

  public init(
    schema: ChildSchema? = nil, onChild: ((LexicalNode) throws -> LexicalNode?)? = nil,
    after: (([LexicalNode]) throws -> [LexicalNode])? = nil,
    context: [ContextPairOrUpdater]? = nil, rules: CompiledOverlayRules? = nil
  ) {
    self.schema = schema
    self.onChild = onChild
    self.after = after
    self.context = context
    self.rules = rules
  }
}

/// `DOMImportContext`: what a rule's `$import` receives.
public struct DOMImportContext {
  /// Captures from this rule's selector.
  public let captures: ImportCaptures
  /// The walk's shared store.
  public let session: ImportSession
  fileprivate let runtime: ImportRuntime

  /// Reads a typed context value.
  public func get<V>(_ cfg: ImportStateConfig<V>) throws -> V {
    try getImportContextValue(cfg, runtime.editor)
  }

  /// `$importChildren(parent, opts)`.
  public func importChildren(
    _ parent: DOMNode, _ opts: ImportChildrenOpts? = nil
  ) throws
    -> [LexicalNode]
  {
    try importChildrenInternal(runtime, parent, opts)
  }

  /// `$importOne(node, {context})`.
  public func importOne(
    _ node: DOMNode, context: [ContextPairOrUpdater]? = nil
  ) throws
    -> [LexicalNode]
  {
    try importOneInternal(runtime, node, context)
  }
}

/// `ChildSchema`: which children a parent accepts and what becomes of the
/// rest.
public struct ChildSchema {
  public enum OnReject: Sendable {
    case hoist, drop
  }
  public var name: String?
  public var accepts: (LexicalNode, LexicalNode?) throws -> Bool
  /// Packages a maximal run of rejected siblings; `[]` falls back to
  /// `onReject`.
  public var packageRun: (([LexicalNode], LexicalNode?, DOMNode?) throws -> [LexicalNode])?
  public var onReject: OnReject
  /// A final pass over the assembled children.
  public var finalize: (([LexicalNode], LexicalNode?) throws -> [LexicalNode])?

  public init(
    name: String? = nil, accepts: @escaping (LexicalNode, LexicalNode?) throws -> Bool,
    packageRun: (([LexicalNode], LexicalNode?, DOMNode?) throws -> [LexicalNode])? = nil,
    onReject: OnReject = .drop,
    finalize: (([LexicalNode], LexicalNode?) throws -> [LexicalNode])? = nil
  ) {
    self.name = name
    self.accepts = accepts
    self.packageRun = packageRun
    self.onReject = onReject
    self.finalize = finalize
  }
}

/// `DOMPreprocessContext`.
public struct DOMPreprocessContext {
  public let session: ImportSession
}

/// `DOMPreprocessFn`: runs before the walk; may mutate the DOM, write to
/// the session, and call `next` to run the preprocessors below it.
public typealias DOMPreprocessFn = (DOMNode, DOMPreprocessContext, () throws -> Void) throws -> Void

/// `GenerateNodesFromDOMOptions`.
public struct GenerateNodesFromDOMOptions {
  public var context: [ContextPairOrUpdater]?
  /// Run before (and around) the configured preprocessors; the last runs
  /// first.
  public var preprocess: [DOMPreprocessFn]?
  public init(context: [ContextPairOrUpdater]? = nil, preprocess: [DOMPreprocessFn]? = nil) {
    self.context = context
    self.preprocess = preprocess
  }
}

// MARK: - The walk (runImport.ts)

private final class ImportRuntime {
  let dispatch: CompiledDispatch
  let editor: Editor
  let session: ImportSession
  /// Overlays pushed by `importChildren(rules:)`; the last is tried first.
  var overlays: [CompiledDispatch]

  init(
    dispatch: CompiledDispatch, editor: Editor, session: ImportSession,
    overlays: [CompiledDispatch]
  ) {
    self.dispatch = dispatch
    self.editor = editor
    self.session = session
    self.overlays = overlays
  }
}

private func importChildrenInternal(
  _ runtime: ImportRuntime, _ parent: DOMNode, _ opts: ImportChildrenOpts?
) throws -> [LexicalNode] {
  let overlay = opts?.rules?.dispatch
  if let overlay { runtime.overlays.append(overlay) }
  defer { if overlay != nil { runtime.overlays.removeLast() } }
  if let context = opts?.context {
    return try withImportContext(context, runtime.editor) {
      try importChildrenRun(runtime, parent, opts)
    }
  }
  return try importChildrenRun(runtime, parent, opts)
}

private func importChildrenRun(
  _ runtime: ImportRuntime, _ parent: DOMNode, _ opts: ImportChildrenOpts?
) throws -> [LexicalNode] {
  var collected: [LexicalNode] = []
  for child in parent.childNodes {
    for lex in try importOneInternal(runtime, child, nil) {
      if let onChild = opts?.onChild {
        if let result = try onChild(lex) { collected.append(result) }
      } else {
        collected.append(lex)
      }
    }
  }
  let afterApplied = try opts?.after?(collected) ?? collected
  guard let schema = opts?.schema else { return afterApplied }
  return try applySchema(schema, afterApplied, nil, parent)
}

private func importOneInternal(
  _ runtime: ImportRuntime, _ node: DOMNode, _ context: [ContextPairOrUpdater]?
) throws -> [LexicalNode] {
  if let context {
    return try withImportContext(context, runtime.editor) { try dispatchNode(runtime, node) }
  }
  return try dispatchNode(runtime, node)
}

private func dispatchNode(_ runtime: ImportRuntime, _ node: DOMNode) throws -> [LexicalNode] {
  // Overlays first, top of the stack first; the main dispatcher last.
  var candidates: [(dispatch: CompiledDispatch, indices: [Int])] = []
  for d in runtime.overlays.reversed() {
    let idx = getDispatchIndices(d, node)
    if !idx.isEmpty { candidates.append((d, idx)) }
  }
  let mainIdx = getDispatchIndices(runtime.dispatch, node)
  if !mainIdx.isEmpty { candidates.append((runtime.dispatch, mainIdx)) }
  if candidates.isEmpty { return try hoistChildrenOf(runtime, node) }
  var groupCursor = 0
  var ruleCursor = 0
  var next: ImportNext!
  next = {
    while groupCursor < candidates.count {
      let (dispatch, indices) = candidates[groupCursor]
      while ruleCursor < indices.count {
        let rule = dispatch.rules[indices[ruleCursor]].rule
        ruleCursor += 1
        var captures: ImportCaptures = [:]
        if rule.predicate(node, &captures) {
          let ctx = DOMImportContext(
            captures: captures, session: runtime.session, runtime: runtime)
          // JS also logs the rule's name with `console.error` in dev
          // builds; the error itself propagates the same way.
          return try rule.import(ctx, node, next)
        }
      }
      groupCursor += 1
      ruleCursor = 0
    }
    return try hoistChildrenOf(runtime, node)
  }
  return try next()
}

/// With no rule left, the node's children take its place.
private func hoistChildrenOf(_ runtime: ImportRuntime, _ node: DOMNode) throws -> [LexicalNode] {
  try node.childNodes.flatMap { try importOneInternal(runtime, $0, nil) }
}

/// `$runImport(dispatch, editor, dom, session)`: imports the children of
/// `dom` (a document's body) through `RootSchema`.
func runImport(
  _ dispatch: CompiledDispatch, _ editor: Editor, _ dom: DOMNode, _ session: ImportSession
) throws -> [LexicalNode] {
  let runtime = ImportRuntime(
    dispatch: dispatch, editor: editor, session: session,
    overlays: session.get(ImportOverlays).map(\.dispatch))
  let rootParent: DOMNode = (dom as? DOMDocument)?.body ?? dom
  return try importChildrenRun(runtime, rootParent, ImportChildrenOpts(schema: RootSchema))
}

// MARK: - Schemas (schemas.ts)

/// `$isBlockLevel(node)`: a block element or a block decorator.
public func isBlockLevel(_ node: LexicalNode) throws -> Bool {
  if try isBlockElementNode(node) { return true }
  if let decorator = node as? DecoratorNode { return try !decorator.isInline() }
  return false
}

/// `$distributeInlineWrapper(children, makeWrapper)`: wraps each run of
/// inline children in a fresh wrapper, and recurses into block children so
/// they stay at the top level.
public func distributeInlineWrapper(
  _ children: [LexicalNode], _ makeWrapper: () throws -> ElementNode
) throws -> [LexicalNode] {
  var out: [LexicalNode] = []
  var inlineRun: [LexicalNode] = []
  func flushInline() throws {
    if inlineRun.isEmpty { return }
    out.append(try makeWrapper().splice(0, 0, inlineRun))
    inlineRun = []
  }
  for child in children {
    if try isBlockLevel(child) {
      try flushInline()
      if let element = child as? ElementNode {
        let wrapped = try distributeInlineWrapper(element.getChildren(), makeWrapper)
        try element.splice(0, element.getChildrenSize(), wrapped)
      }
      out.append(child)
    } else {
      inlineRun.append(child)
    }
  }
  try flushInline()
  return out
}

/// `$applySchema(schema, children, parent, domParent)`.
func applySchema(
  _ schema: ChildSchema, _ children: [LexicalNode], _ parent: LexicalNode?, _ domParent: DOMNode?
) throws -> [LexicalNode] {
  var out: [LexicalNode] = []
  var run: [LexicalNode]? = nil
  func flushRun() throws {
    guard let rejected = run else { return }
    run = nil
    if let packageRun = schema.packageRun {
      let packaged = try packageRun(rejected, parent, domParent)
      if !packaged.isEmpty {
        out += packaged
        return
      }
    }
    if schema.onReject == .hoist { out += rejected }
  }
  for child in children {
    if try schema.accepts(child, parent) {
      try flushRun()
      out.append(child)
    } else {
      run = (run ?? []) + [child]
    }
  }
  try flushRun()
  return try schema.finalize?(out, parent) ?? out
}

private let ALIGNMENT_VALUES: Set<String> = ["center", "end", "justify", "left", "right", "start"]

/// `isAlignmentValue(value)`: a non-empty `ElementFormatType`.
func isAlignmentValue(_ value: String) -> Bool { ALIGNMENT_VALUES.contains(value) }

/// `$propagateTextAlignToBlockChildren(children, domParent)`: gives the
/// block children without a format the `text-align` of `domParent`.
@discardableResult
public func propagateTextAlignToBlockChildren(
  _ children: [LexicalNode], _ domParent: DOMNode?
) throws -> [LexicalNode] {
  guard let element = domParent as? DOMElement else { return children }
  let textAlign = element.style.getPropertyValue("text-align")
  guard isAlignmentValue(textAlign) else { return children }
  for child in children {
    if try isBlockElementNode(child), let block = child as? ElementNode,
      try block.getFormatType() == ""
    {
      try block.setFormat(textAlign)
    }
  }
  return children
}

/// Wraps an inline run in a paragraph with the `text-align` of
/// `domParent`; a lone line break becomes an empty paragraph.
private func paragraphPackageRun(
  _ run: [LexicalNode], _ parent: LexicalNode?, _ domParent: DOMNode?
) throws -> [LexicalNode] {
  let run = run.count == 1 && run[0] is LineBreakNode ? [] : run
  let paragraph = try createParagraphNode()
  if let element = domParent as? DOMElement {
    let textAlign = element.style.getPropertyValue("text-align")
    if isAlignmentValue(textAlign) { try paragraph.setFormat(textAlign) }
  }
  return [try paragraph.splice(0, 0, run)]
}

/// `BlockSchema`: blocks, with inline runs wrapped in paragraphs.
nonisolated(unsafe) public let BlockSchema = ChildSchema(
  name: "BlockSchema", accepts: { child, _ in try isBlockLevel(child) },
  packageRun: paragraphPackageRun)

/// `InlineSchema`: inline children; block runs are dropped.
nonisolated(unsafe) public let InlineSchema = ChildSchema(
  name: "InlineSchema", accepts: { child, _ in try !isBlockLevel(child) })

/// `NestedBlockSchema`: blocks, with inline runs passed through.
nonisolated(unsafe) public let NestedBlockSchema = ChildSchema(
  name: "NestedBlockSchema", accepts: { child, _ in try isBlockLevel(child) },
  packageRun: { run, _, _ in run })

/// `RootSchema`: the top level of an import, as `BlockSchema`.
nonisolated(unsafe) public let RootSchema = ChildSchema(
  name: "RootSchema", accepts: { child, _ in try isBlockLevel(child) },
  packageRun: paragraphPackageRun)

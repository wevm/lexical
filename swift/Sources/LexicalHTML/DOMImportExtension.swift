import Lexical
import LexicalExtension

// Port of packages/lexical-html/src/import/DOMImportExtension.ts,
// CoreImportExtension.ts and HorizontalRuleImportExtension.ts
// (`@experimental` upstream).

/// `DOMImportConfig`.
public struct DOMImportConfig {
  /// The rules, tried front to back. Contributions are prepended, so an
  /// extension's rules outrank its dependencies'.
  public var rules: [DOMImportRuleEntry]
  /// Context applied to every import.
  public var contextDefaults: [ContextPairOrUpdater]
  /// Run before the walk, from the last entry; contributions are appended.
  public var preprocess: [DOMPreprocessFn]

  public init(
    rules: [DOMImportRuleEntry] = [], contextDefaults: [ContextPairOrUpdater] = [],
    preprocess: [DOMPreprocessFn] = []
  ) {
    self.rules = rules
    self.contextDefaults = contextDefaults
    self.preprocess = preprocess
  }
}

/// `DOMImportExtensionOutput`.
public struct DOMImportExtensionOutput {
  let generate: (DOMNode, GenerateNodesFromDOMOptions?) throws -> [LexicalNode]
  /// The editor-level context record (`@internal` upstream).
  public let defaults: ContextRecord?

  /// `$generateNodesFromDOM(dom, options)`: the Lexical nodes for a
  /// document (its body) or a parent node. Call it inside `editor.read` or
  /// `update`.
  public func generateNodesFromDOM(
    _ dom: DOMNode, _ options: GenerateNodesFromDOMOptions? = nil
  ) throws -> [LexicalNode] {
    try generate(dom, options)
  }
}

/// `$runPreprocessStack(stack, dom, ctx)`: the last entry first, each
/// deferring to the one below with `next`.
private func runPreprocessStack(
  _ stack: [DOMPreprocessFn], _ dom: DOMNode, _ ctx: DOMPreprocessContext
) throws {
  var i = stack.count - 1
  var next: (() throws -> Void)!
  next = {
    if i >= 0 {
      let cur = stack[i]
      i -= 1
      try cur(dom, ctx, next)
    }
  }
  try next()
}

/// `DefaultHoistRule`: the last rule tried. An element no other rule
/// takes is replaced by its imported children.
nonisolated(unsafe) public let DefaultHoistRule = defineImportRule(
  name: "@lexical/html/default-hoist", match: sel.any()
) { ctx, el, _ in try ctx.importChildren(el) }

let DOMImportExtensionName = "@lexical/html/DOMImport"

/// `DOMImportExtension`: imports HTML through rules compiled into a
/// dispatcher (`@experimental`). The legacy `generateNodesFromDOM` and
/// `importDOM` keep working alongside it.
public let DOMImportExtension = defineExtension(
  name: DOMImportExtensionName,
  config: DOMImportConfig(
    rules: [.rule(DefaultHoistRule)], preprocess: [inlineStylesFromStyleSheets]),
  mergeConfig: { config, partial in
    var merged = partial.applied(to: config)
    if let contextDefaults = partial.contextDefaults {
      merged.contextDefaults = config.contextDefaults + contextDefaults
    }
    if let preprocess = partial.preprocess { merged.preprocess = config.preprocess + preprocess }
    if let rules = partial.rules { merged.rules = rules + config.rules }
    return merged
  },
  build: { editor, config, _ in
    let dispatch = compileImportRules(flattenRuleEntries(config.rules))
    let defaults = contextFromPairs(config.contextDefaults, nil)
    let configPreprocess = config.preprocess
    return DOMImportExtensionOutput(
      generate: { dom, options in
        // The session record is the walk's root layer: a fresh child of
        // the ambient import context (or the editor's defaults), so
        // session writes never leak outward.
        let parentRecord = getContextRecord(.import, editor) ?? defaults
        let fromOpts = options?.context.map { contextFromPairs($0, parentRecord) } ?? parentRecord
        let sessionRecord =
          fromOpts != nil && fromOpts !== parentRecord
          ? fromOpts! : ContextRecord(parent: parentRecord)
        let session = ImportSession(sessionRecord)
        let stack = configPreprocess + (options?.preprocess ?? [])
        try runPreprocessStack(stack, dom, DOMPreprocessContext(session: session))
        return try withFullContext(.import, sessionRecord, editor) {
          try runImport(dispatch, editor, dom, session)
        }
      }, defaults: defaults)
  })

/// `$generateNodesFromDOMViaExtension(dom, options)`: the editor's
/// `DOMImportExtension` import. Throws when the editor was built without it.
public func generateNodesFromDOMViaExtension(
  _ dom: DOMNode, _ options: GenerateNodesFromDOMOptions? = nil
) throws -> [LexicalNode] {
  try getExtensionOutput(DOMImportExtension).generateNodesFromDOM(dom, options)
}

/// `CoreImportExtension`: `CoreImportRules` in a `DOMImportExtension`.
/// Node extensions that contribute rules depend on it.
public let CoreImportExtension = defineExtension(
  name: "@lexical/html/CoreImport",
  dependencies: [
    configExtension(DOMImportExtension) { $0.rules = CoreImportRules.map(\.entry) }
  ])

/// `HorizontalRuleImportRules`: the `<hr>` rule, which `CoreImportRules`
/// already includes.
nonisolated(unsafe) public let HorizontalRuleImportRules = [HorizontalRuleRule]

// MARK: - inlineStylesFromStyleSheets.ts

/// `$inlineStylesFromStyleSheets`: copies the declarations of `<style>`
/// rules onto the elements they match, as inline styles that don't
/// override the element's own. Only for a document.
///
/// JS reads the document's CSSOM. Swift parses each `<style>` sheet's
/// rules with `parseStyleSheet` (at-rules are skipped) and matches their
/// selectors with `querySelectorAll`; a selector it rejects is skipped, as
/// JS skips one `querySelectorAll` throws on. Selectors outside its subset
/// (pseudo-classes, namespaces, escapes) are skipped too, and shorthand
/// properties are copied without the longhands a browser expands them into.
public func inlineStylesFromStyleSheets(
  _ dom: DOMNode, _ ctx: DOMPreprocessContext, _ next: () throws -> Void
) throws {
  inlineStylesFromStyleSheetsDOM(dom)
  try next()
}

func inlineStylesFromStyleSheetsDOM(_ dom: DOMNode) {
  guard let doc = dom as? DOMDocument, let root = doc.documentElement else { return }
  let styles = ([root] + root.descendantElements()).filter { $0.localName == "style" }
  if styles.isEmpty { return }
  var originalInlineStyles: [ObjectIdentifier: Set<String>] = [:]
  for style in styles {
    for case .style(let selectorText, let declaration) in parseStyleSheet(style.textContent) {
      guard let elements = try? doc.querySelectorAll(selectorText) else { continue }
      for el in elements {
        let original =
          originalInlineStyles[ObjectIdentifier(el)] ?? Set(el.style.propertyNames)
        originalInlineStyles[ObjectIdentifier(el)] = original
        for prop in declaration.propertyNames where !original.contains(prop) {
          el.style.setProperty(
            prop, declaration.getPropertyValue(prop), declaration.getPropertyPriority(prop))
        }
      }
    }
  }
}

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
/// style rules (at-rules are skipped) and matches their selectors with
/// `parseSelector`'s subset; a rule whose selector is outside it
/// (combinators, pseudo-classes) is skipped, as JS skips a selector
/// `querySelectorAll` rejects.
public func inlineStylesFromStyleSheets(
  _ dom: DOMNode, _ ctx: DOMPreprocessContext, _ next: () throws -> Void
) throws {
  inlineStylesFromStyleSheetsDOM(dom)
  try next()
}

func inlineStylesFromStyleSheetsDOM(_ dom: DOMNode) {
  guard let doc = dom as? DOMDocument, let root = doc.documentElement else { return }
  let elements = root.descendantElements()
  let styles = elements.filter { $0.localName == "style" }
  if styles.isEmpty { return }
  var originalInlineStyles: [ObjectIdentifier: Set<String>] = [:]
  func originalProps(_ el: DOMElement) -> Set<String> {
    if let props = originalInlineStyles[ObjectIdentifier(el)] { return props }
    let props = Set(el.style.propertyNames)
    originalInlineStyles[ObjectIdentifier(el)] = props
    return props
  }
  // A scratch element parses declarations as `CSSStyleDeclaration` does.
  let scratch = doc.createElement("div")
  for style in styles {
    for rule in parseStyleRules(style.textContent) {
      guard let selector = try? parseSelector(rule.selector) else { continue }
      scratch.setAttribute("style", rule.declarations)
      let declaration = scratch.style
      let predicate = selector.predicate
      for el in elements {
        var captures: ImportCaptures = [:]
        if !selector.tags.isEmpty && !selector.tags.contains(el.nodeName) { continue }
        guard predicate(el, &captures) else { continue }
        let original = originalProps(el)
        for prop in declaration.propertyNames where !original.contains(prop) {
          el.style.setProperty(
            prop, declaration.getPropertyValue(prop), declaration.getPropertyPriority(prop))
        }
      }
    }
  }
}

/// The style rules of a style sheet, as selector text and declaration
/// text. Drops comments and the `<!--`/`-->` tokens, and skips at-rules.
private func parseStyleRules(_ css: String) -> [(selector: String, declarations: String)] {
  var text = css.replacing(/\/\*[\s\S]*?\*\//, with: "")
  text = text.replacing("<!--", with: " ").replacing("-->", with: " ")
  var rules: [(String, String)] = []
  var chars = Substring(text)
  while let open = chars.firstIndex(of: "{") {
    let prelude = jsTrim(String(chars[..<open]))
    // Find the matching close brace.
    var depth = 0
    var close = open
    var i = open
    while i < chars.endIndex {
      if chars[i] == "{" { depth += 1 }
      if chars[i] == "}" {
        depth -= 1
        if depth == 0 {
          close = i
          break
        }
      }
      i = chars.index(after: i)
    }
    if depth != 0 { close = chars.endIndex }
    let body = chars[chars.index(after: open)..<close]
    if !prelude.hasPrefix("@") {
      rules.append((prelude, String(body)))
    }
    chars = close < chars.endIndex ? chars[chars.index(after: close)...] : ""
  }
  return rules
}

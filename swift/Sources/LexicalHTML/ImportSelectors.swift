import Lexical

// Port of packages/lexical-html/src/import/sel.ts and parseCss.ts: the
// selectors import rules match DOM nodes with (`@experimental` upstream).

/// `RegExpMatchArray`: a regex match a selector captured, as
/// `String.prototype.match` returns it: `[0]` is the whole match and each
/// further index a group (`nil` when it did not participate).
public struct RegExpMatch {
  public let groups: [String?]
  /// The named groups (`match.groups`).
  public let namedGroups: [String: String]
  /// Where the match starts, in UTF-16 code units (`match.index`).
  public let index: Int

  public subscript(_ i: Int) -> String? { i < groups.count ? groups[i] : nil }
  public var count: Int { groups.count }

  /// `value.match(re)`: the first match of `regex` in `value`.
  static func match(_ value: String, _ regex: Regex<AnyRegexOutput>) -> RegExpMatch? {
    guard let m = value.firstMatch(of: regex) else { return nil }
    var named: [String: String] = [:]
    for element in m.output {
      if let name = element.name, let substring = element.substring {
        named[name] = String(substring)
      }
    }
    return RegExpMatch(
      groups: m.output.map { $0.substring.map(String.init) }, namedGroups: named,
      index: value.utf16.distance(from: value.startIndex, to: m.range.lowerBound))
  }
}

/// The named regex captures of a rule invocation (`ctx.captures`).
public typealias ImportCaptures = [String: RegExpMatch]

/// A selector predicate, which may record captures. Returns whether the
/// node matches.
typealias SelectorPredicate = (DOMNode, inout ImportCaptures) -> Bool

/// `SelectorKind`.
enum SelectorKind {
  case element, text, comment
}

/// `CompiledSelector<N>`: the `match` of a `DOMImportRule`. `N` is the DOM
/// node type it matches: `DOMElement` for `sel.tag`, `sel.any` and
/// `sel.css`, `DOMText` for `sel.text()`, `DOMComment` for
/// `sel.comment()`. Element selectors refine with `classAll`, `classAny`,
/// `attr` and `styleAny` (JS `ElementSelectorBuilder`).
public struct CompiledSelector<N: DOMNode> {
  let kind: SelectorKind
  /// Upper-cased tag names the selector is restricted to; empty for a
  /// wildcard element selector and for text and comment selectors.
  let tags: Set<String>
  let predicates: [SelectorPredicate]

  /// The composed predicate.
  var predicate: SelectorPredicate {
    switch kind {
    case .text: return { node, _ in node is DOMText }
    case .comment: return { node, _ in node is DOMComment }
    case .element: return combinePredicates(predicates)
    }
  }
}

private func combinePredicates(_ preds: [SelectorPredicate]) -> SelectorPredicate {
  if preds.isEmpty { return { node, _ in node is DOMElement } }
  if preds.count == 1 { return preds[0] }
  return { node, captures in
    for p in preds where !p(node, &captures) { return false }
    return true
  }
}

/// `buildSelector(tags, predicates)`.
func buildSelector(
  _ tags: Set<String>, _ predicates: [SelectorPredicate]
) -> CompiledSelector<
  DOMElement
> {
  CompiledSelector(kind: .element, tags: tags, predicates: predicates)
}

extension CompiledSelector where N == DOMElement {
  private func refine(_ additional: @escaping SelectorPredicate) -> Self {
    buildSelector(tags, predicates + [additional])
  }

  /// Require every listed class to be present on the element.
  public func classAll(_ classes: String...) -> Self { refine(buildClassAllPredicate(classes)) }

  /// Require at least one of the listed classes to be present.
  public func classAny(_ classes: String...) -> Self { refine(buildClassAnyPredicate(classes)) }

  /// `attr(name, true)`: require the attribute to be present.
  public func attr(_ name: String) -> Self { refine(buildAttrPredicate(name, .present)) }

  /// `attr(name, value)`: require the attribute to equal `value`.
  public func attr(_ name: String, _ value: String) -> Self {
    refine(buildAttrPredicate(name, .equals(value)))
  }

  /// `attr(name, regex, {capture})`: require the attribute to match
  /// `regex`, recording the match as `ctx.captures[capture]`.
  public func attr<O>(_ name: String, _ regex: Regex<O>, capture: String? = nil) -> Self {
    refine(buildAttrPredicate(name, .matches(Regex<AnyRegexOutput>(regex), capture)))
  }

  /// `styleAny(prop, value)`: require the inline style declaration to equal
  /// `value`.
  public func styleAny(_ prop: String, _ value: String) -> Self {
    refine { node, _ in
      (node as? DOMElement)?.style.getPropertyValue(prop) == value
    }
  }

  /// `styleAny(prop, regex, {capture})`.
  public func styleAny<O>(_ prop: String, _ regex: Regex<O>, capture: String? = nil) -> Self {
    let regex = Regex<AnyRegexOutput>(regex)
    return refine { node, captures in
      guard let element = node as? DOMElement else { return false }
      let v = element.style.getPropertyValue(prop)
      guard !v.isEmpty, let m = RegExpMatch.match(v, regex) else { return false }
      if let capture { captures[capture] = m }
      return true
    }
  }
}

func buildClassAllPredicate(_ classes: [String]) -> SelectorPredicate {
  let ns = classes.filter { !$0.isEmpty }
  if ns.isEmpty { return { _, _ in true } }
  return { node, _ in
    guard let element = node as? DOMElement else { return false }
    let cl = element.classList
    return ns.allSatisfy { cl.contains($0) }
  }
}

func buildClassAnyPredicate(_ classes: [String]) -> SelectorPredicate {
  let ns = classes.filter { !$0.isEmpty }
  if ns.isEmpty { return { _, _ in false } }
  return { node, _ in
    guard let element = node as? DOMElement else { return false }
    let cl = element.classList
    return ns.contains { cl.contains($0) }
  }
}

enum AttrMatch {
  case present
  case equals(String)
  case matches(Regex<AnyRegexOutput>, String?)
}

func buildAttrPredicate(_ name: String, _ value: AttrMatch) -> SelectorPredicate {
  switch value {
  case .present:
    return { node, _ in (node as? DOMElement)?.hasAttribute(name) ?? false }
  case .equals(let expected):
    return { node, _ in (node as? DOMElement)?.getAttribute(name) == expected }
  case .matches(let regex, let capture):
    return { node, captures in
      guard let v = (node as? DOMElement)?.getAttribute(name),
        let m = RegExpMatch.match(v, regex)
      else { return false }
      if let capture { captures[capture] = m }
      return true
    }
  }
}

/// `sel`: builds `CompiledSelector`s (`@experimental` upstream).
public struct SelectorBuilders: Sendable {
  /// Match any element.
  public func any() -> CompiledSelector<DOMElement> { buildSelector([], []) }

  /// Match comment nodes.
  public func comment() -> CompiledSelector<DOMComment> {
    CompiledSelector(kind: .comment, tags: [], predicates: [])
  }

  /// Match text nodes.
  public func text() -> CompiledSelector<DOMText> {
    CompiledSelector(kind: .text, tags: [], predicates: [])
  }

  /// Match by tag name. JS requires at least one name with an invariant;
  /// Swift requires it in the signature.
  public func tag(_ first: String, _ rest: String...) -> CompiledSelector<DOMElement> {
    buildSelector(Set(([first] + rest).map { $0.uppercased() }), [])
  }

  /// `sel.css(source)`: parses a reduced CSS selector subset (see
  /// `parseSelector`).
  public func css(_ source: String) throws -> CompiledSelector<DOMElement> {
    try parseSelector(source)
  }
}

/// `sel`.
public let sel = SelectorBuilders()

/// `isElementOfTag(node, tag)` (`@experimental`): `node` is an element
/// named `tag`, compared case-insensitively.
public func isElementOfTag(_ node: DOMNode?, _ tag: String) -> Bool {
  guard let element = node as? DOMElement else { return false }
  return element.nodeName == tag.uppercased()
}

// MARK: - parseCss.ts

private func isIdentChar(_ c: Character?) -> Bool {
  guard let c, c.isASCII else { return false }
  return c.isLetter || c.isNumber || c == "_" || c == "-"
}

private struct Cursor {
  let source: [Character]
  let text: String
  var pos = 0

  init(_ text: String) {
    self.text = text
    self.source = Array(text)
  }

  func peek(_ offset: Int = 0) -> Character? {
    pos + offset < source.count ? source[pos + offset] : nil
  }
  mutating func consume() -> Character? {
    defer { pos += 1 }
    return peek()
  }
  var eof: Bool { pos >= source.count }
  mutating func skipWhitespace() {
    while let c = peek(), c.isWhitespace { pos += 1 }
  }
  mutating func readIdent() -> String {
    let start = pos
    while isIdentChar(peek()) { pos += 1 }
    return String(source[start..<pos])
  }
  mutating func readQuoted() throws -> String {
    let quote = consume()
    try check(quote == "\"" || quote == "'", "expected quote")
    var value = ""
    while let c = peek(), c != quote {
      if c == "\\" {
        // `\\(.)` unescapes to the escaped character.
        if let escaped = peek(1) { value.append(escaped) }
        pos += 2
      } else {
        value.append(c)
        pos += 1
      }
    }
    try check(!eof, "unterminated string")
    pos += 1
    return value
  }
  func check(_ cond: Bool, _ msg: String) throws {
    try invariant(cond, "invalid CSS selector at col \(pos + 1): \(msg) in \(text)")
  }
}

private struct ParsedSimpleSelector {
  var tags: Set<String> = []
  var predicates: [SelectorPredicate] = []
}

private func parseSimpleSelector(_ c: inout Cursor) throws -> ParsedSimpleSelector {
  var parsed = ParsedSimpleSelector()
  var classes: [String] = []
  var isUniversal = false
  c.skipWhitespace()
  if c.peek() == "*" {
    _ = c.consume()
    isUniversal = true
  } else if isIdentChar(c.peek()) {
    let tag = c.readIdent()
    if !tag.isEmpty { parsed.tags.insert(tag.uppercased()) }
  }
  while let ch = c.peek() {
    if ch == "." {
      _ = c.consume()
      let cls = c.readIdent()
      try c.check(!cls.isEmpty, #"expected class name after ".""#)
      classes.append(cls)
    } else if ch == "#" {
      _ = c.consume()
      let id = c.readIdent()
      try c.check(!id.isEmpty, "expected id after \"#\"")
      parsed.predicates.append(buildAttrPredicate("id", .equals(id)))
    } else if ch == "[" {
      _ = c.consume()
      c.skipWhitespace()
      let name = c.readIdent()
      try c.check(!name.isEmpty, #"expected attribute name after "[""#)
      c.skipWhitespace()
      var value = AttrMatch.present
      if c.peek() == "=" {
        _ = c.consume()
        c.skipWhitespace()
        if c.peek() == "\"" || c.peek() == "'" {
          value = .equals(try c.readQuoted())
        } else {
          let ident = c.readIdent()
          try c.check(!ident.isEmpty, "expected attribute value")
          value = .equals(ident)
        }
        c.skipWhitespace()
      }
      try c.check(c.peek() == "]", #"expected "]""#)
      _ = c.consume()
      parsed.predicates.append(buildAttrPredicate(name, value))
    } else {
      break
    }
  }
  if !classes.isEmpty { parsed.predicates.append(buildClassAllPredicate(classes)) }
  // Neither tag nor refinement is only legitimate for a lone `*`.
  try c.check(
    isUniversal || !parsed.tags.isEmpty || !parsed.predicates.isEmpty, "expected a selector")
  return parsed
}

/// `parseSelector(source)` (`@experimental`): a selector from a reduced CSS
/// subset: tags, `*`, comma lists, `.class`, `#id`, `[attr]` and
/// `[attr="value"]`. Anything else (combinators, pseudo-classes, regex
/// attributes) throws; chain the builder methods instead.
public func parseSelector(_ source: String) throws -> CompiledSelector<DOMElement> {
  var c = Cursor(source)
  var groups: [ParsedSimpleSelector] = []
  while true {
    groups.append(try parseSimpleSelector(&c))
    c.skipWhitespace()
    if c.eof { break }
    try c.check(
      c.peek() == ",", #"expected "," (selector lists are the only supported combinator)"#)
    _ = c.consume()
    c.skipWhitespace()
  }
  if groups.count == 1 {
    return buildSelector(groups[0].tags, groups[0].predicates)
  }
  // Merge tag sets only when every group is tag-restricted, and OR the
  // groups' refinements so a candidate satisfies some group entirely.
  var tags: Set<String> = []
  if groups.allSatisfy({ !$0.tags.isEmpty }) {
    for g in groups { tags.formUnion(g.tags) }
  }
  let orPredicate: SelectorPredicate = { node, captures in
    for g in groups {
      if !g.tags.isEmpty && !g.tags.contains(node.nodeName) { continue }
      if g.predicates.allSatisfy({ $0(node, &captures) }) { return true }
    }
    return false
  }
  return buildSelector(tags, [orPredicate])
}

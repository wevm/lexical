/// `querySelectorAll` and `matches` for the selectors a pasted style sheet
/// uses: compound selectors (a type or `*`, `#id`, `.class`, attribute
/// selectors with every operator and the `i` and `s` flags, and
/// pseudo-classes) joined by the descendant, child (`>`), next-sibling
/// (`+`) and subsequent-sibling (`~`) combinators, in comma lists.
///
/// The pseudo-classes are those of a document nobody interacts with, as
/// jsdom's selector engine (`@asamuzakjp/dom-selector`) matches them: the
/// tree-structural ones (`:root`, `:empty`, `:first-child` and the other
/// child and type positions, `:nth-child(An+B of S)` and the rest of the
/// `:nth-` family), the logical ones (`:not()`, `:is()` and `:where()`,
/// forgiving as in JS, and `:has()`), `:link` and `:any-link`, `:lang()`,
/// `:checked`, `:disabled` and `:enabled`, and `:scope`; the user action
/// and location ones (`:hover`, `:focus`, `:visited`, `:target`...) and
/// pseudo-elements (`::before`, `:first-line`...) match nothing. Their
/// names are lower case, as jsdom's engine takes no other.
///
/// Anything else throws, as an invalid selector does in JS; some of it is
/// valid in JS (namespaces, escapes, the form-state and `:dir()`
/// pseudo-classes), which `SelectorError.unsupported` tells apart. Type
/// selectors and attribute names match in any case, as in an HTML
/// document; `#id` and `.class` match as written.
public enum SelectorError: Error, Equatable {
  /// Not a selector (JS throws a `SyntaxError`).
  case invalid
  /// A valid selector outside the subset above.
  case unsupported
}

private enum AttrOp { case exists, equals, includes, dashMatch, prefix, suffix, substring }

private enum Simple {
  case type(String)
  case id(String)
  case `class`(String)
  case attr(name: String, op: AttrOp, value: String, ignoreCase: Bool)
  case pseudo(Pseudo)
}

/// A pseudo-class (or a pseudo-element, which matches nothing).
private indirect enum Pseudo {
  case root
  case empty
  /// `:nth-child(An+B of S)` and its family: from the end with `last`,
  /// among siblings of the element's type with `ofType`.
  case nth(a: Int, b: Int, last: Bool, ofType: Bool, of: [Complex]?)
  /// `:only-child` and `:only-of-type`.
  case only(ofType: Bool)
  case not([Complex])
  /// `:is()` and `:where()`.
  case `is`([Complex])
  case has([(Combinator, Complex)])
  case anyLink
  case lang(String)
  case checked
  case disabled
  case enabled
  /// A state no element of a parsed document is in, or a pseudo-element.
  case never
}

private enum Combinator { case descendant, child, nextSibling, subsequentSibling }

/// A complex selector, right to left: the subject's compound, then each
/// combinator with the compound to its left.
private struct Complex {
  let subject: [Simple]
  let rest: [(Combinator, [Simple])]
}

private func isIdentStart(_ c: Unicode.Scalar) -> Bool {
  (c.isASCII && (c.properties.isAlphabetic || c == "_")) || !c.isASCII
}
private func isIdentChar(_ c: Unicode.Scalar) -> Bool {
  isIdentStart(c) || ("0"..."9").contains(c) || c == "-"
}
private func isCSSWhitespace(_ c: Unicode.Scalar) -> Bool {
  c == " " || c == "\t" || c == "\n" || c == "\r" || c == "\u{0C}"
}
private func isCSSWhitespace(_ c: Character) -> Bool {
  c.unicodeScalars.allSatisfy(isCSSWhitespace)
}

private struct SelectorParser {
  let chars: [Unicode.Scalar]
  var pos = 0

  func peek(_ offset: Int = 0) -> Unicode.Scalar? {
    pos + offset < chars.count ? chars[pos + offset] : nil
  }

  @discardableResult
  mutating func skipWhitespace() -> Bool {
    let start = pos
    while let c = peek(), isCSSWhitespace(c) { pos += 1 }
    return pos > start
  }

  mutating func ident() throws -> String {
    let start = pos
    if peek() == "-" {
      pos += 1
      if peek() == "-" { pos += 1 }
    }
    switch peek() {
    case "\\": throw SelectorError.unsupported
    case let c? where isIdentStart(c) || pos > start + 1: break
    default: throw SelectorError.invalid
    }
    while let c = peek() {
      if c == "\\" { throw SelectorError.unsupported }
      if !isIdentChar(c) { break }
      pos += 1
    }
    return String(String.UnicodeScalarView(chars[start..<pos]))
  }

  mutating func string() throws -> String {
    guard let quote = peek() else { throw SelectorError.invalid }
    pos += 1
    var value = String.UnicodeScalarView()
    while true {
      switch peek() {
      case nil: return String(value)
      case "\\": throw SelectorError.unsupported
      case "\n": throw SelectorError.invalid
      case let c? where c == quote:
        pos += 1
        return String(value)
      case let c?:
        value.append(c)
        pos += 1
      }
    }
  }

  mutating func attribute() throws -> Simple {
    pos += 1
    skipWhitespace()
    if peek() == "|" { throw SelectorError.unsupported }
    let name = try ident().lowercased()
    skipWhitespace()
    let op: AttrOp
    switch peek() {
    case "]":
      pos += 1
      return .attr(name: name, op: .exists, value: "", ignoreCase: false)
    case "=": op = .equals
    case let c?:
      guard peek(1) == "=" else {
        throw c == "|" ? SelectorError.unsupported : SelectorError.invalid
      }
      pos += 1
      switch c {
      case "~": op = .includes
      case "|": op = .dashMatch
      case "^": op = .prefix
      case "$": op = .suffix
      case "*": op = .substring
      default: throw SelectorError.invalid
      }
    case nil: throw SelectorError.invalid
    }
    pos += 1
    skipWhitespace()
    let value = peek() == "\"" || peek() == "'" ? try string() : try ident()
    skipWhitespace()
    var ignoreCase = false
    if let c = peek(), c == "i" || c == "I" || c == "s" || c == "S" {
      ignoreCase = c == "i" || c == "I"
      pos += 1
      skipWhitespace()
    }
    guard peek() == "]" else { throw SelectorError.invalid }
    pos += 1
    return .attr(name: name, op: op, value: value, ignoreCase: ignoreCase)
  }

  /// A pseudo-class or pseudo-element, after its `:`.
  mutating func pseudo() throws -> Pseudo {
    pos += 1
    let element = peek() == ":"
    if element { pos += 1 }
    let name = try ident()
    // jsdom's engine knows the names in lower case only.
    if name != asciiLowercase(name) { throw SelectorError.invalid }
    let functional = peek() == "("
    if element || ["before", "after", "first-line", "first-letter"].contains(name) {
      if functional { throw SelectorError.unsupported }
      switch name {
      case "before", "after", "first-line", "first-letter", "marker", "selection", "placeholder",
        "backdrop", "file-selector-button", "target-text", "spelling-error", "grammar-error",
        "cue":
        return .never
      default: throw SelectorError.invalid
      }
    }
    if !functional {
      switch name {
      case "root", "scope": return .root
      case "empty": return .empty
      case "first-child", "last-child", "first-of-type", "last-of-type":
        return .nth(
          a: 0, b: 1, last: name.hasPrefix("last"), ofType: name.hasSuffix("of-type"), of: nil)
      case "only-child": return .only(ofType: false)
      case "only-of-type": return .only(ofType: true)
      case "link", "any-link": return .anyLink
      case "checked": return .checked
      case "disabled": return .disabled
      case "enabled": return .enabled
      case "visited", "hover", "active", "focus", "focus-within", "focus-visible", "target",
        "autofill":
        return .never
      case "read-only", "read-write", "required", "optional", "default", "indeterminate",
        "placeholder-shown", "defined", "valid", "invalid", "in-range", "out-of-range",
        "user-valid", "user-invalid", "blank", "playing", "paused", "modal", "fullscreen",
        "popover-open", "open", "closed", "local-link", "current", "past", "future":
        throw SelectorError.unsupported
      default: throw SelectorError.invalid
      }
    }
    pos += 1
    let pseudo: Pseudo
    switch name {
    case "not": pseudo = .not(try selectorList(forgiving: false))
    case "is", "where": pseudo = .is(try selectorList(forgiving: true))
    case "has": pseudo = .has(try relativeList())
    case "nth-child", "nth-last-child", "nth-of-type", "nth-last-of-type":
      let (a, b) = try anPlusB()
      let ofType = name.hasSuffix("of-type")
      skipWhitespace()
      var of: [Complex]?
      if !ofType && peek() != ")" {
        if try ident() != "of" || !skipWhitespace() { throw SelectorError.invalid }
        of = try selectorList(forgiving: false)
      }
      pseudo = .nth(a: a, b: b, last: name.contains("last"), ofType: ofType, of: of)
    case "lang":
      skipWhitespace()
      let range = peek() == "\"" || peek() == "'" ? try string() : try ident()
      skipWhitespace()
      pseudo = .lang(range)
    case "dir", "host", "host-context", "state", "nth-col", "nth-last-col":
      throw SelectorError.unsupported
    default: throw SelectorError.invalid
    }
    guard peek() == ")" else { throw SelectorError.invalid }
    pos += 1
    return pseudo
  }

  /// Digits from here, as text.
  private mutating func digits() -> String {
    let start = pos
    while let c = peek(), ("0"..."9").contains(c) { pos += 1 }
    return String(String.UnicodeScalarView(chars[start..<pos]))
  }

  /// The argument of `:nth-child()` and its family: `odd`, `even`, or
  /// `An+B` with its parts optional.
  mutating func anPlusB() throws -> (Int, Int) {
    skipWhitespace()
    let start = pos
    while let c = peek(),
      ("a"..."z").contains(c) || ("A"..."Z").contains(c) || ("0"..."9").contains(c) || c == "+"
        || c == "-"
    {
      pos += 1
    }
    let text = asciiLowercase(String(String.UnicodeScalarView(chars[start..<pos])))
    if text == "odd" { return (2, 1) }
    if text == "even" { return (2, 0) }
    func number(_ digits: String) throws -> Int {
      guard let n = Int(digits) else { throw SelectorError.invalid }
      return n
    }
    guard let n = text.firstIndex(of: "n") else { return (0, try number(text)) }
    let a: Int
    switch text[..<n] {
    case "", "+": a = 1
    case "-": a = -1
    case let digits: a = try number(String(digits))
    }
    var bText = String(text[text.index(after: n)...])
    if bText.isEmpty {
      // `An + B` with spaces around the sign.
      let hadSpace = skipWhitespace()
      guard hadSpace, let sign = peek(), sign == "+" || sign == "-" else { return (a, 0) }
      pos += 1
      skipWhitespace()
      let rest = digits()
      if rest.isEmpty { throw SelectorError.invalid }
      bText = String(sign) + rest
    } else if !(bText.hasPrefix("+") || bText.hasPrefix("-"))
      || bText.dropFirst().hasPrefix("+") || bText.dropFirst().hasPrefix("-")
    {
      throw SelectorError.invalid
    } else if bText.count == 1 {
      // `An+ B`: the sign, then the digits after a space.
      skipWhitespace()
      let rest = digits()
      if rest.isEmpty { throw SelectorError.invalid }
      bText += rest
    }
    return (a, try number(bText))
  }

  /// A comma list inside a functional pseudo-class, up to its `)`.
  /// Forgiving (`:is()`, `:where()`), an item that isn't a selector is
  /// dropped, and the list may be empty.
  mutating func selectorList(forgiving: Bool) throws -> [Complex] {
    var out: [Complex] = []
    while true {
      let start = pos
      do {
        out.append(try complex())
      } catch SelectorError.invalid where forgiving {
        skipItem(start)
      }
      switch peek() {
      case ",": pos += 1
      case ")": if out.isEmpty && !forgiving { throw SelectorError.invalid } else { return out }
      default: throw SelectorError.invalid
      }
    }
  }

  /// Moves from `start` to the next `,` or `)` at this nesting level.
  private mutating func skipItem(_ start: Int) {
    pos = start
    var depth = 0
    while let c = peek() {
      if c == "(" {
        depth += 1
      } else if c == ")" || c == "," {
        if depth == 0 { return }
        if c == ")" { depth -= 1 }
      }
      pos += 1
    }
  }

  /// `:has()`'s relative selectors, each with its leading combinator.
  mutating func relativeList() throws -> [(Combinator, Complex)] {
    var out: [(Combinator, Complex)] = []
    while true {
      skipWhitespace()
      let combinator: Combinator
      switch peek() {
      case ">": combinator = .child
      case "+": combinator = .nextSibling
      case "~": combinator = .subsequentSibling
      default: combinator = .descendant
      }
      if combinator != .descendant { pos += 1 }
      out.append((combinator, try complex()))
      switch peek() {
      case ",": pos += 1
      case ")": return out
      default: throw SelectorError.invalid
      }
    }
  }

  /// A compound selector, which may be empty.
  mutating func compound() throws -> [Simple] {
    var out: [Simple] = []
    if peek() == "*" {
      pos += 1
      if peek() == "|" { throw SelectorError.unsupported }
      out.append(.type("*"))
    } else if let c = peek(), isIdentStart(c) || c == "-" || c == "\\" {
      let name = try ident()
      if peek() == "|" { throw SelectorError.unsupported }
      out.append(.type(name.lowercased()))
    }
    while true {
      switch peek() {
      case "#":
        pos += 1
        // An id is a name: it may start with a digit.
        let start = pos
        while let c = peek(), isIdentChar(c) { pos += 1 }
        if peek() == "\\" { throw SelectorError.unsupported }
        if pos == start { throw SelectorError.invalid }
        out.append(.id(String(String.UnicodeScalarView(chars[start..<pos]))))
      case ".":
        pos += 1
        out.append(.class(try ident()))
      case "[": out.append(try attribute())
      case ":": out.append(.pseudo(try pseudo()))
      default: return out
      }
    }
  }

  mutating func complex() throws -> Complex {
    skipWhitespace()
    var compounds = [try compound()]
    var combinators: [Combinator] = []
    if compounds[0].isEmpty { throw SelectorError.invalid }
    while true {
      let hadSpace = skipWhitespace()
      let combinator: Combinator
      switch peek() {
      case nil, ",", ")": return finish(compounds, combinators)
      case ">": combinator = .child
      case "+": combinator = .nextSibling
      case "~": combinator = .subsequentSibling
      default:
        guard hadSpace else { throw SelectorError.invalid }
        combinator = .descendant
      }
      if combinator != .descendant {
        pos += 1
        skipWhitespace()
      }
      let next = try compound()
      if next.isEmpty { throw SelectorError.invalid }
      combinators.append(combinator)
      compounds.append(next)
    }
  }

  private func finish(_ compounds: [[Simple]], _ combinators: [Combinator]) -> Complex {
    var compounds = compounds
    let subject = compounds.removeLast()
    return Complex(
      subject: subject, rest: Array(zip(combinators.reversed(), compounds.reversed())))
  }
}

private func parseSelectorList(_ source: String) throws -> [Complex] {
  var parser = SelectorParser(chars: Array(source.unicodeScalars))
  var out: [Complex] = []
  while true {
    out.append(try parser.complex())
    switch parser.peek() {
    case nil: return out
    case ",": parser.pos += 1
    default: throw SelectorError.invalid
    }
  }
}

private func matchesSimple(_ element: DOMElement, _ simple: Simple) -> Bool {
  switch simple {
  case .type(let name): return name == "*" || element.localName.lowercased() == name
  case .id(let id): return element.getAttribute("id") == id
  case .class(let cls):
    return (element.getAttribute("class") ?? "").split(whereSeparator: isCSSWhitespace)
      .contains { $0 == cls }
  case .attr(let name, let op, let value, let ignoreCase):
    guard var actual = element.getAttribute(name) else { return false }
    var value = value
    if ignoreCase {
      actual = actual.lowercased()
      value = value.lowercased()
    }
    switch op {
    case .exists: return true
    case .equals: return actual == value
    case .includes:
      return !value.isEmpty && !value.contains(where: isCSSWhitespace)
        && actual.split(whereSeparator: isCSSWhitespace).contains { String($0) == value }
    case .dashMatch: return actual == value || actual.hasPrefix(value + "-")
    case .prefix: return !value.isEmpty && actual.hasPrefix(value)
    case .suffix: return !value.isEmpty && actual.hasSuffix(value)
    case .substring: return !value.isEmpty && actual.contains(value)
    }
  case .pseudo(let pseudo): return matchesPseudo(element, pseudo)
  }
}

/// The form controls `:disabled` and `:enabled` apply to.
private let formControls: Set<String> = [
  "button", "input", "select", "textarea", "optgroup", "option", "fieldset",
]

private func matchesPseudo(_ element: DOMElement, _ pseudo: Pseudo) -> Bool {
  let name = element.localName
  switch pseudo {
  case .root: return element.parentNode?.nodeType == .document
  case .empty:
    return element.childNodes.allSatisfy {
      !($0 is DOMElement) && !(($0 as? DOMText).map { !$0.data.isEmpty } ?? false)
    }
  case .nth(let a, let b, let last, let ofType, let of):
    if let of, !matchesList(element, of) { return false }
    guard let position = nthPosition(element, last: last, ofType: ofType, of: of) else {
      return false
    }
    let p = position - b
    return a == 0 ? p == 0 : p % a == 0 && p / a >= 0
  case .only(let ofType):
    return nthPosition(element, last: false, ofType: ofType, of: nil) == 1
      && nthPosition(element, last: true, ofType: ofType, of: nil) == 1
  case .not(let list): return !matchesList(element, list)
  case .is(let list): return matchesList(element, list)
  case .has(let relatives):
    return relatives.contains { combinator, complex in
      hasCandidates(element, combinator).contains {
        matchesComplex($0, complex, anchor: (combinator, element))
      }
    }
  case .anyLink: return (name == "a" || name == "area") && element.hasAttribute("href")
  case .lang(let range):
    var node: DOMElement? = element
    while let n = node {
      if let lang = n.getAttribute("lang") {
        let (lang, range) = (asciiLowercase(lang), asciiLowercase(range))
        return lang == range || lang.hasPrefix(range + "-")
      }
      node = n.parentElement
    }
    return false
  case .checked:
    switch name {
    case "input":
      let type = asciiLowercase(element.getAttribute("type") ?? "")
      return element.hasAttribute("checked") && (type == "checkbox" || type == "radio")
    case "option": return element.hasAttribute("selected")
    default: return false
    }
  case .disabled: return formControls.contains(name) && element.hasAttribute("disabled")
  case .enabled: return formControls.contains(name) && !element.hasAttribute("disabled")
  case .never: return false
  }
}

/// The element's 1-based position among its siblings (from the end with
/// `last`), counting those of its type with `ofType`, or those that match
/// `of`.
private func nthPosition(_ element: DOMElement, last: Bool, ofType: Bool, of: [Complex]?) -> Int? {
  guard let parent = element.parentNode else { return nil }
  var counted = parent.children.filter { sibling in
    (!ofType || sibling.localName == element.localName)
      && (of.map { matchesList(sibling, $0) } ?? true)
  }
  if last { counted.reverse() }
  return counted.firstIndex { $0 === element }.map { $0 + 1 }
}

/// The elements a relative selector of `:has()` on `anchor` may match: its
/// descendants, or its following siblings and theirs.
private func hasCandidates(_ anchor: DOMElement, _ combinator: Combinator) -> [DOMElement] {
  func descendants(_ node: DOMNode, _ out: inout [DOMElement]) {
    for child in node.children {
      out.append(child)
      descendants(child, &out)
    }
  }
  var out: [DOMElement] = []
  switch combinator {
  case .descendant, .child: descendants(anchor, &out)
  case .nextSibling, .subsequentSibling:
    var sibling = anchor.nextElementSibling
    while let s = sibling {
      out.append(s)
      descendants(s, &out)
      sibling = s.nextElementSibling
    }
  }
  return out
}

/// Whether `element` stands in `combinator`'s relation to `anchor`.
private func isRelated(
  _ element: DOMElement, _ combinator: Combinator, _ anchor: DOMElement
)
  -> Bool
{
  switch combinator {
  case .child: return element.parentElement === anchor
  case .descendant:
    var ancestor = element.parentElement
    while let a = ancestor {
      if a === anchor { return true }
      ancestor = a.parentElement
    }
    return false
  case .nextSibling: return element.previousElementSibling === anchor
  case .subsequentSibling:
    var sibling = element.previousElementSibling
    while let s = sibling {
      if s === anchor { return true }
      sibling = s.previousElementSibling
    }
    return false
  }
}

/// `string` with A to Z in lower case.
private func asciiLowercase(_ string: String) -> String {
  String(
    String.UnicodeScalarView(
      string.unicodeScalars.map { ("A"..."Z").contains($0) ? Unicode.Scalar($0.value + 32)! : $0 }))
}

private func matchesCompound(_ element: DOMElement, _ compound: [Simple]) -> Bool {
  compound.allSatisfy { matchesSimple(element, $0) }
}

/// Whether `element` matches `complex`; with an anchor (a `:has()`
/// argument), its leftmost compound must stand in the anchor's relation.
private func matchesComplex(
  _ element: DOMElement, _ complex: Complex, anchor: (Combinator, DOMElement)? = nil
) -> Bool {
  matchesCompound(element, complex.subject) && matchesRest(element, complex.rest[...], anchor)
}

/// Whether `element`'s relatives match `rest` (combinators and the
/// compounds to their left, nearest first), the last of them related to
/// `anchor` when there is one.
private func matchesRest(
  _ element: DOMElement, _ rest: ArraySlice<(Combinator, [Simple])>,
  _ anchor: (Combinator, DOMElement)?
) -> Bool {
  guard let (combinator, compound) = rest.first else {
    return anchor.map { isRelated(element, $0.0, $0.1) } ?? true
  }
  let more = rest.dropFirst()
  func check(_ e: DOMElement) -> Bool {
    matchesCompound(e, compound) && matchesRest(e, more, anchor)
  }
  switch combinator {
  case .child: return element.parentElement.map(check) ?? false
  case .descendant:
    var ancestor = element.parentElement
    while let a = ancestor {
      if check(a) { return true }
      ancestor = a.parentElement
    }
    return false
  case .nextSibling: return element.previousElementSibling.map(check) ?? false
  case .subsequentSibling:
    var sibling = element.previousElementSibling
    while let s = sibling {
      if check(s) { return true }
      sibling = s.previousElementSibling
    }
    return false
  }
}

private func matchesList(_ element: DOMElement, _ list: [Complex]) -> Bool {
  list.contains { matchesComplex(element, $0) }
}

extension DOMNode {
  /// `querySelectorAll(selectors)`: the descendant elements that match, in
  /// document order.
  public func querySelectorAll(_ selectors: String) throws -> [DOMElement] {
    let list = try parseSelectorList(selectors)
    var out: [DOMElement] = []
    func walk(_ node: DOMNode) {
      for child in node.childNodes {
        guard let element = child as? DOMElement else { continue }
        if matchesList(element, list) { out.append(element) }
        walk(element)
      }
    }
    walk(self)
    return out
  }
}

extension DOMElement {
  /// `matches(selectors)`.
  public func matches(_ selectors: String) throws -> Bool {
    matchesList(self, try parseSelectorList(selectors))
  }
}

/// `querySelectorAll` and `matches` for the selectors a pasted style sheet
/// uses: compound selectors (a type or `*`, `#id`, `.class`, and attribute
/// selectors with every operator and the `i` and `s` flags) joined by the
/// descendant, child (`>`), next-sibling (`+`) and subsequent-sibling (`~`)
/// combinators, in comma lists.
///
/// Anything else (pseudo-classes and pseudo-elements, namespaces, escapes)
/// throws, as an invalid selector does in JS; some of it is valid in JS,
/// which `SelectorError.unsupported` tells apart. Type selectors and
/// attribute names match in any case, as in an HTML document; `#id` and
/// `.class` match as written.
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
      case ":": throw SelectorError.unsupported
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
      case nil, ",": return finish(compounds, combinators)
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
  }
}

private func matchesCompound(_ element: DOMElement, _ compound: [Simple]) -> Bool {
  compound.allSatisfy { matchesSimple(element, $0) }
}

/// Whether `element`'s relatives match `rest` (combinators and the
/// compounds to their left, nearest first).
private func matchesRest(_ element: DOMElement, _ rest: ArraySlice<(Combinator, [Simple])>) -> Bool
{
  guard let (combinator, compound) = rest.first else { return true }
  let more = rest.dropFirst()
  func check(_ e: DOMElement) -> Bool { matchesCompound(e, compound) && matchesRest(e, more) }
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
  list.contains { matchesCompound(element, $0.subject) && matchesRest(element, $0.rest[...]) }
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

// A pure-Swift HTML parser: the WHATWG tokenizer and the parts of the tree
// builder that HTML import meets (head, body, tables with foster
// parenting, lists, the adoption agency and active formatting elements).
// The `html/parse` vectors pin it against jsdom's `DOMParser`. Not covered:
// templates, framesets, `<select>` and `<ruby>` insertion modes (their
// contents parse as ordinary body content), and the case adjustments of SVG
// and MathML names.

/// `DOMParser`: parses an HTML string into a document.
public struct DOMParser {
  public init() {}

  /// `parseFromString(string, 'text/html')`.
  public func parseFromString(_ string: String, _ type: String = "text/html") -> DOMDocument {
    HTMLTreeBuilder(string).run()
  }
}

// MARK: - Tokenizer

struct HTMLAttribute {
  var name: String
  var value: String
}

enum HTMLToken {
  case doctype(name: String)
  case startTag(name: String, attributes: [HTMLAttribute], selfClosing: Bool)
  case endTag(name: String)
  case comment(String)
  case characters(String)
  case eof
}

enum HTMLTokenizerState { case data, rcdata, rawtext, plaintext }

private func isASCIIAlpha(_ c: Unicode.Scalar) -> Bool {
  ("a"..."z").contains(c) || ("A"..."Z").contains(c)
}

private func isASCIIAlnum(_ c: Unicode.Scalar) -> Bool {
  isASCIIAlpha(c) || ("0"..."9").contains(c)
}

/// Tab, LF, FF, CR and space.
private func isHTMLWhitespace(_ c: Unicode.Scalar) -> Bool {
  c == " " || c == "\t" || c == "\n" || c == "\u{0C}" || c == "\r"
}

private func lowerASCII(_ c: Unicode.Scalar) -> Unicode.Scalar {
  ("A"..."Z").contains(c) ? Unicode.Scalar(c.value + 32)! : c
}

/// Code points 0x80–0x9F of a numeric reference, read as windows-1252.
private let windows1252: [UInt32: UInt32] = [
  0x80: 0x20AC, 0x82: 0x201A, 0x83: 0x0192, 0x84: 0x201E, 0x85: 0x2026, 0x86: 0x2020,
  0x87: 0x2021, 0x88: 0x02C6, 0x89: 0x2030, 0x8A: 0x0160, 0x8B: 0x2039, 0x8C: 0x0152,
  0x8E: 0x017D, 0x91: 0x2018, 0x92: 0x2019, 0x93: 0x201C, 0x94: 0x201D, 0x95: 0x2022,
  0x96: 0x2013, 0x97: 0x2014, 0x98: 0x02DC, 0x99: 0x2122, 0x9A: 0x0161, 0x9B: 0x203A,
  0x9C: 0x0153, 0x9E: 0x017E, 0x9F: 0x0178,
]

struct HTMLTokenizer {
  private let input: [Unicode.Scalar]
  private var pos = 0
  var state = HTMLTokenizerState.data
  /// The name of the element whose raw text is being read.
  var lastStartTag = ""

  init(_ html: String) {
    // Newlines are normalized to LF before tokenizing.
    var scalars: [Unicode.Scalar] = []
    scalars.reserveCapacity(html.unicodeScalars.count)
    var previousCR = false
    for scalar in html.unicodeScalars {
      if scalar == "\r" {
        scalars.append("\n")
        previousCR = true
        continue
      }
      if scalar == "\n" && previousCR {
        previousCR = false
        continue
      }
      previousCR = false
      scalars.append(scalar)
    }
    input = scalars
  }

  private func peek(_ offset: Int = 0) -> Unicode.Scalar? {
    pos + offset < input.count ? input[pos + offset] : nil
  }

  private func string(_ range: Range<Int>) -> String {
    var view = String.UnicodeScalarView()
    view.append(contentsOf: input[range])
    return String(view)
  }

  mutating func next() -> HTMLToken {
    while pos < input.count {
      switch state {
      case .plaintext:
        let text = string(pos..<input.count)
        pos = input.count
        return .characters(text)
      case .rawtext, .rcdata:
        if let token = nextRawText(decode: state == .rcdata) { return token }
      case .data:
        if isTagStart(pos) {
          if let token = tagOpen() { return token }
        } else {
          return .characters(dataText())
        }
      }
    }
    return .eof
  }

  /// Whether a tag, comment or declaration starts at `i` (a `<` that is
  /// not literal text).
  private func isTagStart(_ i: Int) -> Bool {
    guard input[i] == "<", i + 1 < input.count else { return false }
    let c = input[i + 1]
    if isASCIIAlpha(c) || c == "!" || c == "?" { return true }
    return c == "/" && i + 2 < input.count
  }

  private mutating func dataText() -> String {
    var text = String.UnicodeScalarView()
    while pos < input.count {
      let c = input[pos]
      if c == "<" && isTagStart(pos) { break }
      if c == "&" {
        consumeCharacterReference(into: &text, inAttribute: false)
        continue
      }
      if c != "\0" { text.append(c) }
      pos += 1
    }
    return String(text)
  }

  /// At `<`: the tag, comment or declaration, or `nil` for one that is
  /// dropped (`</>`, a tag cut off by the end of input).
  private mutating func tagOpen() -> HTMLToken? {
    pos += 1
    let c = input[pos]
    if c == "!" {
      pos += 1
      return markupDeclaration()
    }
    if c == "?" { return bogusComment() }
    if c == "/" {
      pos += 1
      guard let d = peek() else { return nil }
      if d == ">" {
        pos += 1
        return nil
      }
      if isASCIIAlpha(d) {
        guard let (name, _, _) = tag() else { return nil }
        return .endTag(name: name)
      }
      return bogusComment()
    }
    guard let (name, attributes, selfClosing) = tag() else { return nil }
    return .startTag(name: name, attributes: attributes, selfClosing: selfClosing)
  }

  /// Text up to the next `>` as a comment.
  private mutating func bogusComment() -> HTMLToken {
    let start = pos
    while let c = peek(), c != ">" { pos += 1 }
    var view = String.UnicodeScalarView()
    view.append(contentsOf: input[start..<pos].map { $0 == "\0" ? "\u{FFFD}" : $0 })
    let data = String(view)
    if pos < input.count { pos += 1 }
    return .comment(data)
  }

  private func matchesCaseInsensitive(_ word: String) -> Bool {
    var i = pos
    for expected in word.unicodeScalars {
      guard i < input.count, lowerASCII(input[i]) == lowerASCII(expected) else { return false }
      i += 1
    }
    return true
  }

  /// After `<!`.
  private mutating func markupDeclaration() -> HTMLToken {
    if peek() == "-" && peek(1) == "-" {
      pos += 2
      return comment()
    }
    if matchesCaseInsensitive("doctype") {
      pos += 7
      while let c = peek(), isHTMLWhitespace(c) { pos += 1 }
      var name = String.UnicodeScalarView()
      while let c = peek(), !isHTMLWhitespace(c), c != ">" {
        name.append(lowerASCII(c))
        pos += 1
      }
      while let c = peek(), c != ">" { pos += 1 }
      if pos < input.count { pos += 1 }
      return .doctype(name: String(name))
    }
    return bogusComment()
  }

  /// After `<!--`.
  private mutating func comment() -> HTMLToken {
    if peek() == ">" {
      pos += 1
      return .comment("")
    }
    if peek() == "-" && peek(1) == ">" {
      pos += 2
      return .comment("")
    }
    let start = pos
    while pos < input.count {
      if input[pos] == "-" && peek(1) == "-" {
        if peek(2) == ">" {
          let data = string(start..<pos)
          pos += 3
          return .comment(data)
        }
        if peek(2) == "!" && peek(3) == ">" {
          let data = string(start..<pos)
          pos += 4
          return .comment(data)
        }
      }
      pos += 1
    }
    return .comment(string(start..<input.count))
  }

  /// A tag's name and attributes, from its first name character through
  /// `>`; `nil` when the input ends first.
  private mutating func tag() -> (String, [HTMLAttribute], Bool)? {
    var name = String.UnicodeScalarView()
    while let c = peek(), !isHTMLWhitespace(c), c != "/", c != ">" {
      name.append(c == "\0" ? "\u{FFFD}" : lowerASCII(c))
      pos += 1
    }
    var attributes: [HTMLAttribute] = []
    var selfClosing = false
    while let c = peek() {
      if isHTMLWhitespace(c) {
        pos += 1
        continue
      }
      if c == ">" {
        pos += 1
        return (String(name), attributes, selfClosing)
      }
      if c == "/" {
        pos += 1
        if peek() == ">" { selfClosing = true }
        continue
      }
      // An attribute: its first character may be `=`.
      var attributeName = String.UnicodeScalarView()
      attributeName.append(lowerASCII(c))
      pos += 1
      while let d = peek(), !isHTMLWhitespace(d), d != "/", d != ">", d != "=" {
        attributeName.append(d == "\0" ? "\u{FFFD}" : lowerASCII(d))
        pos += 1
      }
      while let d = peek(), isHTMLWhitespace(d) { pos += 1 }
      var value = String.UnicodeScalarView()
      if peek() == "=" {
        pos += 1
        while let d = peek(), isHTMLWhitespace(d) { pos += 1 }
        if let quote = peek(), quote == "\"" || quote == "'" {
          pos += 1
          while let d = peek(), d != quote {
            if d == "&" {
              consumeCharacterReference(into: &value, inAttribute: true)
            } else {
              value.append(d == "\0" ? "\u{FFFD}" : d)
              pos += 1
            }
          }
          if pos < input.count { pos += 1 }
        } else {
          while let d = peek(), !isHTMLWhitespace(d), d != ">" {
            if d == "&" {
              consumeCharacterReference(into: &value, inAttribute: true)
            } else {
              value.append(d == "\0" ? "\u{FFFD}" : d)
              pos += 1
            }
          }
        }
      }
      let attribute = HTMLAttribute(name: String(attributeName), value: String(value))
      // A repeated attribute is dropped.
      if !attributes.contains(where: { $0.name == attribute.name }) {
        attributes.append(attribute)
      }
    }
    return nil
  }

  /// At `&`: appends what the character reference decodes to, or `&`.
  private mutating func consumeCharacterReference(
    into out: inout String.UnicodeScalarView, inAttribute: Bool
  ) {
    pos += 1
    guard let c = peek() else {
      out.append("&")
      return
    }
    if c == "#" {
      var i = pos + 1
      var hex = false
      if i < input.count, input[i] == "x" || input[i] == "X" {
        hex = true
        i += 1
      }
      let digitsStart = i
      var value: UInt32 = 0
      while i < input.count, let digit = hexDigit(input[i], hex: hex) {
        value = min(value &* (hex ? 16 : 10) &+ digit, 0x110000)
        i += 1
      }
      guard i > digitsStart else {
        out.append("&")
        return
      }
      if i < input.count, input[i] == ";" { i += 1 }
      pos = i
      out.append(numericReference(value))
      return
    }
    guard isASCIIAlnum(c) else {
      out.append("&")
      return
    }
    var end = pos
    while end < input.count, isASCIIAlnum(input[end]), end - pos < 40 { end += 1 }
    let run = string(pos..<end)
    if end < input.count, input[end] == ";", let value = htmlNamedEntities[run] {
      pos = end + 1
      out.append(contentsOf: value.unicodeScalars)
      return
    }
    // The longest name that may go without its semicolon.
    for length in stride(from: min(end - pos, 6), through: 2, by: -1) {
      let name = string(pos..<(pos + length))
      guard htmlLegacyEntityNames.contains(name), let value = htmlNamedEntities[name] else {
        continue
      }
      if inAttribute, pos + length < input.count {
        let following = input[pos + length]
        if following == "=" || isASCIIAlnum(following) {
          out.append("&")
          return
        }
      }
      pos += length
      out.append(contentsOf: value.unicodeScalars)
      return
    }
    out.append("&")
  }

  private func hexDigit(_ c: Unicode.Scalar, hex: Bool) -> UInt32? {
    if ("0"..."9").contains(c) { return c.value - 48 }
    guard hex else { return nil }
    if ("a"..."f").contains(c) { return c.value - 87 }
    if ("A"..."F").contains(c) { return c.value - 55 }
    return nil
  }

  private func numericReference(_ value: UInt32) -> Unicode.Scalar {
    if let mapped = windows1252[value] { return Unicode.Scalar(mapped)! }
    if value == 0 || value > 0x10FFFF || (0xD800...0xDFFF).contains(value) { return "\u{FFFD}" }
    return Unicode.Scalar(value) ?? "\u{FFFD}"
  }

  /// Whether `</name` of the raw-text element starts at `i`.
  private func isAppropriateEndTag(_ i: Int) -> Bool {
    guard input[i] == "<", i + 1 < input.count, input[i + 1] == "/" else { return false }
    var j = i + 2
    for expected in lastStartTag.unicodeScalars {
      guard j < input.count, lowerASCII(input[j]) == expected else { return false }
      j += 1
    }
    guard j < input.count else { return false }
    return isHTMLWhitespace(input[j]) || input[j] == "/" || input[j] == ">"
  }

  /// Raw text or RCDATA up to the end tag of its element, or that end tag.
  private mutating func nextRawText(decode: Bool) -> HTMLToken? {
    if isAppropriateEndTag(pos) {
      pos += 2
      state = .data
      guard let (name, _, _) = tag() else { return nil }
      return .endTag(name: name)
    }
    var text = String.UnicodeScalarView()
    while pos < input.count {
      if input[pos] == "<" && isAppropriateEndTag(pos) { break }
      if decode && input[pos] == "&" {
        consumeCharacterReference(into: &text, inAttribute: false)
        continue
      }
      text.append(input[pos] == "\0" ? "\u{FFFD}" : input[pos])
      pos += 1
    }
    return .characters(String(text))
  }
}

// MARK: - Tree builder

private let specialElements: Set<String> = [
  "address", "applet", "area", "article", "aside", "base", "basefont", "bgsound", "blockquote",
  "body", "br", "button", "caption", "center", "col", "colgroup", "dd", "details", "dir", "div",
  "dl", "dt", "embed", "fieldset", "figcaption", "figure", "footer", "form", "frame", "frameset",
  "h1", "h2", "h3", "h4", "h5", "h6", "head", "header", "hgroup", "hr", "html", "iframe", "img",
  "input", "keygen", "li", "link", "listing", "main", "marquee", "menu", "meta", "nav", "noembed",
  "noframes", "noscript", "object", "ol", "p", "param", "plaintext", "pre", "script", "search",
  "section", "select", "source", "style", "summary", "table", "tbody", "td", "template",
  "textarea", "tfoot", "th", "thead", "title", "tr", "track", "ul", "wbr", "xmp",
]

private let scopeBarriers: Set<String> = [
  "applet", "caption", "html", "table", "td", "th", "marquee", "object", "template",
]

private let impliedEndTags: Set<String> = [
  "dd", "dt", "li", "optgroup", "option", "p", "rb", "rp", "rt", "rtc",
]

private let headings: Set<String> = ["h1", "h2", "h3", "h4", "h5", "h6"]

private let formattingTags: Set<String> = [
  "b", "big", "code", "em", "font", "i", "s", "small", "strike", "strong", "tt", "u",
]

private let closesParagraph: Set<String> = [
  "address", "article", "aside", "blockquote", "center", "details", "dialog", "dir", "div", "dl",
  "fieldset", "figcaption", "figure", "footer", "header", "hgroup", "main", "menu", "nav", "ol",
  "p", "search", "section", "summary", "ul",
]

private let blockEndTags: Set<String> = closesParagraph.union(["button", "listing", "pre"])
  .subtracting(["p"])

/// Start tags that leave foreign (SVG, MathML) content.
private let foreignBreakout: Set<String> = [
  "b", "big", "blockquote", "body", "br", "center", "code", "dd", "div", "dl", "dt", "em",
  "embed", "h1", "h2", "h3", "h4", "h5", "h6", "head", "hr", "i", "img", "li", "listing", "menu",
  "meta", "nobr", "ol", "p", "pre", "ruby", "s", "small", "span", "strong", "strike", "sub", "sup",
  "table", "tt", "u", "ul", "var",
]

private let tableSections: Set<String> = ["tbody", "tfoot", "thead"]

final class HTMLTreeBuilder {
  enum Mode {
    case initial, beforeHTML, beforeHead, inHead, inHeadNoscript, afterHead, inBody, text
    case inTable, inCaption, inColumnGroup, inTableBody, inRow, inCell, afterBody, afterAfterBody
  }

  private var tokenizer: HTMLTokenizer
  private let document = DOMDocument()
  private var mode = Mode.initial
  private var originalMode = Mode.inBody
  private var stack: [DOMElement] = []
  /// The list of active formatting elements; `nil` is a marker.
  private var formatting: [DOMElement?] = []
  private var head: DOMElement?
  private var fosterParenting = false
  private var skipNextNewline = false
  private var foreign = Set<ObjectIdentifier>()
  private var stopped = false
  /// No `<!DOCTYPE html>`: the quirks mode of a document without one.
  private var quirks = true

  init(_ html: String) { tokenizer = HTMLTokenizer(html) }

  func run() -> DOMDocument {
    while !stopped {
      var token = tokenizer.next()
      if skipNextNewline {
        skipNextNewline = false
        if case .characters(let text) = token, text.unicodeScalars.first == "\n" {
          let rest = String(text.unicodeScalars.dropFirst())
          if rest.isEmpty { continue }
          token = .characters(rest)
        }
      }
      dispatch(token)
      if case .eof = token { break }
    }
    return document
  }

  private var currentNode: DOMElement { stack[stack.count - 1] }

  private func isForeign(_ element: DOMElement) -> Bool {
    foreign.contains(ObjectIdentifier(element))
  }

  private func dispatch(_ token: HTMLToken) {
    if let current = stack.last, isForeign(current) {
      switch token {
      case .characters(let text):
        insertCharacters(text)
        return
      case .comment(let data):
        insertComment(data)
        return
      case .startTag(let name, let attributes, let selfClosing):
        let isFontBreakout =
          name == "font"
          && attributes.contains { ["color", "face", "size"].contains($0.name) }
        if foreignBreakout.contains(name) || isFontBreakout {
          while let node = stack.last, isForeign(node) { stack.removeLast() }
          process(token)
        } else {
          insertForeign(name, attributes, selfClosing)
        }
        return
      case .endTag(let name):
        for index in stride(from: stack.count - 1, through: 0, by: -1) {
          let node = stack[index]
          if !isForeign(node) { break }
          if node.localName == name {
            stack.removeSubrange(index...)
            return
          }
        }
      default: break
      }
    }
    process(token)
  }

  private func process(_ token: HTMLToken) {
    switch mode {
    case .initial: initial(token)
    case .beforeHTML: beforeHTML(token)
    case .beforeHead: beforeHead(token)
    case .inHead: inHead(token)
    case .inHeadNoscript: inHeadNoscript(token)
    case .afterHead: afterHead(token)
    case .inBody: inBody(token)
    case .text: text(token)
    case .inTable: inTable(token)
    case .inCaption: inCaption(token)
    case .inColumnGroup: inColumnGroup(token)
    case .inTableBody: inTableBody(token)
    case .inRow: inRow(token)
    case .inCell: inCell(token)
    case .afterBody: afterBody(token)
    case .afterAfterBody: afterAfterBody(token)
    }
  }

  /// Splits leading whitespace off a character token: handles it with
  /// `whitespace` and returns the rest, if any, as a token.
  private func splitWhitespace(_ token: HTMLToken, _ whitespace: (String) -> Void) -> HTMLToken? {
    guard case .characters(let text) = token else { return token }
    let scalars = text.unicodeScalars
    let index = scalars.firstIndex { !isHTMLWhitespace($0) } ?? scalars.endIndex
    let leading = String(scalars[..<index])
    if !leading.isEmpty { whitespace(leading) }
    let rest = String(scalars[index...])
    return rest.isEmpty ? nil : .characters(rest)
  }

  // MARK: Insertion

  private func createElement(_ name: String, _ attributes: [HTMLAttribute]) -> DOMElement {
    let element = DOMElement(name, document)
    for attribute in attributes { element.setAttribute(attribute.name, attribute.value) }
    return element
  }

  private func attributes(of element: DOMElement) -> [HTMLAttribute] {
    element.attributes.map { HTMLAttribute(name: $0.name, value: $0.value) }
  }

  private func insertionPlace(_ override: DOMElement? = nil) -> (DOMNode, DOMNode?) {
    let target = override ?? currentNode
    if fosterParenting, ["table", "tbody", "tfoot", "thead", "tr"].contains(target.localName) {
      if let tableIndex = stack.lastIndex(where: { $0.localName == "table" }) {
        let table = stack[tableIndex]
        if let parent = table.parentNode { return (parent, table) }
        return (stack[tableIndex - 1], nil)
      }
      return (stack[0], nil)
    }
    return (target, nil)
  }

  @discardableResult
  private func insertElement(_ name: String, _ attributes: [HTMLAttribute] = []) -> DOMElement {
    let element = createElement(name, attributes)
    let (parent, before) = insertionPlace()
    parent.insertBefore(element, before)
    stack.append(element)
    return element
  }

  private func insertVoid(_ name: String, _ attributes: [HTMLAttribute]) {
    insertElement(name, attributes)
    stack.removeLast()
  }

  private func insertForeign(_ name: String, _ attributes: [HTMLAttribute], _ selfClosing: Bool) {
    let element = insertElement(name, attributes)
    foreign.insert(ObjectIdentifier(element))
    if selfClosing { stack.removeLast() }
  }

  private func insertCharacters(_ text: String) {
    var view = text.unicodeScalars
    view.removeAll { $0 == "\0" }
    let text = String(view)
    guard !text.isEmpty else { return }
    let (parent, before) = insertionPlace()
    if parent is DOMDocument { return }
    let previous = before.map(\.previousSibling) ?? parent.lastChild
    if let previous = previous as? DOMText {
      previous.data += text
    } else {
      parent.insertBefore(DOMText(text, document), before)
    }
  }

  private func insertComment(_ data: String, into parent: DOMNode? = nil) {
    let comment = DOMComment(data, document)
    if let parent {
      parent.appendChild(comment)
    } else {
      let (parent, before) = insertionPlace()
      parent.insertBefore(comment, before)
    }
  }

  private func startRawText(
    _ name: String, _ attributes: [HTMLAttribute], _ state: HTMLTokenizerState
  ) {
    insertElement(name, attributes)
    tokenizer.state = state
    tokenizer.lastStartTag = name
    originalMode = mode
    mode = .text
  }

  // MARK: Stack and scope

  private func inScope(_ names: Set<String>, _ extra: Set<String> = []) -> Bool {
    for element in stack.reversed() {
      if names.contains(element.localName) { return true }
      if scopeBarriers.contains(element.localName) || extra.contains(element.localName)
        || isForeign(element)
      {
        return false
      }
    }
    return false
  }

  private func inButtonScope(_ name: String) -> Bool { inScope([name], ["button"]) }

  private func inTableScope(_ names: Set<String>) -> Bool {
    for element in stack.reversed() {
      if names.contains(element.localName) { return true }
      if ["html", "table", "template"].contains(element.localName) { return false }
    }
    return false
  }

  private func elementInScope(_ target: DOMElement) -> Bool {
    for element in stack.reversed() {
      if element === target { return true }
      if scopeBarriers.contains(element.localName) { return false }
    }
    return false
  }

  private func generateImpliedEndTags(except: String? = nil) {
    while let node = stack.last, impliedEndTags.contains(node.localName),
      node.localName != except
    {
      stack.removeLast()
    }
  }

  private func popUntil(_ names: Set<String>) {
    while let node = stack.popLast() {
      if names.contains(node.localName) { return }
    }
  }

  private func closeParagraph() {
    guard inButtonScope("p") else { return }
    generateImpliedEndTags(except: "p")
    popUntil(["p"])
  }

  private func clearStack(backTo names: Set<String>) {
    while let node = stack.last, !names.contains(node.localName) { stack.removeLast() }
  }

  private func resetInsertionMode() {
    for index in stride(from: stack.count - 1, through: 0, by: -1) {
      let last = index == 0
      switch stack[index].localName {
      case "td" where !last, "th" where !last:
        mode = .inCell
      case "tr": mode = .inRow
      case "tbody", "thead", "tfoot": mode = .inTableBody
      case "caption": mode = .inCaption
      case "colgroup": mode = .inColumnGroup
      case "table": mode = .inTable
      case "head" where !last: mode = .inHead
      case "body": mode = .inBody
      case "html": mode = head == nil ? .beforeHead : .afterHead
      default:
        if last { mode = .inBody }
        continue
      }
      return
    }
    mode = .inBody
  }

  // MARK: Active formatting elements

  private func pushFormatting(_ element: DOMElement) {
    // At most three matching entries after the last marker.
    var matches: [Int] = []
    for index in stride(from: formatting.count - 1, through: 0, by: -1) {
      guard let entry = formatting[index] else { break }
      if entry.localName == element.localName
        && entry.attributes.count == element.attributes.count
        && zip(entry.attributes, element.attributes).allSatisfy({
          $0.name == $1.name && $0.value == $1.value
        })
      {
        matches.append(index)
      }
    }
    if matches.count >= 3, let earliest = matches.last { formatting.remove(at: earliest) }
    formatting.append(element)
  }

  private func isOpen(_ element: DOMElement) -> Bool { stack.contains { $0 === element } }

  private func reconstructFormatting() {
    guard let last = formatting.last, let lastElement = last, !isOpen(lastElement) else { return }
    var index = formatting.count - 1
    while index > 0, let entry = formatting[index - 1], !isOpen(entry) { index -= 1 }
    for entryIndex in index..<formatting.count {
      let entry = formatting[entryIndex]!
      formatting[entryIndex] = insertElement(entry.localName, attributes(of: entry))
    }
  }

  private func clearFormattingToMarker() {
    while let entry = formatting.popLast() {
      if entry == nil { return }
    }
  }

  private func formattingElementAfterMarker(_ name: String) -> Int? {
    for index in stride(from: formatting.count - 1, through: 0, by: -1) {
      guard let entry = formatting[index] else { return nil }
      if entry.localName == name { return index }
    }
    return nil
  }

  /// The adoption agency algorithm for an end tag `subject`; `false` when
  /// the tag should be handled as any other end tag.
  private func adoptionAgency(_ subject: String) -> Bool {
    let current = currentNode
    if current.localName == subject && !formatting.contains(where: { $0 === current }) {
      stack.removeLast()
      return true
    }
    for _ in 0..<8 {
      guard let formattingIndex = formattingElementAfterMarker(subject) else { return false }
      let formattingElement = formatting[formattingIndex]!
      guard let stackIndex = stack.firstIndex(where: { $0 === formattingElement }) else {
        formatting.remove(at: formattingIndex)
        return true
      }
      guard elementInScope(formattingElement) else { return true }
      guard
        let furthestIndex = stack.indices.first(where: {
          $0 > stackIndex && specialElements.contains(stack[$0].localName)
        })
      else {
        stack.removeSubrange(stackIndex...)
        formatting.remove(at: formattingIndex)
        return true
      }
      let furthestBlock = stack[furthestIndex]
      let commonAncestor = stack[stackIndex - 1]
      var bookmark = formattingIndex
      var lastNode = furthestBlock
      var nodeIndex = furthestIndex
      var innerLoop = 0
      while true {
        innerLoop += 1
        nodeIndex -= 1
        var node = stack[nodeIndex]
        if node === formattingElement { break }
        var nodeFormattingIndex = formatting.firstIndex { $0 === node }
        if innerLoop > 3, let index = nodeFormattingIndex {
          formatting.remove(at: index)
          if index < bookmark { bookmark -= 1 }
          nodeFormattingIndex = nil
        }
        guard let index = nodeFormattingIndex else {
          stack.remove(at: nodeIndex)
          continue
        }
        let replacement = createElement(node.localName, attributes(of: node))
        formatting[index] = replacement
        stack[nodeIndex] = replacement
        node = replacement
        if lastNode === furthestBlock { bookmark = index + 1 }
        node.appendChild(lastNode)
        lastNode = node
      }
      let (parent, before) = insertionPlace(commonAncestor)
      parent.insertBefore(lastNode, before)
      let newElement = createElement(formattingElement.localName, attributes(of: formattingElement))
      for child in furthestBlock.childNodes { newElement.appendChild(child) }
      furthestBlock.appendChild(newElement)
      if let index = formatting.firstIndex(where: { $0 === formattingElement }) {
        formatting.remove(at: index)
        if index < bookmark { bookmark -= 1 }
      }
      formatting.insert(newElement, at: min(bookmark, formatting.count))
      stack.removeAll { $0 === formattingElement }
      if let index = stack.firstIndex(where: { $0 === furthestBlock }) {
        stack.insert(newElement, at: index + 1)
      }
    }
    return true
  }

  // MARK: Insertion modes

  private func initial(_ token: HTMLToken) {
    switch token {
    case .comment(let data): insertComment(data, into: document)
    case .doctype(let name):
      quirks = name != "html"
      mode = .beforeHTML
    default:
      guard let rest = splitWhitespace(token, { _ in }) else { return }
      mode = .beforeHTML
      process(rest)
    }
  }

  private func beforeHTML(_ token: HTMLToken) {
    switch token {
    case .doctype: return
    case .comment(let data): insertComment(data, into: document)
    case .startTag("html", let attributes, _):
      let html = createElement("html", attributes)
      document.appendChild(html)
      stack.append(html)
      mode = .beforeHead
    case .endTag(let name) where !["head", "body", "html", "br"].contains(name):
      return
    default:
      guard let rest = splitWhitespace(token, { _ in }) else { return }
      let html = createElement("html", [])
      document.appendChild(html)
      stack.append(html)
      mode = .beforeHead
      process(rest)
    }
  }

  private func beforeHead(_ token: HTMLToken) {
    switch token {
    case .doctype: return
    case .comment(let data): insertComment(data)
    case .startTag("html", _, _): inBody(token)
    case .startTag("head", let attributes, _):
      head = insertElement("head", attributes)
      mode = .inHead
    case .endTag(let name) where !["head", "body", "html", "br"].contains(name):
      return
    default:
      guard let rest = splitWhitespace(token, { _ in }) else { return }
      head = insertElement("head")
      mode = .inHead
      process(rest)
    }
  }

  /// The start tags the head handles, wherever they appear.
  private func headStartTag(_ name: String, _ attributes: [HTMLAttribute]) -> Bool {
    switch name {
    case "base", "basefont", "bgsound", "link", "meta":
      insertVoid(name, attributes)
    case "title":
      startRawText(name, attributes, .rcdata)
    case "noframes", "style", "script":
      startRawText(name, attributes, .rawtext)
    default:
      return false
    }
    return true
  }

  private func inHead(_ token: HTMLToken) {
    switch token {
    case .doctype: return
    case .comment(let data): insertComment(data)
    case .startTag("html", _, _): inBody(token)
    case .startTag("noscript", let attributes, _):
      insertElement("noscript", attributes)
      mode = .inHeadNoscript
    case .startTag(let name, let attributes, _) where headStartTag(name, attributes):
      return
    case .startTag("head", _, _): return
    case .endTag("head"):
      stack.removeLast()
      mode = .afterHead
    case .endTag(let name) where !["body", "html", "br"].contains(name):
      return
    default:
      guard let rest = splitWhitespace(token, { self.insertCharacters($0) }) else { return }
      stack.removeLast()
      mode = .afterHead
      process(rest)
    }
  }

  private func inHeadNoscript(_ token: HTMLToken) {
    switch token {
    case .doctype: return
    case .startTag("html", _, _): inBody(token)
    case .endTag("noscript"):
      stack.removeLast()
      mode = .inHead
    case .comment, .startTag("basefont", _, _), .startTag("bgsound", _, _),
      .startTag("link", _, _), .startTag("meta", _, _), .startTag("noframes", _, _),
      .startTag("style", _, _):
      inHead(token)
    case .startTag("head", _, _), .startTag("noscript", _, _): return
    case .endTag(let name) where name != "br": return
    default:
      guard let rest = splitWhitespace(token, { self.insertCharacters($0) }) else { return }
      stack.removeLast()
      mode = .inHead
      process(rest)
    }
  }

  private func afterHead(_ token: HTMLToken) {
    switch token {
    case .doctype: return
    case .comment(let data): insertComment(data)
    case .startTag("html", _, _): inBody(token)
    case .startTag("body", let attributes, _):
      insertElement("body", attributes)
      mode = .inBody
    case .startTag(let name, let attributes, _)
    where ["base", "basefont", "bgsound", "link", "meta", "noframes", "script", "style", "title"]
      .contains(name):
      if let head { stack.append(head) }
      _ = headStartTag(name, attributes)
      if let head { stack.removeAll { $0 === head } }
    case .startTag("head", _, _): return
    case .endTag(let name) where !["body", "html", "br"].contains(name):
      return
    default:
      guard let rest = splitWhitespace(token, { self.insertCharacters($0) }) else { return }
      insertElement("body")
      mode = .inBody
      process(rest)
    }
  }

  private func text(_ token: HTMLToken) {
    switch token {
    case .characters(let text): insertCharacters(text)
    case .eof:
      stack.removeLast()
      mode = originalMode
      process(token)
    case .endTag:
      stack.removeLast()
      mode = originalMode
    default: return
    }
  }

  private func mergeAttributes(_ attributes: [HTMLAttribute], into element: DOMElement) {
    for attribute in attributes where !element.hasAttribute(attribute.name) {
      element.setAttribute(attribute.name, attribute.value)
    }
  }

  private func inBody(_ token: HTMLToken) {
    switch token {
    case .characters(let text):
      reconstructFormatting()
      insertCharacters(text)
    case .comment(let data): insertComment(data)
    case .doctype: return
    case .eof: stopped = true
    case .startTag(let name, let attributes, let selfClosing):
      inBodyStartTag(name, attributes, selfClosing)
    case .endTag(let name):
      inBodyEndTag(name)
    }
  }

  private func inBodyStartTag(_ name: String, _ attributes: [HTMLAttribute], _ selfClosing: Bool) {
    switch name {
    case "html":
      mergeAttributes(attributes, into: stack[0])
    case "base", "basefont", "bgsound", "link", "meta", "noframes", "script", "style", "title":
      _ = headStartTag(name, attributes)
    case "body":
      if stack.count > 1, stack[1].localName == "body" {
        mergeAttributes(attributes, into: stack[1])
      }
    case "frameset", "template":
      if name == "template" {
        insertElement(name, attributes)
        formatting.append(nil)
      }
    case _ where closesParagraph.contains(name):
      closeParagraph()
      insertElement(name, attributes)
    case _ where headings.contains(name):
      closeParagraph()
      if headings.contains(currentNode.localName) { stack.removeLast() }
      insertElement(name, attributes)
    case "pre", "listing":
      closeParagraph()
      insertElement(name, attributes)
      skipNextNewline = true
    case "form":
      closeParagraph()
      insertElement(name, attributes)
    case "li", "dd", "dt":
      let closes: Set<String> = name == "li" ? ["li"] : ["dd", "dt"]
      for node in stack.reversed() {
        if closes.contains(node.localName) {
          generateImpliedEndTags(except: node.localName)
          popUntil([node.localName])
          break
        }
        if specialElements.contains(node.localName)
          && !["address", "div", "p"].contains(node.localName)
        {
          break
        }
      }
      closeParagraph()
      insertElement(name, attributes)
    case "plaintext":
      closeParagraph()
      insertElement(name, attributes)
      tokenizer.state = .plaintext
    case "button":
      if inScope(["button"]) {
        generateImpliedEndTags()
        popUntil(["button"])
      }
      reconstructFormatting()
      insertElement(name, attributes)
    case "a":
      if let index = formattingElementAfterMarker("a") {
        let element = formatting[index]!
        _ = adoptionAgency("a")
        formatting.removeAll { $0 === element }
        stack.removeAll { $0 === element }
      }
      reconstructFormatting()
      pushFormatting(insertElement(name, attributes))
    case _ where formattingTags.contains(name):
      reconstructFormatting()
      pushFormatting(insertElement(name, attributes))
    case "nobr":
      reconstructFormatting()
      if inScope(["nobr"]) {
        _ = adoptionAgency("nobr")
        reconstructFormatting()
      }
      pushFormatting(insertElement(name, attributes))
    case "applet", "marquee", "object":
      reconstructFormatting()
      insertElement(name, attributes)
      formatting.append(nil)
    case "table":
      // In quirks mode a table stays inside an open paragraph.
      if !quirks { closeParagraph() }
      insertElement(name, attributes)
      mode = .inTable
    case "area", "br", "embed", "img", "image", "keygen", "wbr", "input":
      reconstructFormatting()
      insertVoid(name == "image" ? "img" : name, attributes)
    case "param", "source", "track":
      insertVoid(name, attributes)
    case "hr":
      closeParagraph()
      insertVoid(name, attributes)
    case "textarea":
      startRawText(name, attributes, .rcdata)
      skipNextNewline = true
    case "xmp":
      closeParagraph()
      reconstructFormatting()
      startRawText(name, attributes, .rawtext)
    case "iframe", "noembed":
      startRawText(name, attributes, .rawtext)
    case "optgroup", "option":
      if currentNode.localName == "option" { stack.removeLast() }
      reconstructFormatting()
      insertElement(name, attributes)
    case "math", "svg":
      reconstructFormatting()
      insertForeign(name, attributes, selfClosing)
    case "caption", "col", "colgroup", "frame", "head", "tbody", "td", "tfoot", "th", "thead",
      "tr":
      return
    default:
      reconstructFormatting()
      insertElement(name, attributes)
    }
  }

  private func inBodyEndTag(_ name: String) {
    switch name {
    case "body", "html":
      guard inScope(["body"]) else { return }
      mode = .afterBody
      if name == "html" { process(.endTag(name: name)) }
    case _ where blockEndTags.contains(name) || name == "form" || name == "template":
      guard inScope([name]) else { return }
      generateImpliedEndTags()
      popUntil([name])
      if name == "template" { clearFormattingToMarker() }
    case "p":
      if !inButtonScope("p") { insertElement("p") }
      closeParagraph()
    case "li":
      guard inScope(["li"], ["ol", "ul"]) else { return }
      generateImpliedEndTags(except: "li")
      popUntil(["li"])
    case "dd", "dt":
      guard inScope([name]) else { return }
      generateImpliedEndTags(except: name)
      popUntil([name])
    case _ where headings.contains(name):
      guard inScope(headings) else { return }
      generateImpliedEndTags()
      popUntil(headings)
    case "a", "nobr",
      _ where formattingTags.contains(name):
      if !adoptionAgency(name) { anyOtherEndTag(name) }
    case "applet", "marquee", "object":
      guard inScope([name]) else { return }
      generateImpliedEndTags()
      popUntil([name])
      clearFormattingToMarker()
    case "br":
      reconstructFormatting()
      insertVoid("br", [])
    default:
      anyOtherEndTag(name)
    }
  }

  private func anyOtherEndTag(_ name: String) {
    for index in stride(from: stack.count - 1, through: 0, by: -1) {
      let node = stack[index]
      if node.localName == name {
        generateImpliedEndTags(except: name)
        if let index = stack.lastIndex(where: { $0 === node }) {
          stack.removeSubrange(index...)
        }
        return
      }
      if specialElements.contains(node.localName) { return }
    }
  }

  /// Handles a token in body mode, with misplaced content moved in front
  /// of the table.
  private func fosterParent(_ token: HTMLToken) {
    fosterParenting = true
    inBody(token)
    fosterParenting = false
  }

  private func inTable(_ token: HTMLToken) {
    switch token {
    case .characters(let text):
      if ["table", "tbody", "tfoot", "thead", "tr"].contains(currentNode.localName),
        !text.unicodeScalars.allSatisfy(isHTMLWhitespace)
      {
        fosterParent(token)
      } else {
        insertCharacters(text)
      }
    case .comment(let data): insertComment(data)
    case .doctype: return
    case .startTag("caption", let attributes, _):
      clearStack(backTo: ["table", "template", "html"])
      formatting.append(nil)
      insertElement("caption", attributes)
      mode = .inCaption
    case .startTag("colgroup", let attributes, _):
      clearStack(backTo: ["table", "template", "html"])
      insertElement("colgroup", attributes)
      mode = .inColumnGroup
    case .startTag("col", _, _):
      clearStack(backTo: ["table", "template", "html"])
      insertElement("colgroup")
      mode = .inColumnGroup
      process(token)
    case .startTag(let name, let attributes, _) where tableSections.contains(name):
      clearStack(backTo: ["table", "template", "html"])
      insertElement(name, attributes)
      mode = .inTableBody
    case .startTag("td", _, _), .startTag("th", _, _), .startTag("tr", _, _):
      clearStack(backTo: ["table", "template", "html"])
      insertElement("tbody")
      mode = .inTableBody
      process(token)
    case .startTag("table", _, _):
      guard inTableScope(["table"]) else { return }
      popUntil(["table"])
      resetInsertionMode()
      process(token)
    case .startTag("style", _, _), .startTag("script", _, _):
      inHead(token)
    case .startTag("input", let attributes, _)
    where attributes.contains { $0.name == "type" && $0.value.lowercased() == "hidden" }:
      insertVoid("input", attributes)
    case .endTag("table"):
      guard inTableScope(["table"]) else { return }
      popUntil(["table"])
      resetInsertionMode()
    case .endTag(let name)
    where [
      "body", "caption", "col", "colgroup", "html", "tbody", "td", "tfoot", "th", "thead", "tr",
    ]
    .contains(name):
      return
    case .eof: inBody(token)
    default: fosterParent(token)
    }
  }

  private func inCaption(_ token: HTMLToken) {
    switch token {
    case .endTag("caption"):
      guard inTableScope(["caption"]) else { return }
      generateImpliedEndTags()
      popUntil(["caption"])
      clearFormattingToMarker()
      mode = .inTable
    case .startTag(let name, _, _)
    where ["caption", "col", "colgroup", "tbody", "td", "tfoot", "th", "thead", "tr"].contains(name):
      closeCaptionAndReprocess(token)
    case .endTag("table"):
      closeCaptionAndReprocess(token)
    case .endTag(let name)
    where ["body", "col", "colgroup", "html", "tbody", "td", "tfoot", "th", "thead", "tr"]
      .contains(name):
      return
    default: inBody(token)
    }
  }

  private func closeCaptionAndReprocess(_ token: HTMLToken) {
    guard inTableScope(["caption"]) else { return }
    generateImpliedEndTags()
    popUntil(["caption"])
    clearFormattingToMarker()
    mode = .inTable
    process(token)
  }

  private func inColumnGroup(_ token: HTMLToken) {
    switch token {
    case .comment(let data): insertComment(data)
    case .doctype: return
    case .startTag("col", let attributes, _): insertVoid("col", attributes)
    case .endTag("colgroup"):
      guard currentNode.localName == "colgroup" else { return }
      stack.removeLast()
      mode = .inTable
    case .endTag("col"): return
    case .eof: inBody(token)
    default:
      guard let rest = splitWhitespace(token, { self.insertCharacters($0) }) else { return }
      guard currentNode.localName == "colgroup" else { return }
      stack.removeLast()
      mode = .inTable
      process(rest)
    }
  }

  private func inTableBody(_ token: HTMLToken) {
    switch token {
    case .startTag("tr", let attributes, _):
      clearStack(backTo: ["tbody", "tfoot", "thead", "template", "html"])
      insertElement("tr", attributes)
      mode = .inRow
    case .startTag("th", _, _), .startTag("td", _, _):
      clearStack(backTo: ["tbody", "tfoot", "thead", "template", "html"])
      insertElement("tr")
      mode = .inRow
      process(token)
    case .endTag(let name) where tableSections.contains(name):
      guard inTableScope([name]) else { return }
      clearStack(backTo: ["tbody", "tfoot", "thead", "template", "html"])
      stack.removeLast()
      mode = .inTable
    case .startTag(let name, _, _)
    where ["caption", "col", "colgroup", "tbody", "tfoot", "thead"].contains(name):
      closeTableSectionAndReprocess(token)
    case .endTag("table"):
      closeTableSectionAndReprocess(token)
    case .endTag(let name)
    where ["body", "caption", "col", "colgroup", "html", "td", "th", "tr"].contains(name):
      return
    default: inTable(token)
    }
  }

  private func closeTableSectionAndReprocess(_ token: HTMLToken) {
    guard inTableScope(tableSections) else { return }
    clearStack(backTo: ["tbody", "tfoot", "thead", "template", "html"])
    stack.removeLast()
    mode = .inTable
    process(token)
  }

  private func inRow(_ token: HTMLToken) {
    switch token {
    case .startTag(let name, let attributes, _) where name == "td" || name == "th":
      clearStack(backTo: ["tr", "template", "html"])
      insertElement(name, attributes)
      mode = .inCell
      formatting.append(nil)
    case .endTag("tr"):
      guard inTableScope(["tr"]) else { return }
      clearStack(backTo: ["tr", "template", "html"])
      stack.removeLast()
      mode = .inTableBody
    case .startTag(let name, _, _)
    where ["caption", "col", "colgroup", "tbody", "tfoot", "thead", "tr"].contains(name):
      closeRowAndReprocess(token)
    case .endTag("table"):
      closeRowAndReprocess(token)
    case .endTag(let name) where tableSections.contains(name):
      guard inTableScope([name]) else { return }
      closeRowAndReprocess(token)
    case .endTag(let name)
    where ["body", "caption", "col", "colgroup", "html", "td", "th"].contains(name):
      return
    default: inTable(token)
    }
  }

  private func closeRowAndReprocess(_ token: HTMLToken) {
    guard inTableScope(["tr"]) else { return }
    clearStack(backTo: ["tr", "template", "html"])
    stack.removeLast()
    mode = .inTableBody
    process(token)
  }

  private func closeCell() {
    generateImpliedEndTags()
    popUntil(["td", "th"])
    clearFormattingToMarker()
    mode = .inRow
  }

  private func inCell(_ token: HTMLToken) {
    switch token {
    case .endTag(let name) where name == "td" || name == "th":
      guard inTableScope([name]) else { return }
      closeCell()
    case .startTag(let name, _, _)
    where ["caption", "col", "colgroup", "tbody", "td", "tfoot", "th", "thead", "tr"].contains(name):
      guard inTableScope(["td", "th"]) else { return }
      closeCell()
      process(token)
    case .endTag(let name) where ["body", "caption", "col", "colgroup", "html"].contains(name):
      return
    case .endTag(let name) where ["table", "tbody", "tfoot", "thead", "tr"].contains(name):
      guard inTableScope([name]) else { return }
      closeCell()
      process(token)
    default: inBody(token)
    }
  }

  private func afterBody(_ token: HTMLToken) {
    switch token {
    case .comment(let data): insertComment(data, into: stack[0])
    case .doctype: return
    case .endTag("html"): mode = .afterAfterBody
    case .eof: stopped = true
    case .startTag("html", _, _): inBody(token)
    default:
      guard let rest = splitWhitespace(token, { self.inBody(.characters($0)) }) else { return }
      mode = .inBody
      process(rest)
    }
  }

  private func afterAfterBody(_ token: HTMLToken) {
    switch token {
    case .comment(let data): insertComment(data, into: document)
    case .doctype, .startTag("html", _, _): inBody(token)
    case .eof: stopped = true
    default:
      guard let rest = splitWhitespace(token, { self.inBody(.characters($0)) }) else { return }
      mode = .inBody
      process(rest)
    }
  }
}

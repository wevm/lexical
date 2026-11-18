/// `Element` (an HTML element): its tag, its attributes in the order they
/// were set, its inline style and its class list.
public final class DOMElement: DOMNode {
  /// `localName`: the lower-case tag name.
  public let localName: String
  private var attributeList: [(name: String, value: String)] = []
  private var styleDeclaration: CSSStyleDeclaration?

  public init(_ localName: String, _ ownerDocument: DOMDocument? = nil) {
    self.localName = localName
    super.init(.element, ownerDocument)
  }

  /// `tagName` and `nodeName`: the upper-case tag name.
  public var tagName: String { localName.uppercased() }
  public override var nodeName: String { tagName }

  // MARK: Attributes

  /// The attributes as `[name, value]` in order, as `element.attributes`.
  public var attributes: [(name: String, value: String)] { attributeList }

  public func getAttribute(_ name: String) -> String? {
    let name = name.lowercased()
    return attributeList.first { $0.name == name }?.value
  }

  public func hasAttribute(_ name: String) -> Bool { getAttribute(name) != nil }

  public func getAttributeNames() -> [String] { attributeList.map(\.name) }

  public func setAttribute(_ name: String, _ value: String) {
    let name = name.lowercased()
    setAttributeValue(name, value)
    if name == "style" { styleDeclaration?.parse(value) }
  }

  public func removeAttribute(_ name: String) {
    let name = name.lowercased()
    attributeList.removeAll { $0.name == name }
    if name == "style" { styleDeclaration?.parse("") }
  }

  /// `toggleAttribute(name, force)`.
  @discardableResult
  public func toggleAttribute(_ name: String, _ force: Bool? = nil) -> Bool {
    let present = hasAttribute(name)
    let next = force ?? !present
    if next && !present { setAttribute(name, "") }
    if !next && present { removeAttribute(name) }
    return next
  }

  /// Writes an attribute without touching the style declaration (which is
  /// how the declaration itself updates the `style` attribute).
  fileprivate func setAttributeValue(_ name: String, _ value: String) {
    if let index = attributeList.firstIndex(where: { $0.name == name }) {
      attributeList[index].value = value
    } else {
      attributeList.append((name, value))
    }
  }

  /// `id`.
  public var id: String {
    get { getAttribute("id") ?? "" }
    set { setAttribute("id", newValue) }
  }

  /// `dir`.
  public var dir: String {
    get { getAttribute("dir") ?? "" }
    set { setAttribute("dir", newValue) }
  }

  // MARK: Classes

  /// `className`.
  public var className: String {
    get { getAttribute("class") ?? "" }
    set { setAttribute("class", newValue) }
  }

  /// `classList`.
  public var classList: DOMTokenList { DOMTokenList(self, "class") }

  // MARK: Style

  /// `style`: the inline style declaration, kept in step with the `style`
  /// attribute.
  public var style: CSSStyleDeclaration {
    if let styleDeclaration { return styleDeclaration }
    let declaration = CSSStyleDeclaration(owner: self)
    declaration.parse(getAttribute("style") ?? "")
    styleDeclaration = declaration
    return declaration
  }

  fileprivate func styleDidChange(_ cssText: String) { setAttributeValue("style", cssText) }

  // MARK: Serialization

  /// `innerHTML`: the serialized children.
  public var innerHTML: String { serializeHTMLChildren(of: self) }

  /// `outerHTML`: the element serialized.
  public var outerHTML: String {
    var out = ""
    serializeHTML(self, into: &out)
    return out
  }

  public override func cloneNode(_ deep: Bool = false) -> DOMNode {
    let copy = DOMElement(localName, ownerDocument)
    copy.attributeList = attributeList
    if deep { cloneChildren(into: copy) }
    return copy
  }

  // MARK: Queries

  /// The descendant elements in document order.
  public func descendantElements() -> [DOMElement] {
    var out: [DOMElement] = []
    func walk(_ node: DOMNode) {
      for child in node.childNodes {
        if let element = child as? DOMElement {
          out.append(element)
          walk(element)
        }
      }
    }
    walk(self)
    return out
  }

  /// `getElementsByTagName(name)`.
  public func getElementsByTagName(_ name: String) -> [DOMElement] {
    let name = name.lowercased()
    return descendantElements().filter { name == "*" || $0.localName == name }
  }
}

/// `DOMTokenList` over an attribute: unique tokens separated by single
/// spaces once changed.
public struct DOMTokenList {
  let element: DOMElement
  let attribute: String

  init(_ element: DOMElement, _ attribute: String) {
    self.element = element
    self.attribute = attribute
  }

  /// The tokens, without duplicates.
  public var tokens: [String] {
    var seen = Set<String>()
    return (element.getAttribute(attribute) ?? "")
      .split(whereSeparator: {
        $0 == " " || $0 == "\t" || $0 == "\n" || $0 == "\r" || $0 == "\u{0C}"
      })
      .map(String.init)
      .filter { seen.insert($0).inserted }
  }

  public var length: Int { tokens.count }
  public var value: String { element.getAttribute(attribute) ?? "" }

  public func contains(_ token: String) -> Bool { tokens.contains(token) }

  private func update(_ tokens: [String]) {
    // The spec skips the update when there is no attribute and no tokens.
    if !element.hasAttribute(attribute) && tokens.isEmpty { return }
    element.setAttribute(attribute, tokens.joined(separator: " "))
  }

  public func add(_ tokens: String...) { add(contentsOf: tokens) }

  public func add(contentsOf newTokens: [String]) {
    var list = tokens
    for token in newTokens where !list.contains(token) { list.append(token) }
    update(list)
  }

  public func remove(_ tokens: String...) { remove(contentsOf: tokens) }

  public func remove(contentsOf removed: [String]) {
    update(tokens.filter { !removed.contains($0) })
  }

  @discardableResult
  public func toggle(_ token: String, _ force: Bool? = nil) -> Bool {
    let present = contains(token)
    let next = force ?? !present
    if next && !present { add(token) }
    if !next && present { remove(token) }
    return next
  }
}

/// `CSSStyleDeclaration` for an element's inline style: properties in the
/// order they were first set, with their priority. Values are normalized
/// the way the declaration in the upstream tests (jsdom) normalizes the
/// values Lexical writes; see `normalizeCSSValue`.
public final class CSSStyleDeclaration {
  private weak var owner: DOMElement?
  private var entries: [(name: String, value: String, priority: String)] = []

  init(owner: DOMElement?) { self.owner = owner }

  /// A declaration that belongs to no element.
  public convenience init() { self.init(owner: nil) }

  public var length: Int { entries.count }

  /// `item(index)`: the name of the property at `index`.
  public func item(_ index: Int) -> String { index < entries.count ? entries[index].name : "" }

  /// The property names in order.
  public var propertyNames: [String] { entries.map(\.name) }

  public func getPropertyValue(_ name: String) -> String {
    let name = canonicalName(name)
    return entries.first { $0.name == name }?.value ?? ""
  }

  public func getPropertyPriority(_ name: String) -> String {
    let name = canonicalName(name)
    return entries.first { $0.name == name }?.priority ?? ""
  }

  /// `setProperty(name, value, priority)`: an empty value removes the
  /// property; an unknown property or invalid priority is ignored.
  public func setProperty(_ name: String, _ value: String, _ priority: String = "") {
    let name = canonicalName(name)
    let value = preprocessCSS(value)
    if jsTrim(value).isEmpty {
      // The `font` setter clears its longhands too.
      var changed = false
      if name == "font" {
        for longhand in fontLonghands { changed = remove(longhand) != nil || changed }
      }
      if remove(name) != nil || changed { notify() }
      return
    }
    let priority = priority.lowercased()
    guard priority == "" || priority == "important" else { return }
    guard setPropertyValue(name, value, priority) else { return }
    notify()
  }

  /// `removeProperty(name)`: returns the removed value.
  @discardableResult
  public func removeProperty(_ name: String) -> String {
    guard let value = remove(canonicalName(name)) else { return "" }
    notify()
    return value
  }

  private func remove(_ name: String) -> String? {
    guard let index = entries.firstIndex(where: { $0.name == name }) else { return nil }
    return entries.remove(at: index).value
  }

  /// Reads or writes a property by its CSS name, as `style.whiteSpace`
  /// does by its IDL name.
  public subscript(_ name: String) -> String {
    get { getPropertyValue(name) }
    set { setProperty(name, newValue) }
  }

  /// `cssText`.
  public var cssText: String {
    get {
      // `font` stands for the longhands set before it, unless one of them
      // is important and it isn't.
      var serialized: [(name: String, value: String, priority: String)] = []
      for entry in entries {
        if entry.name == "font" {
          serialized.removeAll {
            fontLonghands.contains($0.name) && (!entry.priority.isEmpty || $0.priority.isEmpty)
          }
        }
        serialized.append(entry)
      }
      return serialized.map {
        "\($0.name): \($0.value)\($0.priority.isEmpty ? "" : " !important");"
      }
      .joined(separator: " ")
    }
    set {
      parse(newValue)
      notify()
    }
  }

  /// Replaces the declarations with those in `css`, without writing the
  /// `style` attribute (which is where `css` came from). A style rule's
  /// block (`keepCase: false`) matches property names in any case, as
  /// jsdom's CSSOM does; a style attribute's keeps their case.
  ///
  /// jsdom's `cssText` setter: each property's last declaration, unless an
  /// earlier one is important, in the place of its first; then
  /// `prepareProperties`, which puts the longhands of a `font` before it;
  /// then each property's setter in that order.
  func parse(_ css: String, keepCase: Bool = true) {
    entries = []
    var declared: [(name: String, value: String, priority: String)] = []
    for (name, rawValue) in parseCSSDeclarations(preprocessCSS(css)) {
      let (value, important) = splitImportant(rawValue)
      let name = canonicalName(name, fromText: keepCase)
      if name != "font" && normalizeCSSValue(name, value) == nil { continue }
      let priority = important ? "important" : ""
      if let i = declared.firstIndex(where: { $0.name == name }) {
        if declared[i].priority.isEmpty { declared[i] = (name, value, priority) }
      } else {
        declared.append((name, value, priority))
      }
    }
    var prepared: [(name: String, value: String, priority: String)] = []
    func prepare(_ name: String, _ value: String, _ priority: String) {
      if let i = prepared.firstIndex(where: { $0.name == name }) {
        prepared[i] = (name, value, priority)
      } else {
        prepared.append((name, value, priority))
      }
    }
    for entry in declared {
      if entry.name == "font" && !hasVarFunc(entry.value) {
        for (longhand, value) in parseFont(entry.value) ?? [] {
          prepare(longhand, value, entry.priority)
        }
      }
      prepare(entry.name, entry.value, entry.priority)
    }
    for entry in prepared { _ = setPropertyValue(entry.name, entry.value, entry.priority) }
  }

  /// jsdom's `_setPropertyValue`: the property's setter, which for `font`
  /// writes its longhands, and for a longhand of it set to a `var()`
  /// clears the shorthand.
  private func setPropertyValue(_ name: String, _ value: String, _ priority: String) -> Bool {
    if name == "font" { return setFont(jsTrim(value), priority) }
    var changed = false
    if fontLonghands.contains(name) && hasVarFunc(value) { changed = remove("font") != nil }
    return write(name, value, priority) || changed
  }

  /// The `font` setter (`properties/font.js`' `descriptor.set`): the
  /// longhands `value` expands into, then the shorthand, as the longhands
  /// that aren't `normal`; a `var()` is kept whole, without longhands.
  private func setFont(_ value: String, _ priority: String) -> Bool {
    if hasVarFunc(value) {
      for longhand in fontLonghands { _ = remove(longhand) }
      writeRaw("font", value, priority)
      return true
    }
    guard let longhands = parseFont(value) else { return false }
    for key in fontLonghands {
      if let value = longhands.first(where: { $0.0 == key })?.1 { writeRaw(key, value, priority) }
    }
    writeRaw("font", serializeFont(longhands), priority)
    return true
  }

  /// jsdom's `_setProperty`: stores the value as it is, in place when the
  /// property is set already.
  private func writeRaw(_ name: String, _ value: String, _ priority: String) {
    if let index = entries.firstIndex(where: { $0.name == name }) {
      entries[index].value = value
      entries[index].priority = priority
    } else {
      entries.append((name, value, priority))
    }
  }

  private func write(_ name: String, _ value: String, _ priority: String) -> Bool {
    guard let value = normalizeCSSValue(name, value) else { return false }
    writeRaw(name, value, priority)
    return true
  }

  private func notify() { owner?.styleDidChange(cssText) }

  /// Property names are matched lower-case, except custom properties
  /// (`--name`). Declarations parsed from text keep their case, so an
  /// upper-case name there is unknown, as in jsdom.
  private func canonicalName(_ name: String, fromText: Bool = false) -> String {
    name.hasPrefix("--") || fromText ? name : name.lowercased()
  }
}

/// Splits a trailing `!important` (any case) off a declaration value.
private func splitImportant(_ value: String) -> (String, Bool) {
  let trimmed = jsTrim(value)
  let flag = "!important"
  guard trimmed.utf16.count >= flag.utf16.count,
    trimmed.lowercased().hasSuffix(flag)
  else {
    return (trimmed, false)
  }
  return (jsTrim(String(trimmed.dropLast(flag.count))), true)
}

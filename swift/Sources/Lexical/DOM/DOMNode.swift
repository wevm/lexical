// A small DOM: the part of the WHATWG DOM that Lexical's `createDOM`,
// `exportDOM`, HTML import and export, and the clipboard use. The views
// don't render it; they render the editor state through TextKit. It exists
// so that HTML export and import run the same node methods as JS and write
// the same bytes.

/// `Node.nodeType`.
public enum DOMNodeType: Int, Sendable {
  case element = 1
  case text = 3
  case comment = 8
  case document = 9
  case documentFragment = 11
}

/// `Node`.
open class DOMNode {
  public let nodeType: DOMNodeType
  public private(set) weak var parentNode: DOMNode?
  public private(set) var childNodes: [DOMNode] = []
  /// The document that created the node.
  public internal(set) weak var ownerDocument: DOMDocument?

  init(_ nodeType: DOMNodeType, _ ownerDocument: DOMDocument?) {
    self.nodeType = nodeType
    self.ownerDocument = ownerDocument
  }

  /// `nodeName`: the upper-case tag name of an element, `#text`, `#comment`,
  /// `#document` or `#document-fragment`.
  open var nodeName: String {
    switch nodeType {
    case .element: ""
    case .text: "#text"
    case .comment: "#comment"
    case .document: "#document"
    case .documentFragment: "#document-fragment"
    }
  }

  public var parentElement: DOMElement? { parentNode as? DOMElement }
  public var firstChild: DOMNode? { childNodes.first }
  public var lastChild: DOMNode? { childNodes.last }
  public var hasChildNodes: Bool { !childNodes.isEmpty }

  public var nextSibling: DOMNode? {
    guard let parent = parentNode, let index = parent.indexOf(self) else { return nil }
    return index + 1 < parent.childNodes.count ? parent.childNodes[index + 1] : nil
  }

  public var previousSibling: DOMNode? {
    guard let parent = parentNode, let index = parent.indexOf(self) else { return nil }
    return index > 0 ? parent.childNodes[index - 1] : nil
  }

  /// `previousElementSibling`: the nearest element before this node.
  public var previousElementSibling: DOMElement? {
    guard let parent = parentNode, let index = parent.indexOf(self) else { return nil }
    return parent.childNodes[..<index].last { $0 is DOMElement } as? DOMElement
  }

  /// The element children.
  public var children: [DOMElement] { childNodes.compactMap { $0 as? DOMElement } }

  func indexOf(_ child: DOMNode) -> Int? { childNodes.firstIndex { $0 === child } }

  /// `textContent`: the text of the descendants; setting it replaces the
  /// children with one text node.
  open var textContent: String {
    get { childNodes.map(\.textContent).joined() }
    set {
      replaceChildren(newValue.isEmpty ? [] : [DOMText(newValue, ownerDocument)])
    }
  }

  /// The nodes `node` stands for when inserted: a fragment's children.
  private func expand(_ node: DOMNode) -> [DOMNode] {
    node.nodeType == .documentFragment ? node.childNodes : [node]
  }

  /// `insertBefore(node, child)`: moves `node` (a fragment's children) in
  /// front of `child`, or to the end when `child` is `nil`.
  @discardableResult
  public func insertBefore(_ node: DOMNode, _ child: DOMNode?) -> DOMNode {
    let nodes = expand(node)
    for n in nodes { n.parentNode?.detach(n) }
    var index = child.flatMap(indexOf) ?? childNodes.count
    for n in nodes {
      n.parentNode = self
      childNodes.insert(n, at: index)
      index += 1
    }
    return node
  }

  /// `appendChild(node)`.
  @discardableResult
  public func appendChild(_ node: DOMNode) -> DOMNode { insertBefore(node, nil) }

  /// `append(...nodes)`.
  public func append(_ nodes: DOMNode...) { append(contentsOf: nodes) }

  public func append(contentsOf nodes: [DOMNode]) {
    for node in nodes { appendChild(node) }
  }

  /// `append(text)`.
  public func append(_ text: String) { appendChild(DOMText(text, ownerDocument)) }

  /// `prepend(...nodes)`.
  public func prepend(_ nodes: DOMNode...) {
    let first = firstChild
    for node in nodes { insertBefore(node, first) }
  }

  private func detach(_ child: DOMNode) {
    if let index = indexOf(child) {
      childNodes.remove(at: index)
      child.parentNode = nil
    }
  }

  /// `removeChild(child)`.
  @discardableResult
  public func removeChild(_ child: DOMNode) -> DOMNode {
    detach(child)
    return child
  }

  /// `replaceChild(node, child)`.
  @discardableResult
  public func replaceChild(_ node: DOMNode, _ child: DOMNode) -> DOMNode {
    let next = child.nextSibling
    detach(child)
    insertBefore(node, next)
    return child
  }

  /// `remove()`.
  public func remove() { parentNode?.detach(self) }

  /// `replaceWith(...nodes)`.
  public func replaceWith(_ nodes: DOMNode...) {
    guard let parent = parentNode else { return }
    let next = nextSibling
    parent.detach(self)
    for node in nodes { parent.insertBefore(node, next) }
  }

  /// `replaceChildren(...nodes)`.
  public func replaceChildren(_ nodes: [DOMNode]) {
    for child in childNodes { child.parentNode = nil }
    childNodes = []
    append(contentsOf: nodes)
  }

  public func replaceChildren(_ nodes: DOMNode...) { replaceChildren(nodes) }

  /// `contains(other)`: whether `other` is this node or a descendant.
  public func contains(_ other: DOMNode?) -> Bool {
    var node = other
    while let current = node {
      if current === self { return true }
      node = current.parentNode
    }
    return false
  }

  /// `cloneNode(deep)`.
  open func cloneNode(_ deep: Bool = false) -> DOMNode {
    fatalError("cloneNode is implemented by each node type")
  }

  func cloneChildren(into copy: DOMNode) {
    for child in childNodes { copy.appendChild(child.cloneNode(true)) }
  }
}

/// `CharacterData`: a text or comment node.
open class DOMCharacterData: DOMNode {
  public var data: String
  init(_ nodeType: DOMNodeType, _ data: String, _ ownerDocument: DOMDocument?) {
    self.data = data
    super.init(nodeType, ownerDocument)
  }
  open override var textContent: String {
    get { data }
    set { data = newValue }
  }
  public var nodeValue: String {
    get { data }
    set { data = newValue }
  }
}

/// `Text`.
public final class DOMText: DOMCharacterData {
  public init(_ data: String, _ ownerDocument: DOMDocument? = nil) {
    super.init(.text, data, ownerDocument)
  }
  public override func cloneNode(_ deep: Bool = false) -> DOMNode { DOMText(data, ownerDocument) }
}

/// `Comment`.
public final class DOMComment: DOMCharacterData {
  public init(_ data: String, _ ownerDocument: DOMDocument? = nil) {
    super.init(.comment, data, ownerDocument)
  }
  public override func cloneNode(_ deep: Bool = false) -> DOMNode {
    DOMComment(data, ownerDocument)
  }
}

/// `DocumentFragment`.
public final class DOMDocumentFragment: DOMNode {
  public init(_ ownerDocument: DOMDocument? = nil) { super.init(.documentFragment, ownerDocument) }
  public override func cloneNode(_ deep: Bool = false) -> DOMNode {
    let copy = DOMDocumentFragment(ownerDocument)
    if deep { cloneChildren(into: copy) }
    return copy
  }
  /// The serialized children, as an element's `innerHTML`.
  public var innerHTML: String { serializeHTMLChildren(of: self) }
}

/// `Document`: creates nodes. A parsed HTML document has `documentElement`,
/// `head` and `body`.
public final class DOMDocument: DOMNode {
  public init() { super.init(.document, nil) }

  public func createElement(_ tagName: String) -> DOMElement {
    DOMElement(tagName.lowercased(), self)
  }
  public func createTextNode(_ data: String) -> DOMText { DOMText(data, self) }
  public func createComment(_ data: String) -> DOMComment { DOMComment(data, self) }
  public func createDocumentFragment() -> DOMDocumentFragment { DOMDocumentFragment(self) }

  public var documentElement: DOMElement? { children.first }
  public var head: DOMElement? { documentElement?.children.first { $0.localName == "head" } }
  public var body: DOMElement? { documentElement?.children.first { $0.localName == "body" } }

  public override func cloneNode(_ deep: Bool = false) -> DOMNode {
    let copy = DOMDocument()
    if deep { cloneChildren(into: copy) }
    return copy
  }
}

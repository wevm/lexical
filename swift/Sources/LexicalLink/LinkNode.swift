import Lexical

// Port of packages/lexical-link/src/LexicalLinkNode.ts. The generated JSON
// implementations (LexicalLinkGeneratedJSON.ts) are a JS speed-up of the
// schema walk and are not ported; the schema below drives both.

/// `LinkAttributes`. Each attribute is `undefined` (outer `nil`, left as
/// it is), `null` (`.some(nil)`, cleared) or a string, as in JS.
public struct LinkAttributes {
  public var rel: String??
  public var target: String??
  public var title: String??

  public init(rel: String?? = .none, target: String?? = .none, title: String?? = .none) {
    self.rel = rel
    self.target = target
    self.title = title
  }

  /// `{...self, ...overrides}`: each attribute `overrides` carries replaces
  /// this one's.
  public func merging(_ overrides: LinkAttributes) -> LinkAttributes {
    LinkAttributes(
      rel: overrides.rel ?? rel, target: overrides.target ?? target,
      title: overrides.title ?? title)
  }
}

/// `AutoLinkAttributes`: `LinkAttributes` and `isUnlinked`.
public struct AutoLinkAttributes {
  public var rel: String??
  public var target: String??
  public var title: String??
  public var isUnlinked: Bool?

  public init(
    rel: String?? = .none, target: String?? = .none, title: String?? = .none,
    isUnlinked: Bool? = nil
  ) {
    self.rel = rel
    self.target = target
    self.title = title
    self.isUnlinked = isUnlinked
  }

  var linkAttributes: LinkAttributes { LinkAttributes(rel: rel, target: target, title: title) }
}

private let SUPPORTED_URL_PROTOCOLS: Set<String> = ["http:", "https:", "mailto:", "sms:", "tel:"]

/// `$structureMerges(link, next)`: whether NodeStructureExtension merges
/// `link` and its next sibling `next` itself, so that `linkNodeTransform`
/// leaves the pair to it.
private func structureMerges(_ link: LinkNode, _ next: LinkNode) throws -> Bool {
  let klass = type(of: link)
  return try type(of: next) == klass && INTERNAL_isNodeStructureEnforced(getEditor())
    && getDeclaredNodeStructure(klass).declaredBy.mergeAdjacent == klass
}

/// `LinkNode`: an inline `<a>`.
open class LinkNode: ElementNode {
  public var __url: String = ""
  public var __target: String?
  public var __rel: String?
  public var __title: String?

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      LinkNode.self, type: "link", extends: ElementNode.self,
      json: [
        // defaultAsNull preserves the legacy `value || null` semantics: an
        // empty string (or junk that coerces to it) imports as null.
        SchemaField(
          "rel", nullable(stringValue(), defaultAsNull: true), field: \LinkNode.__rel),
        SchemaField(
          "target", nullable(stringValue(), defaultAsNull: true), field: \LinkNode.__target),
        SchemaField(
          "title", nullable(stringValue(), defaultAsNull: true), field: \LinkNode.__title),
        SchemaField("url", stringValue(), field: \LinkNode.__url),
      ],
      inline: true, children: .inline,
      // What shouldMergeAdjacentLink compares. The method stays the
      // authority: linkNodeTransform calls it, and a subclass that inherits
      // this declaration is never merged by NodeStructureExtension.
      mergeAdjacent: .by([.field("url"), .field("target"), .field("rel"), .field("title")]))
  }

  open override class func importDOM() -> DOMConversionMap? {
    ["a": { _ in DOMConversion(priority: 1, convertAnchorElement) }]
  }

  /// `new LinkNode(url, attributes, key)`.
  public convenience init(
    _ url: String = "", _ attributes: LinkAttributes = LinkAttributes(), key: NodeKey? = nil
  ) throws {
    try self.init(key: key)
    __url = url
    __target = attributes.target ?? nil
    __rel = attributes.rel ?? nil
    __title = attributes.title ?? nil
  }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    let element = getDocument().createElement("a")
    updateLinkDOM(nil, element, config)
    addClassNamesToElement(element, config.theme["link"]?.stringValue)
    return element
  }

  /// Writes the link's `href`, `target`, `rel` and `title` that differ
  /// from `prevNode`'s (all of them when `prevNode` is `nil`). Only an
  /// `<a>` takes them.
  open func updateLinkDOM(_ prevNode: LinkNode?, _ anchor: DOMElement, _ config: EditorConfig) {
    guard anchor.tagName == "A" else { return }
    if prevNode == nil || prevNode!.__url != __url {
      anchor.setAttribute("href", sanitizeUrl(__url))
    }
    for (attr, value, prev) in [
      ("target", __target, prevNode?.__target), ("rel", __rel, prevNode?.__rel),
      ("title", __title, prevNode?.__title),
    ] where prevNode == nil || prev != value {
      if let value, !value.isEmpty {
        anchor.setAttribute(attr, value)
      } else {
        anchor.removeAttribute(attr)
      }
    }
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    updateLinkDOM(prevNode as? LinkNode, dom, config)
    return false
  }

  /// The URL to navigate to: `url` formatted with `formatUrl`, or
  /// `about:blank` when its scheme is not one of http, https, mailto, sms
  /// and tel. See `whatwgURLProtocol` for how a URL that JS `new URL()`
  /// rejects is handled.
  open func sanitizeUrl(_ url: String) -> String {
    let rawUrl = url
    let url = formatUrl(url)
    if let urlProtocol = whatwgURLProtocol(formatUrl(url)) {
      if !SUPPORTED_URL_PROTOCOLS.contains(urlProtocol) {
        return "about:blank"
      }
    } else {
      // `new URL()` threw. Check the original input, with C0 controls, DEL
      // and whitespace stripped as browsers do when resolving a scheme: an
      // explicit scheme that is not allowlisted is neutralized.
      let normalized = String(
        String.UnicodeScalarView(
          rawUrl.unicodeScalars.filter {
            !($0.value <= 0x1F || $0.value == 0x7F || isJSWhitespace($0))
          }
        ))
      if let scheme = leadingScheme(normalized),
        !SUPPORTED_URL_PROTOCOLS.contains(scheme.lowercased() + ":")
      {
        return "about:blank"
      }
    }
    return url
  }

  open func getURL() throws -> String { try getLatest().__url }

  @discardableResult
  open func setURL(_ url: String) throws -> Self {
    let writable = try getWritable()
    writable.__url = url
    return writable
  }

  open func getTarget() throws -> String? { try getLatest().__target }

  @discardableResult
  open func setTarget(_ target: String?) throws -> Self {
    let writable = try getWritable()
    writable.__target = target
    return writable
  }

  open func getRel() throws -> String? { try getLatest().__rel }

  @discardableResult
  open func setRel(_ rel: String?) throws -> Self {
    let writable = try getWritable()
    writable.__rel = rel
    return writable
  }

  open func getTitle() throws -> String? { try getLatest().__title }

  @discardableResult
  open func setTitle(_ title: String?) throws -> Self {
    let writable = try getWritable()
    writable.__title = title
    return writable
  }

  open override func insertNewAfter(
    _ selection: RangeSelection, restoreSelection: Bool = true
  ) throws -> LexicalNode? {
    let linkNode = try copyNode(self)
    try insertAfter(linkNode, restoreSelection: restoreSelection)
    return linkNode
  }

  open override func canInsertTextBefore() throws -> Bool { false }
  open override func canInsertTextAfter() throws -> Bool { false }
  open override func canBeEmpty() throws -> Bool { false }
  open override func isInline() throws -> Bool { true }

  open override func extractWithChild(
    _ child: LexicalNode, _ selection: BaseSelection?, _ destination: String
  ) throws -> Bool {
    guard let selection = selection as? RangeSelection else { return false }
    let anchorNode = try selection.anchor.getNode()
    let focusNode = try selection.focus.getNode()
    return try (self.is(anchorNode) || isParentOf(anchorNode))
      && (self.is(focusNode) || isParentOf(focusNode)) && !selection.getTextContent().isEmpty
  }

  open func isEmailURI() throws -> Bool { try getURL().hasPrefix("mailto:") }

  open func isWebSiteURI() throws -> Bool {
    let url = try getURL()
    return url.hasPrefix("https://") || url.hasPrefix("http://")
  }

  open func shouldMergeAdjacentLink(_ otherLink: LinkNode) throws -> Bool {
    try getType() == otherLink.getType() && getURL() == otherLink.getURL()
      && getTarget() == otherLink.getTarget() && getRel() == otherLink.getRel()
      && getTitle() == otherLink.getTitle()
  }
}

/// JS `\s`.
private func isJSWhitespace(_ s: Unicode.Scalar) -> Bool {
  switch s.value {
  case 0x09...0x0D, 0x20, 0xA0, 0x1680, 0x2000...0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000,
    0xFEFF:
    true
  default: false
  }
}

private func isASCIIAlpha(_ c: Character) -> Bool {
  c.isASCII && c.isLetter
}

/// The scheme `/^([a-z][a-z0-9+.-]*):/i` matches at the start of `s`.
private func leadingScheme(_ s: String) -> String? {
  guard let first = s.first, isASCIIAlpha(first) else { return nil }
  var scheme = ""
  for c in s {
    if c == ":" { return scheme }
    guard c.isASCII, c.isLetter || c.isNumber || c == "+" || c == "." || c == "-" else {
      return nil
    }
    scheme.append(c)
  }
  return nil
}

/// `new URL(url).protocol`, or `nil` where `new URL(url)` throws.
///
/// This follows the WHATWG URL parser as far as it decides whether a URL
/// without a base parses: leading and trailing C0 controls and spaces are
/// stripped and tabs and newlines removed, a scheme is required, and the
/// authority of a special scheme (or of any `scheme://`) needs a valid port
/// and a host without forbidden code points (a special one also needs a
/// non-empty host). It does not percent-decode or IDNA-map hosts, so a host
/// those steps would reject is accepted here.
func whatwgURLProtocol(_ url: String) -> String? {
  var scalars = Array(url.unicodeScalars)
  while let first = scalars.first, first.value <= 0x20 { scalars.removeFirst() }
  while let last = scalars.last, last.value <= 0x20 { scalars.removeLast() }
  scalars.removeAll { $0 == "\t" || $0 == "\n" || $0 == "\r" }
  let input = String(String.UnicodeScalarView(scalars))
  guard let scheme = leadingScheme(input)?.lowercased() else { return nil }
  let special: Set<String> = ["http", "https", "ws", "wss", "ftp", "file"]
  let isSpecial = special.contains(scheme)
  var rest = Substring(input.dropFirst(scheme.count + 1))
  if isSpecial {
    if scheme == "file" { return "file:" }
    while let c = rest.first, c == "/" || c == "\\" { rest.removeFirst() }
  } else if rest.hasPrefix("//") {
    rest.removeFirst(2)
  } else {
    return scheme + ":"
  }
  let authorityEnd =
    rest.firstIndex { $0 == "/" || $0 == "?" || $0 == "#" || (isSpecial && $0 == "\\") }
    ?? rest.endIndex
  var authority = rest[..<authorityEnd]
  if let at = authority.lastIndex(of: "@") {
    authority = authority[authority.index(after: at)...]
  }
  var host = authority
  var port: Substring?
  if host.hasPrefix("[") {
    guard let close = host.firstIndex(of: "]") else { return nil }
    let after = host[host.index(after: close)...]
    if after.hasPrefix(":") { port = after.dropFirst() } else if !after.isEmpty { return nil }
    host = host[...close]
  } else if let colon = host.firstIndex(of: ":") {
    port = host[host.index(after: colon)...]
    host = host[..<colon]
  }
  if let port, !port.isEmpty {
    guard port.allSatisfy({ $0.isASCII && $0.isNumber }), let n = Int(port.prefix(6)),
      n <= 65535, port.count < 6 || port.drop(while: { $0 == "0" }).count <= 5
    else { return nil }
  }
  if host.isEmpty {
    return isSpecial || port != nil ? nil : scheme + ":"
  }
  if !host.hasPrefix("[") {
    let forbidden: Set<Unicode.Scalar> = [
      "\0", "\t", "\n", "\r", " ", "#", "/", ":", "<", ">", "?", "@", "[", "\\", "]", "^", "|",
    ]
    for s in host.unicodeScalars
    where forbidden.contains(s) || (isSpecial && (s.value <= 0x1F || s == "%" || s.value == 0x7F)) {
      return nil
    }
  }
  return scheme + ":"
}

private typealias CaretPair = (PointCaret, PointCaret)

private func saveCaretPair(_ point: Point) throws -> CaretPair {
  let next = try caretFromPoint(point, .next)
  return (next, try next.getFlipped())
}

private func restoreCaretPair(_ point: Point, _ pair: CaretPair) throws {
  for caret in [pair.0, pair.1] where try caret.origin.isAttached() {
    try setPointFromCaret(point, normalizeCaret(caret))
    return
  }
}

/// `$linkNodeTransform(link)`: extracts block-level children from a
/// `LinkNode`, splitting ancestors as needed, and merges it with an
/// adjacent link it should merge with.
public func linkNodeTransform(_ link: LinkNode) throws {
  let selection = try getSelection() as? RangeSelection
  var anchorPair: CaretPair? = nil
  var focusPair: CaretPair? = nil
  if let selection {
    anchorPair = try saveCaretPair(selection.anchor)
    focusPair = try saveCaretPair(selection.focus)
  }
  func restoreSelection() throws {
    if let selection {
      try restoreCaretPair(selection.anchor, anchorPair!)
      try restoreCaretPair(selection.focus, focusPair!)
      _ = try normalizeSelection(selection)
    }
  }

  var transformed = false
  // Where NodeStructureExtension enforces declared structure, it lifts
  // blocks out of links itself, and merges equal adjacent LinkNodes, so
  // this transform leaves them to it.
  let liftsBlocks = try !INTERNAL_isNodeStructureEnforced(getEditor())
  // As the JS caret iterator: the next caret is taken before the body runs.
  var caret = liftsBlocks ? try getChildCaret(link, .next).getAdjacentCaret() : nil
  while let current = caret {
    caret = try current.getAdjacentCaret()
    let node = current.origin
    if let element = node as? ElementNode, try !element.isInline() {
      let blockChildren = try element.getChildren()
      if !blockChildren.isEmpty {
        let innerLink = try copyNode(link)
        try innerLink.append(contentsOf: blockChildren)
        try element.append(innerLink)
        transformed = true
      }
      try insertNodeToNearestRootAtCaret(
        element, rewindSiblingCaret(current),
        SplitAtPointCaretNextOptions(shouldSplit: { _, _ in false }))
    }
  }
  // Fix a caret pair that points to the end of the absorbing link, which
  // would shift when new children are appended during merge.
  func fixMergeBoundaryCaret(
    _ pair: CaretPair, _ absorbingLink: LinkNode, _ mergingLink: LinkNode
  ) throws -> CaretPair {
    func isAffected(_ caret: PointCaret) -> Bool {
      caret.isSibling && caret.origin.is(absorbingLink)
    }
    if !isAffected(pair.0) && !isAffected(pair.1) {
      return pair
    }
    // Resolve the merge boundary from the merging link's start, since those
    // children survive the move and represent the boundary position.
    let fixed = try normalizeCaret(getChildCaret(mergingLink, .next))
    return (fixed, try fixed.getFlipped())
  }

  if try link.isAttached() {
    if let prevSibling = try link.getPreviousSibling() as? LinkNode,
      try !structureMerges(prevSibling, link), try prevSibling.shouldMergeAdjacentLink(link)
    {
      if let pair = anchorPair { anchorPair = try fixMergeBoundaryCaret(pair, prevSibling, link) }
      if let pair = focusPair { focusPair = try fixMergeBoundaryCaret(pair, prevSibling, link) }
      try prevSibling.append(contentsOf: link.getChildren())
      try link.remove()
      try restoreSelection()
      return
    }
    if let nextSibling = try link.getNextSibling() as? LinkNode,
      try !structureMerges(link, nextSibling), try link.shouldMergeAdjacentLink(nextSibling)
    {
      if let pair = anchorPair { anchorPair = try fixMergeBoundaryCaret(pair, link, nextSibling) }
      if let pair = focusPair { focusPair = try fixMergeBoundaryCaret(pair, link, nextSibling) }
      try link.append(contentsOf: nextSibling.getChildren())
      try nextSibling.remove()
      transformed = true
    }
  }
  if !transformed {
    return
  }
  if try !link.canBeEmpty(), try link.isEmpty() {
    let parent = try link.getParent()
    try link.remove()
    if let parent, try parent.isEmpty() {
      try parent.remove()
    }
  }
  try restoreSelection()
}

private func convertAnchorElement(_ domNode: DOMNode) throws -> DOMConversionOutput {
  var node: LexicalNode? = nil
  if let element = domNode as? DOMElement, element.tagName == "A" {
    if !element.textContent.isEmpty || !element.children.isEmpty {
      node = try createLinkNode(
        element.getAttribute("href") ?? "",
        LinkAttributes(
          rel: .some(element.getAttribute("rel")), target: .some(element.getAttribute("target")),
          title: .some(element.getAttribute("title"))))
    }
  }
  return DOMConversionOutput(node: node)
}

/// `$createLinkNode(url, attributes)`.
public func createLinkNode(
  _ url: String = "", _ attributes: LinkAttributes = LinkAttributes()
) throws -> LinkNode {
  try applyNodeReplacement(LinkNode(url, attributes))
}

/// `$isLinkNode(node)`.
public func isLinkNode(_ node: LexicalNode?) -> Bool { node is LinkNode }

/// `AutoLinkNode`: a link made while typing, which text can be typed
/// into at its end.
open class AutoLinkNode: LinkNode {
  /// Whether the autolink was ever unlinked.
  public var __isUnlinked: Bool = false

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      AutoLinkNode.self, type: "autolink", extends: LinkNode.self,
      json: [SchemaField("isUnlinked", booleanValue(), field: \AutoLinkNode.__isUnlinked)],
      // Adjacent autolinks never merge (see shouldMergeAdjacentLink).
      mergeAdjacent: .off)
  }

  /// `new AutoLinkNode(url, attributes, key)`. Without attributes, the
  /// `LinkNode` initializer it inherits makes the same node.
  public convenience init(
    _ url: String, _ attributes: AutoLinkAttributes, key: NodeKey? = nil
  ) throws {
    try self.init(url, attributes.linkAttributes, key: key)
    __isUnlinked = attributes.isUnlinked ?? false
  }

  open override func shouldMergeAdjacentLink(_ otherLink: LinkNode) throws -> Bool { false }

  open func getIsUnlinked() throws -> Bool { try getLatest().__isUnlinked }

  @discardableResult
  open func setIsUnlinked(_ value: Bool) throws -> Self {
    let writable = try getWritable()
    writable.__isUnlinked = value
    return writable
  }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    if __isUnlinked {
      return getDocument().createElement("span")
    }
    return try super.createDOM(config, editor)
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    let prevNode = prevNode as! AutoLinkNode
    return try super.updateDOM(prevNode, dom, config) || prevNode.__isUnlinked != __isUnlinked
  }

  // insertNewAfter is deliberately not overridden: LinkNode's uses
  // copyNode, which carries the element properties, NodeState and
  // __isUnlinked, and keeps a subclass's class.
}

/// `$createAutoLinkNode(url, attributes)`.
public func createAutoLinkNode(
  _ url: String = "", _ attributes: AutoLinkAttributes = AutoLinkAttributes()
) throws -> AutoLinkNode {
  try applyNodeReplacement(AutoLinkNode(url, attributes))
}

/// `$isAutoLinkNode(node)`.
public func isAutoLinkNode(_ node: LexicalNode?) -> Bool { node is AutoLinkNode }

/// The payload of `TOGGLE_LINK_COMMAND`: a URL, a URL with attributes, or
/// `nil` (JS `null`) to remove the link.
public enum ToggleLinkPayload {
  case url(String)
  case attributes(url: String, LinkAttributes)
}

public let TOGGLE_LINK_COMMAND = createCommand(
  "TOGGLE_LINK_COMMAND", payload: ToggleLinkPayload?.self)

private func getPointNode(_ point: Point, _ offset: Int) throws -> LexicalNode? {
  if point.type == .element {
    let node = try point.getNode()
    guard let element = node as? ElementNode else {
      throw LexicalError("$getPointNode: element point is not an ElementNode")
    }
    let children = try element.getChildren()
    let index = point.offset + offset
    return index >= 0 && index < children.count ? children[index] : nil
  }
  return nil
}

/// `$withSelectedNodes(fn)`: keeps the logical start and end of a range
/// selection whose element points may be reparented by `fn`.
private func withSelectedNodes<T>(_ fn: () throws -> T) throws -> T {
  guard let initialSelection = try getSelection() as? RangeSelection else {
    return try fn()
  }
  let normalized = try normalizeSelection(initialSelection)
  let isBackwards = try normalized.isBackward()
  let anchorNode = try getPointNode(normalized.anchor, isBackwards ? -1 : 0)
  let focusNode = try getPointNode(normalized.focus, isBackwards ? 0 : -1)
  let rval = try fn()
  if anchorNode != nil || focusNode != nil,
    let updatedSelection = try getSelection() as? RangeSelection
  {
    let finalSelection = updatedSelection.cloneRange()
    if let anchorNode, let anchorParent = try anchorNode.getParent() {
      try finalSelection.anchor.set(
        anchorParent.getKey(), anchorNode.getIndexWithinParent() + (isBackwards ? 1 : 0), .element)
    }
    if let focusNode, let focusParent = try focusNode.getParent() {
      try finalSelection.focus.set(
        focusParent.getKey(), focusNode.getIndexWithinParent() + (isBackwards ? 0 : 1), .element)
    }
    try setSelection(normalizeSelection(finalSelection))
  }
  return rval
}

/// `$splitLinkAtSelection(parentLink, extractedNodes)`: removes the
/// selected children from a link, at its start, its end or its middle.
private func splitLinkAtSelection(_ parentLink: LinkNode, _ extractedNodes: [LexicalNode]) throws {
  var extractedKeys = Set<NodeKey>()
  for n in extractedNodes where try parentLink.isParentOf(n) {
    extractedKeys.insert(n.getKey())
  }
  let allChildren = try parentLink.getChildren()
  // A child that is an extracted node or contains one (LinkNode >
  // HeadingNode > TextNode).
  func isExtractedChild(_ child: LexicalNode) throws -> Bool {
    if extractedKeys.contains(child.getKey()) { return true }
    guard child is ElementNode else { return false }
    for n in extractedNodes where try parentLink.isParentOf(n) && child.isParentOf(n) {
      return true
    }
    return false
  }
  let extractedChildren = try allChildren.filter(isExtractedChild)

  if extractedChildren.isEmpty && !allChildren.isEmpty {
    // The selection only touches the link at a boundary, so there is
    // nothing to unlink.
    return
  }
  if extractedChildren.count == allChildren.count {
    for child in allChildren { try parentLink.insertBefore(child) }
    try parentLink.remove()
    return
  }
  let firstExtractedIndex = try allChildren.firstIndex(where: isExtractedChild)!
  let lastExtractedIndex = try allChildren.lastIndex(where: isExtractedChild)!
  let isAtStart = firstExtractedIndex == 0
  let isAtEnd = lastExtractedIndex == allChildren.count - 1
  if isAtStart {
    for child in extractedChildren { try parentLink.insertBefore(child) }
  } else if isAtEnd {
    for child in extractedChildren.reversed() { try parentLink.insertAfter(child) }
  } else {
    for child in extractedChildren.reversed() { try parentLink.insertAfter(child) }
    let trailingChildren = allChildren[(lastExtractedIndex + 1)...]
    if !trailingChildren.isEmpty {
      let newLink = try copyNode(parentLink)
      try extractedChildren[extractedChildren.count - 1].insertAfter(newLink)
      for child in trailingChildren { try newLink.append(child) }
    }
  }
}

private func nearestNonAutoLink(_ node: LexicalNode) throws -> LinkNode? {
  try findMatchingParent(node) { $0 is LinkNode && !($0 is AutoLinkNode) } as? LinkNode
}

/// `$toggleLink(url, attributes)`: links the selection to `url`, updates
/// the links it is in, or with `url` `nil` removes them and keeps their
/// children. `rel` defaults to `noreferrer`.
public func toggleLink(_ url: String?, _ attributes: LinkAttributes = LinkAttributes()) throws {
  let target = attributes.target
  let title = attributes.title
  let rel: String?? = attributes.rel == nil ? .some("noreferrer") : attributes.rel
  let selection = try getSelection()

  if let selection = selection as? NodeSelection {
    let nodes = try selection.getNodes()
    if nodes.isEmpty {
      return
    }
    for node in nodes {
      if url == nil {
        if let linkParent = try nearestNonAutoLink(node) {
          try linkParent.insertBefore(node)
          if try linkParent.getChildren().isEmpty {
            try linkParent.remove()
          }
        }
      } else if let existingLink = try nearestNonAutoLink(node) {
        try existingLink.setURL(url!)
        if let target { try existingLink.setTarget(target) }
        if let rel { try existingLink.setRel(rel) }
        if let title { try existingLink.setTitle(title) }
      } else {
        let linkNode = try createLinkNode(
          url!, LinkAttributes(rel: rel, target: target, title: title))
        try node.insertBefore(linkNode)
        try linkNode.append(node)
      }
    }
    return
  }
  guard let selection = selection as? RangeSelection else { return }

  if selection.isCollapsed() && url == nil {
    for node in try selection.getNodes() {
      if let parentLink = try nearestNonAutoLink(node) {
        try parentLink.getParentOrThrow().splice(
          parentLink.getIndexWithinParent(), 0, parentLink.getChildren())
        try parentLink.remove()
      }
      return
    }
  }

  let nodes = try selection.extract()

  guard let url else {
    var processedLinks = Set<NodeKey>()
    for node in nodes {
      if let parentLink = try nearestNonAutoLink(node) {
        let linkKey = parentLink.getKey()
        if processedLinks.contains(linkKey) {
          continue
        }
        try splitLinkAtSelection(parentLink, nodes)
        processedLinks.insert(linkKey)
      }
    }
    return
  }
  var updatedNodes = Set<NodeKey>()
  func updateLinkNode(_ linkNode: LinkNode) throws {
    if updatedNodes.contains(linkNode.getKey()) {
      return
    }
    updatedNodes.insert(linkNode.getKey())
    try linkNode.setURL(url)
    if let target { try linkNode.setTarget(target) }
    if let rel { try linkNode.setRel(rel) }
    if let title { try linkNode.setTitle(title) }
  }
  // Add or merge LinkNodes
  if nodes.count == 1 {
    // If the node is a LinkNode or is in one, update its URL, target, rel
    // and title.
    if let linkNode = try findMatchingParent(nodes[0], isLinkNode) as? LinkNode {
      return try updateLinkNode(linkNode)
    }
  }

  try withSelectedNodes {
    var linkNode: LinkNode? = nil
    for node in nodes {
      if try !node.isAttached() {
        continue
      }
      if let parentLinkNode = try findMatchingParent(node, isLinkNode) as? LinkNode {
        try updateLinkNode(parentLinkNode)
        continue
      }
      if let element = node as? ElementNode {
        if try !element.isInline() {
          // Ignore block nodes: their children come later and are wrapped
          // in a new LinkNode.
          continue
        }
        if let link = element as? LinkNode {
          // Reuse it unless it is an autolink or this block already has a
          // LinkNode.
          if !(link is AutoLinkNode),
            try linkNode == nil || !linkNode!.getParentOrThrow().isParentOf(link)
          {
            try updateLinkNode(link)
            linkNode = link
            continue
          }
          // Unwrap it.
          for child in try link.getChildren() {
            try link.insertBefore(child)
          }
          try link.remove()
          continue
        }
      }
      if let prevLinkNode = try node.getPreviousSibling() as? LinkNode, prevLinkNode.is(linkNode) {
        try prevLinkNode.append(node)
        continue
      }
      let newLink = try createLinkNode(url, LinkAttributes(rel: rel, target: target, title: title))
      try node.insertAfter(newLink)
      try newLink.append(node)
      linkNode = newLink
    }
  }
}

/// `formatUrl(url)`: adds the protocol a URL is missing. A URL with a
/// scheme or a relative path is kept; an email address gets `mailto:`, a
/// phone number `tel:`, anything else `https://`.
public func formatUrl(_ url: String) -> String {
  if leadingScheme(url) != nil {
    return url
  }
  if let first = url.first, first == "/" || first == "#" || first == "." {
    return url
  }
  if url.contains("@") {
    return "mailto:\(url)"
  }
  if isPhoneNumber(url) {
    return "tel:\(url)"
  }
  return "https://\(url)"
}

/// `/^\+?[0-9\s()-]{5,}$/`.
private func isPhoneNumber(_ url: String) -> Bool {
  var scalars = Substring(url).unicodeScalars[...]
  if scalars.first == "+" { scalars = scalars.dropFirst() }
  return scalars.count >= 5
    && scalars.allSatisfy {
      ("0"..."9").contains($0) || isJSWhitespace($0) || $0 == "(" || $0 == ")" || $0 == "-"
    }
}

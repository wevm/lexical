import Lexical

// What a text view applies after a commit: the reconciler's edits with the
// theme's attributes for their runs, and the paragraphs and decorators the
// commit touched, whose attributes the view sets again once the edits are
// in. The views map the attributes to platform fonts and paragraph styles.

/// A text node's run and its look.
public struct TextKitStyledRun: Equatable, Sendable {
  /// Where the run is once its edit applies.
  public var range: TextKitRange
  public var attributes: TextKitTextAttributes
}

/// One reconciler edit, with the look of its runs.
public struct TextKitStyledEdit: Equatable, Sendable {
  public var edit: TextKitEdit
  public var runs: [TextKitStyledRun]
}

/// A block whose children are inline: its text and the separator after it,
/// the paragraph's look, and the look of its empty line.
public struct TextKitStyledParagraph: Equatable, Sendable {
  public var key: NodeKey
  /// Where the paragraph is once all the edits apply.
  public var range: TextKitRange
  public var attributes: TextKitParagraphAttributes
  public var text: TextKitTextAttributes
}

/// A decorator's attachment character.
public struct TextKitDecoratorSpan: Equatable, Sendable {
  public var key: NodeKey
  /// Where it is once all the edits apply.
  public var location: Int
  public var isInline: Bool
  /// For a horizontal rule, which has no view of its own (its `decorate`
  /// is `nil`, as in JS, where its `<hr>` is all there is), how to draw it.
  public var rule: TextKitRule?
}

/// One commit's changes to what the view shows.
public struct TextKitUpdate: Sendable {
  /// Applied in order; each edit's range is in the text as the edits before
  /// it left it.
  public var edits: [TextKitStyledEdit] = []
  /// The paragraphs to style again, after the edits.
  public var paragraphs: [TextKitStyledParagraph] = []
  /// Text the edits leave in place whose block changed what it passes down
  /// (a check list item's strikethrough, a list's type), to style again
  /// after the edits: the view replaces the look these attributes set.
  public var restyled: [TextKitStyledRun] = []
  /// The decorators to attach again, after the edits.
  public var decorators: [TextKitDecoratorSpan] = []
  /// The look of text outside any paragraph (the root's), for the
  /// separators the edits insert.
  public var base = TextKitTextAttributes()
  /// The selection in the rendered text, when it is a range or one node.
  public var selectedRange: TextKitRange?
  /// The nodes of a node selection, which a view marks (a selected
  /// horizontal rule is outlined).
  public var selectedNodes: Set<NodeKey> = []
  /// The commit's update tags.
  public var tags: Set<String> = []
  /// Whether the edits replace the whole text.
  public var isFull = false
  /// Whether the view shows its placeholder: `canShowPlaceholder` of the
  /// committed state, as upstream's `ContentEditable` decides it.
  public var canShowPlaceholder = false
  /// Whether the model ended the IME composition the view shows, so the
  /// view discards its marked text: the input method's composition ends
  /// with the model's.
  public var endsComposition = false

  public init() {}
}

extension TextKitTheme {
  /// Styles `edits` of `document` (call it in a read of the state they
  /// render). `dirtyElements` are the commit's; `nil` styles every
  /// paragraph and decorator. A decorator in `dirtyLeaves` is attached
  /// again, so its view shows what the node now decorates, as React
  /// renders a decorator again when its node changes.
  func update(
    _ edits: [TextKitEdit], _ document: TextKitDocument, editor: Editor,
    dirtyElements: JSMap<NodeKey, Bool>?, dirtyLeaves: JSSet<NodeKey>? = nil
  ) throws -> TextKitUpdate {
    var update = TextKitUpdate()
    update.base = textAttributes(format: 0, style: "", ancestors: [])
    // Within one state, an element's chain and a block's text styles are
    // the same for every run, and most blocks share their chain with
    // others, so a full render computes each look once.
    var chains: [NodeKey: (chain: [TextKitElement], id: Int)] = [:]
    var chainIDs: [[TextKitElement]: Int] = [:]
    func chain(_ element: ElementNode) throws -> (chain: [TextKitElement], id: Int) {
      if let chain = chains[element.getKey()] { return chain }
      var chain = try element.getParent().map { try chain($0).chain } ?? []
      chain.append(try TextKitElement(element))
      let id = chainIDs[chain] ?? chainIDs.count
      chainIDs[chain] = id
      chains[element.getKey()] = (chain, id)
      return (chain, id)
    }
    struct TextKey: Hashable {
      var chain: Int
      var format: Int
      var style: String
      var type: String
    }
    var texts: [TextKey: TextKitTextAttributes] = [:]
    func attributes(
      format: Int, style: String, parent: ElementNode?, type: String = "text"
    ) throws -> TextKitTextAttributes {
      try attributes(
        format: format, style: style, chain: parent.map(chain) ?? ([], -1), type: type)
    }
    func attributes(
      format: Int, style: String, chain: (chain: [TextKitElement], id: Int),
      type: String = "text"
    ) throws -> TextKitTextAttributes {
      let (ancestors, id) = chain
      let key = TextKey(chain: id, format: format, style: style, type: type)
      if let attributes = texts[key] { return attributes }
      let attributes = textAttributes(
        format: format, style: style, ancestors: ancestors, type: type)
      texts[key] = attributes
      return attributes
    }
    func attributes(for node: TextNode) throws -> TextKitTextAttributes {
      try attributes(
        format: node.getFormat(), style: node.getStyle(), parent: node.getParent(),
        type: node.getType())
    }
    update.isFull = dirtyElements == nil
    update.edits = try edits.map { edit in
      TextKitStyledEdit(
        edit: edit,
        runs: try edit.runs.map { run in
          // The run carries its node's format and style, and the document
          // its parent.
          let parent = try document.entries[run.key]?.parent.flatMap {
            try getNodeByKey($0) as? ElementNode
          }
          // A text node of another type (a hashtag) takes its type's style,
          // and a suffix its own.
          let type = try run.type ?? (getNodeByKey(run.key) as? TextNode)?.getType() ?? "text"
          return TextKitStyledRun(
            range: TextKitRange(location: run.location, length: run.length),
            attributes: try attributes(
              format: run.format, style: run.style, parent: parent, type: type))
        })
    }
    // Where each edit ends up: the edits after it, lower in the text, shift
    // it by how much they grow.
    var finalRanges: [TextKitRange] = []
    var shift = 0
    for edit in edits.reversed() {
      finalRanges.append(
        TextKitRange(location: edit.location + shift, length: edit.string.utf16.count))
      shift += edit.string.utf16.count - edit.length
    }

    var blocks: [ElementNode] = []
    var seen: Set<NodeKey> = []
    // Blocks under an element changed itself, whose text may inherit
    // something new.
    var restyle: Set<NodeKey> = []
    func collectLeafBlocks(_ element: ElementNode, deep: Bool) throws {
      if element is RootNode && !deep { return }
      let children = try element.getChildren()
      let hasBlockChild = try children.contains { try TextKitDocument.isBlock($0) }
      if !(element is RootNode), try !TextKitDocument.isInline(element), !hasBlockChild {
        if seen.insert(element.getKey()).inserted { blocks.append(element) }
        if deep { restyle.insert(element.getKey()) }
        return
      }
      guard deep else { return }
      for case let child as ElementNode in children {
        try collectLeafBlocks(child, deep: true)
      }
    }
    if let dirtyElements {
      for (key, intentionallyMarkedAsDirty) in dirtyElements {
        if let element = try getNodeByKey(key) as? ElementNode, try element.isAttached() {
          // An element marked dirty itself may have changed what its blocks
          // inherit (format, indent, direction); one marked as an ancestor
          // of a change only needs its own paragraph. The root, which most
          // commits mark, gives its blocks nothing a commit changes.
          try collectLeafBlocks(element, deep: intentionallyMarkedAsDirty && !(element is RootNode))
        }
      }
    } else {
      try collectLeafBlocks(getRoot(), deep: true)
    }
    // Whether a "\n" follows each block: a full render looks at every block,
    // so it reads the units from an array rather than walking the string.
    let utf16 = document.string.utf16
    let units = blocks.count > 64 ? Array(utf16) : nil
    func isNewline(at location: Int) -> Bool {
      if let units { return location < units.count && units[location] == 0x0A }
      return location < utf16.count
        && utf16[utf16.index(utf16.startIndex, offsetBy: location)] == 0x0A
    }
    var paragraphLooks: [Int: TextKitParagraphAttributes] = [:]
    for block in blocks {
      guard let span = document.span(block.getKey()) else { continue }
      var length = span.length
      if isNewline(at: span.end) {
        length += 1
      }
      let (ancestors, id) = try chain(block)
      let paragraph = paragraphLooks[id] ?? paragraphAttributes(blocks: ancestors)
      paragraphLooks[id] = paragraph
      update.paragraphs.append(
        TextKitStyledParagraph(
          key: block.getKey(), range: TextKitRange(location: span.location, length: length),
          attributes: paragraph,
          text: try attributes(
            format: block.getTextFormat(), style: block.getTextStyle(), chain: (ancestors, id))))
    }

    // A full render's one edit styles every run already.
    if !restyle.isEmpty && !update.isFull {
      // Only the restyled blocks' text: they are leaf blocks, so none holds
      // another, and in document order their runs are too.
      let restyled = restyle.compactMap { key in document.span(key).map { (key, $0.location) } }
      for run in restyled.sorted(by: { $0.1 < $1.1 }).flatMap({ document.runs(under: $0.0) })
      where run.length > 0
        && !finalRanges.contains(where: { $0.location <= run.location && run.location < $0.end })
      {
        guard let node = try getNodeByKey(run.key) as? TextNode else { continue }
        update.restyled.append(
          TextKitStyledRun(
            range: TextKitRange(location: run.location, length: run.length),
            attributes: try run.type.map {
              try attributes(
                format: node.getFormat(), style: node.getStyle(), parent: node.getParent(), type: $0
              )
            } ?? attributes(for: node)))
      }
    }

    let decorators = editor.getDecorators()
    var keys = Array(decorators.keys)
    // A horizontal rule is drawn from the theme. Only an edit that brings a
    // decorator's character can bring a rule.
    if horizontalRule != nil,
      update.isFull || edits.contains(where: { $0.string.utf16.contains(0xFFFC) })
    {
      keys += document.entries.compactMap { key, entry in
        entry.kind == .decorator && decorators[key.key] == nil ? key.key : nil
      }
    }
    for key in keys.sorted() {
      guard let decorator = try getNodeByKey(key) as? DecoratorNode,
        let span = document.span(key),
        update.isFull || dirtyLeaves?.has(key) == true
          || finalRanges.contains(where: { $0.location <= span.location && span.location < $0.end })
      else { continue }
      var rule: TextKitRule?
      if decorators[key] == nil {
        guard decorator.getType() == "horizontalrule", let horizontalRule else { continue }
        rule = horizontalRule
      }
      update.decorators.append(
        TextKitDecoratorSpan(
          key: key, location: span.location, isInline: try decorator.isInline(), rule: rule))
    }
    return update
  }
}

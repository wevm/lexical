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
  /// The commit's update tags.
  public var tags: Set<String> = []
  /// Whether the edits replace the whole text.
  public var isFull = false

  public init() {}
}

extension TextKitTheme {
  /// Styles `edits` of `document` (call it in a read of the state they
  /// render). `dirtyElements` are the commit's; `nil` styles every
  /// paragraph and decorator.
  func update(
    _ edits: [TextKitEdit], _ document: TextKitDocument, editor: Editor,
    dirtyElements: JSMap<NodeKey, Bool>?
  ) throws -> TextKitUpdate {
    var update = TextKitUpdate()
    update.base = textAttributes(format: 0, style: "", ancestors: [])
    update.isFull = dirtyElements == nil
    update.edits = try edits.map { edit in
      TextKitStyledEdit(
        edit: edit,
        runs: try edit.runs.map { run in
          let node = try getNodeByKey(run.key) as? TextNode
          return TextKitStyledRun(
            range: TextKitRange(location: run.location, length: run.length),
            attributes: try node.map(textAttributes(for:))
              ?? textAttributes(format: run.format, style: run.style, ancestors: []))
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
      if !(element is RootNode), try !element.isInline(), !hasBlockChild {
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
    let units = document.string.utf16
    for block in blocks {
      guard let span = document.span(block.getKey()) else { continue }
      var length = span.length
      if span.end < units.count, units[units.index(units.startIndex, offsetBy: span.end)] == 0x0A {
        length += 1
      }
      let ancestors = try ([block] + block.getParents()).reversed().map(TextKitElement.init)
      update.paragraphs.append(
        TextKitStyledParagraph(
          key: block.getKey(), range: TextKitRange(location: span.location, length: length),
          attributes: paragraphAttributes(blocks: ancestors),
          text: textAttributes(
            format: try block.getTextFormat(), style: try block.getTextStyle(),
            ancestors: ancestors)))
    }

    if !restyle.isEmpty {
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
            attributes: try textAttributes(for: node)))
      }
    }

    for key in editor.getDecorators().keys.sorted() {
      guard let decorator = try getNodeByKey(key) as? DecoratorNode,
        let span = document.span(key),
        update.isFull
          || finalRanges.contains(where: { $0.location <= span.location && span.location < $0.end })
      else { continue }
      update.decorators.append(
        TextKitDecoratorSpan(
          key: key, location: span.location, isInline: try decorator.isInline()))
    }
    return update
  }
}

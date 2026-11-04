import Lexical
import LexicalLink
import LexicalSelection

// Port of packages/lexical-markdown/src/MarkdownExport.ts. Offsets and
// "characters" are UTF-16 code units, as in JS; a character the JS reads
// with `text[i]` is a `UInt16` here, and `nil` is JS's `''` or `undefined`.

/// An `ElementTransformer`'s or `MultilineElementTransformer`'s `export`.
private typealias ElementExport = (LexicalNode, ExportChildren, BaseSelection?) throws -> String?

/// The `export`s of `[...byType.multilineElement, ...byType.element]`, less
/// the transformers without one.
private func elementExports(_ byType: TransformersByType) -> [ElementExport] {
  byType.multilineElement.compactMap(\.export) + byType.element.map(\.export)
}

/// Export only uses text formats that are responsible for single format
/// e.g. it will filter out *** (bold, italic) and instead use separate ** and *.
/// Make sure all text transformers that contain 'code' in their format are
/// at the end of the array. Otherwise, formatted code like
/// <strong><code>code</code></strong> will be exported as `**Bold Code**`, as
/// the code format will be applied first, and the bold format will be
/// applied second and thus skipped entirely, as the code format will prevent
/// any further formatting. (JS `sort` is stable, so ties keep their order.)
private func exportTextFormatTransformers(
  _ byType: TransformersByType
)
  -> [TextFormatTransformer]
{
  byType.textFormat
    .filter { $0.format.count == 1 }
    .enumerated()
    .sorted { a, b in
      let codeA = a.element.format.contains("code") ? 1 : 0
      let codeB = b.element.format.contains("code") ? 1 : 0
      return codeA != codeB ? codeA < codeB : a.offset < b.offset
    }
    .map(\.element)
}

/// `createMarkdownExport(transformers, shouldPreserveNewLines)`: renders
/// string from markdown. The selection is moved to the start after the
/// operation.
package func createMarkdownExport(
  _ transformers: [any Transformer], _ shouldPreserveNewLines: Bool = false
) -> (ElementNode?) throws -> String {
  let byType = transformersByType(transformers)
  let elementTransformers = elementExports(byType)
  let isNewlineDelimited = !shouldPreserveNewLines
  let textFormatTransformers = exportTextFormatTransformers(byType)

  return { node in
    try withBracketContext(byType.textMatch) {
      var output: [String] = []
      let children = try (node ?? getRoot()).getChildren()

      for i in children.indices {
        let child = children[i]
        let result = try exportTopLevelElements(
          child, elementTransformers, textFormatTransformers, byType.textMatch,
          shouldPreserveNewLines)

        if let result {
          // separate consecutive group of texts with a line break: eg.
          // ["hello", "world"] -> ["hello", "/nworld"]
          let separated =
            try isNewlineDelimited && i > 0 && !isEmptyParagraph(child)
            && !isEmptyParagraph(children[i - 1])
          output.append(separated ? "\n" + result : result)
        }
      }
      // Ensure consecutive groups of texts are at least \n\n apart while each
      // empty paragraph render as a newline.
      // Eg. ["hello", "", "", "hi", "\nworld"] -> "hello\n\n\nhi\n\nworld"
      return output.joined(separator: "\n")
    }
  }
}

/// `createSelectionMarkdownExport(transformers, shouldPreserveNewLines)`:
/// creates a markdown export function that only exports selected content.
/// Uses a recursive structure similar to `$appendNodesToHTML` to support
/// `extractWithChild` for proper handling of partial selections within
/// inline elements like links.
package func createSelectionMarkdownExport(
  _ transformers: [any Transformer], _ shouldPreserveNewLines: Bool = false
) -> (BaseSelection) throws -> String {
  let byType = transformersByType(transformers)
  let elementTransformers = elementExports(byType)
  let isNewlineDelimited = !shouldPreserveNewLines
  let textFormatTransformers = exportTextFormatTransformers(byType)

  return { exportedSelection in
    try withBracketContext(byType.textMatch) {
      // Every node asks whether it is selected, which reads the selection's
      // nodes. A read-only selection doesn't cache them, so this works on a
      // copy that does, instead of recomputing them for every node.
      // Its direction is cached too, since slicing a node asks for it.
      let nodes = try exportedSelection.getNodes()
      let selection = exportedSelection.clone()
      selection.setCachedNodes(nodes)
      if let range = selection as? RangeSelection {
        range._cachedIsBackward = try exportedSelection.isBackward()
      }
      var output: [String] = []
      // The separator depends on the previously *emitted* block, not on the
      // previous root child: unselected children produce no output, so keying
      // off the child index would prefix a newline to the first emitted block
      // whenever the selection starts below the top of the document.
      var previousExported: LexicalNode?

      for child in try getRoot().getChildren() {
        let (shouldInclude, markdown) = try processNodeForSelection(
          child, selection, elementTransformers, textFormatTransformers, byType.textMatch,
          shouldPreserveNewLines)

        if shouldInclude, let markdown {
          var separated = false
          if isNewlineDelimited, let previousExported {
            separated = try !isEmptyParagraph(child) && !isEmptyParagraph(previousExported)
          }
          output.append(separated ? "\n" + markdown : markdown)
          previousExported = child
        }
      }
      return output.joined(separator: "\n")
    }
  }
}

/// `$processNodeForSelection`.
private func processNodeForSelection(
  _ node: LexicalNode, _ selection: BaseSelection, _ elementTransformers: [ElementExport],
  _ textFormatTransformers: [TextFormatTransformer],
  _ textMatchTransformers: [TextMatchTransformer], _ shouldPreserveNewLines: Bool
) throws -> (shouldInclude: Bool, markdown: String?) {
  var shouldInclude = try node.isSelected(selection)

  // For element transformers (heading, quote, list, code block, etc.)
  for export in elementTransformers {
    let result = try export(
      node,
      { node_ in
        joinPieces(
          try exportChildrenForSelection(
            node_, selection, textFormatTransformers, textMatchTransformers,
            shouldPreserveNewLines
          ).pieces)
      },
      selection)

    if let result {
      if !shouldInclude {
        // Check if any descendant is selected
        if let element = node as? ElementNode {
          let childResult = try exportChildrenForSelection(
            element, selection, textFormatTransformers, textMatchTransformers,
            shouldPreserveNewLines)
          if childResult.shouldInclude {
            shouldInclude = true
          }
        }
      }
      return (shouldInclude, result)
    }
  }

  if let element = node as? ElementNode {
    let childResult = try exportChildrenForSelection(
      element, selection, textFormatTransformers, textMatchTransformers, shouldPreserveNewLines)
    return (shouldInclude || childResult.shouldInclude, joinPieces(childResult.pieces))
  } else if let decorator = node as? DecoratorNode {
    return (shouldInclude, try decorator.getTextContent())
  } else {
    return (shouldInclude, nil)
  }
}

/// `OpenTag`: a tag opened and not yet closed. A class, because JS shares
/// each one between the lists it is in and fills its `answers` in place.
private final class OpenTag {
  let format: TextFormatType
  let tag: String
  /// `$continuesTag`'s answers so far, by node key.
  var answers: [NodeKey: Bool] = [:]

  init(format: TextFormatType, tag: String) {
    self.format = format
    self.tag = tag
  }
}

/// An `OpenTag[]`, shared by reference as a JS array is.
private final class TagList {
  var tags: [OpenTag]

  init(_ tags: [OpenTag] = []) { self.tags = tags }

  func contains(tag: String) -> Bool { tags.contains { $0.tag == tag } }
}

/// `$exportChildrenForSelection`.
private func exportChildrenForSelection(
  _ node: ElementNode, _ selection: BaseSelection,
  _ textFormatTransformers: [TextFormatTransformer],
  _ textMatchTransformers: [TextMatchTransformer], _ shouldPreserveNewLines: Bool,
  _ unclosedTags: TagList? = nil, _ unclosableTags: TagList? = nil,
  _ output: MarkdownPieces = MarkdownPieces()
) throws -> (shouldInclude: Bool, pieces: MarkdownPieces) {
  let children = try node.getChildren()
  var anyChildIncluded = false

  let unclosedTags = unclosedTags ?? TagList()
  let unclosableTags = unclosableTags ?? TagList()

  mainLoop: for child in children {
    var childIncluded = try child.isSelected(selection)

    // Try text match transformers (links, etc.)
    for transformer in textMatchTransformers {
      guard let export = transformer.export else {
        continue
      }

      let result = try export(
        child,
        { parentNode in
          joinPieces(
            try exportChildrenForSelection(
              parentNode, selection, textFormatTransformers, textMatchTransformers,
              shouldPreserveNewLines, unclosedTags,
              TagList(unclosableTags.tags + unclosedTags.tags)
            ).pieces)
        },
        { textNode, _ in
          joinPieces(
            try exportTextFormat(
              textNode, try exportedText(textNode, selection), textFormatTransformers,
              unclosedTags, unclosableTags, shouldPreserveNewLines, selection))
        })

      if let result {
        // Check extractWithChild if this node wasn't directly selected
        if !childIncluded, let element = child as? ElementNode,
          try element.getChildren().contains(where: { try $0.isSelected(selection) }),
          try element.extractWithChild(element, selection, "html")
        {
          childIncluded = true
        }
        if childIncluded {
          pushPiece(output, .raw, result)
          anyChildIncluded = true
        }
        continue mainLoop
      }
    }

    if let lineBreak = child as? LineBreakNode {
      if childIncluded {
        pushPiece(output, .raw, try exportLineBreak(lineBreak))
        anyChildIncluded = true
      }
    } else if let text = child as? TextNode {
      if childIncluded {
        try exportTextFormat(
          text, try exportedText(text, selection), textFormatTransformers, unclosedTags,
          unclosableTags, shouldPreserveNewLines, selection, output)
        anyChildIncluded = true
      }
    } else if let element = child as? ElementNode {
      let childResult = try exportChildrenForSelection(
        element, selection, textFormatTransformers, textMatchTransformers,
        shouldPreserveNewLines, unclosedTags, unclosableTags, output)

      // extractWithChild: if child has selected descendants, ask parent if it
      // should be included
      if !childIncluded, childResult.shouldInclude,
        try element.extractWithChild(element, selection, "html")
      {
        childIncluded = true
      }

      // An element child writes pieces only when it has included ones.
      if childIncluded || childResult.shouldInclude {
        anyChildIncluded = true
      }
    } else if let decorator = child as? DecoratorNode {
      if childIncluded {
        pushPiece(output, .text, try decorator.getTextContent())
        anyChildIncluded = true
      }
    }
  }

  return (anyChildIncluded, output)
}

/// `$exportTopLevelElements`.
private func exportTopLevelElements(
  _ node: LexicalNode, _ elementTransformers: [ElementExport],
  _ textTransformersIndex: [TextFormatTransformer],
  _ textMatchTransformers: [TextMatchTransformer], _ shouldPreserveNewLines: Bool
) throws -> String? {
  for export in elementTransformers {
    let result = try export(
      node,
      { _node in
        try exportChildren(
          _node, textTransformersIndex, textMatchTransformers, nil, nil, shouldPreserveNewLines)
      },
      nil)

    if let result {
      return result
    }
  }

  if let element = node as? ElementNode {
    return try exportChildren(
      element, textTransformersIndex, textMatchTransformers, nil, nil, shouldPreserveNewLines)
  } else if let decorator = node as? DecoratorNode {
    return try decorator.getTextContent()
  } else {
    return nil
  }
}

/// `$exportChildren`.
private func exportChildren(
  _ node: ElementNode, _ textTransformersIndex: [TextFormatTransformer],
  _ textMatchTransformers: [TextMatchTransformer], _ unclosedTags: TagList? = nil,
  _ unclosableTags: TagList? = nil, _ shouldPreserveNewLines: Bool = false
) throws -> String {
  joinPieces(
    try exportChildPieces(
      node, textTransformersIndex, textMatchTransformers, unclosedTags, unclosableTags,
      shouldPreserveNewLines))
}

/// `$exportChildPieces`.
@discardableResult
private func exportChildPieces(
  _ node: ElementNode, _ textTransformersIndex: [TextFormatTransformer],
  _ textMatchTransformers: [TextMatchTransformer], _ unclosedTags: TagList? = nil,
  _ unclosableTags: TagList? = nil, _ shouldPreserveNewLines: Bool = false,
  _ output: MarkdownPieces = MarkdownPieces()
) throws -> MarkdownPieces {
  let children = try node.getChildren()
  // keep track of unclosed tags from the very beginning
  let unclosedTags = unclosedTags ?? TagList()
  let unclosableTags = unclosableTags ?? TagList()

  mainLoop: for child in children {
    for transformer in textMatchTransformers {
      guard let export = transformer.export else {
        continue
      }

      let result = try export(
        child,
        { parentNode in
          try exportChildren(
            parentNode, textTransformersIndex, textMatchTransformers, unclosedTags,
            // Add current unclosed tags to the list of unclosable tags - we
            // don't want nested tags from textmatch transformers to close the
            // outer ones, as that may result in invalid markdown.
            // E.g. **text [text**](https://lexical.io)
            // is invalid markdown, as the closing ** is inside the link.
            TagList(unclosableTags.tags + unclosedTags.tags), shouldPreserveNewLines)
        },
        { textNode, textContent in
          joinPieces(
            try exportTextFormat(
              textNode, textContent, textTransformersIndex, unclosedTags, unclosableTags,
              shouldPreserveNewLines, nil))
        })

      if let result {
        pushPiece(output, .raw, result)
        continue mainLoop
      }
    }

    if let lineBreak = child as? LineBreakNode {
      pushPiece(output, .raw, try exportLineBreak(lineBreak))
    } else if let text = child as? TextNode {
      try exportTextFormat(
        text, try text.getTextContent(), textTransformersIndex, unclosedTags, unclosableTags,
        shouldPreserveNewLines, nil, output)
    } else if let element = child as? ElementNode {
      // empty paragraph returns ""
      try exportChildPieces(
        element, textTransformersIndex, textMatchTransformers, unclosedTags, unclosableTags,
        shouldPreserveNewLines, output)
    } else if let decorator = child as? DecoratorNode {
      pushPiece(output, .text, try decorator.getTextContent())
    }
  }

  return output
}

/// `$exportLineBreak`.
private func exportLineBreak(_ node: LineBreakNode) throws -> String {
  try getState(node, hardLineBreakState) + "\n"
}

nonisolated(unsafe) private let PRESERVED_ESCAPE = JSRegExp(#"\\+|[*_`~]"#, "g")
nonisolated(unsafe) private let MARKER_CHARACTER = JSRegExp(#"[*_`~]"#)
nonisolated(unsafe) private let DEFAULT_ESCAPE = JSRegExp(#"([*_`~\\]|<(?=\S|$)|&(?=#|$))"#, "g")
nonisolated(unsafe) private let ESCAPED_CHARACTER = JSRegExp(#"[*_`~\\]"#)

/// `$exportTextFormat`.
@discardableResult
private func exportTextFormat(
  _ node: TextNode, _ textContent: String, _ textTransformers: [TextFormatTransformer],
  // unclosed tags include the markdown tags that haven't been closed yet, and
  // their associated formats
  _ unclosedTags: TagList, _ unclosableTags: TagList, _ shouldPreserveNewLines: Bool,
  // the selection being exported, which ends the text a tag can extend over
  _ selection: BaseSelection?,
  // where the exported pieces go
  _ pieces: MarkdownPieces = MarkdownPieces()
) throws -> MarkdownPieces {
  // This function handles the case of a string looking like this: "   foo   "
  // Where it would be invalid markdown to generate: "**   foo   **"
  // so the tags go inside the whitespace instead: "   **foo**   ".

  let isCode = try node.hasFormat("code")

  var output = textContent
  if !isCode {
    // Preserve literal backslashes when preserving source newlines, except
    // right before a character escaped here, where they would escape the
    // escape instead.
    // Default mode guards numeric references; named references keep legacy output.
    // Numeric escapes avoid changing emphasis flanking beside formatting.
    // Guard ambiguous '<' and '&' at node ends for constructs split across nodes.
    if shouldPreserveNewLines {
      // A whole run of backslashes is matched, so that one before no marker
      // is not tried again from each of its backslashes.
      output = output.jsReplace(PRESERVED_ESCAPE) { match in
        let escaped = match.text
        if !escaped.jsStartsWith("\\") {
          return "\\" + escaped
        }
        let next = match.input.jsCharAt(match.index + match.length)
        return next.map { MARKER_CHARACTER.test($0) } ?? false ? escaped + escaped : escaped
      }
    } else {
      output = try escapeLiteralBrackets(
        node,
        output.jsReplace(DEFAULT_ESCAPE) { match in
          ESCAPED_CHARACTER.test(match.text)
            ? "\\" + match.text : toCharacterReference(match.text)
        },
        selection)
    }
  }

  let holdsTag = canHoldTag(textContent)

  // the opening tags to be added to the result
  var openingTags = ""
  // the closing tags to be added to the result
  var closingTagsBefore = ""
  var closingTagsAfter = ""

  let prevNode = try getTextSibling(node, true)

  var applied = Set<TextFormatType>()

  for transformer in textTransformers {
    let format = transformer.format[0]
    let tag = transformer.tag

    // Inline code uses a content-derived fence handled below, not a static tag.
    if format == "code" {
      continue
    }

    // dedup applied formats
    if holdsTag, try node.hasFormat(format), !applied.contains(format) {
      applied.insert(format)

      // append the tag to openingTags, if it's not applied to the previous
      // nodes, or the nodes before that (which would result in an unclosed
      // tag). A whitespace-only previous node with the format keeps the tag
      // open without emitting it, so it counts as applied here.
      if try !hasFormat(prevNode, format) || !unclosedTags.contains(tag: tag) {
        unclosedTags.tags.append(OpenTag(format: format, tag: tag))
        openingTags += tag
      }
    }
  }

  // close any tags in the same order they were applied, if necessary
  var i = 0
  while i < unclosedTags.tags.count {
    let nodeHasFormat = try hasFormat(node, unclosedTags.tags[i].format)
    // prevent adding closing tag if later text with the format will do it
    if nodeHasFormat, try continuesTag(node, unclosedTags.tags[i], selection) {
      i += 1
      continue
    }

    // Shallow copy to avoid modifying the original array
    var unhandledUnclosedTags = unclosedTags.tags

    while unhandledUnclosedTags.count > i {
      let unclosedTag = unhandledUnclosedTags.removeLast()

      // If tag is unclosable, don't close it and leave it in the original
      // array, So that it can be closed when it's no longer unclosable
      if unclosableTags.contains(tag: unclosedTag.tag) {
        continue
      }

      if !nodeHasFormat {
        // Handles cases where the tag has not been closed before, e.g. if the
        // previous node was a text match transformer that did not account for
        // closing tags of the next node (e.g. a link)
        closingTagsBefore += unclosedTag.tag
      } else {
        closingTagsAfter += unclosedTag.tag
      }
      // Mutate the original array to remove the closed tag
      _ = unclosedTags.tags.popLast()
    }
    break
  }
  if isCode && !textContent.isEmpty {
    // Inline code is an atomic literal span with a content-derived fence, so
    // its whitespace stays inside the fence and other formats wrap around it.
    let (fence, padded) = getCodeSpanDelimiter(textContent)
    output = fence + padded + fence
  } else if !holdsTag {
    // If the node is entirely whitespace, we don't apply opening/closing tags
    // around it, but it must still output any closing tags.
    pushPiece(pieces, .closer, closingTagsBefore + closingTagsAfter)
    pushPiece(pieces, .text, output)
    return pieces
  }

  // Flanking Compliance: CommonMark flanking rules require formatting tags to
  // be adjacent to non-whitespace characters, so openingTags and
  // closingTagsAfter are placed INSIDE the whitespace boundaries!
  // Tags stay inside JavaScript whitespace, which holds CommonMark's and the
  // other characters some readers count as whitespace.
  // (`trimStart` and `trimEnd`, which keep this linear in long whitespace
  // runs.)
  let units = Array(output.utf16)
  var trimmedStart = 0
  while trimmedStart < units.count, isJSWhitespace(units[trimmedStart]) {
    trimmedStart += 1
  }
  var trimmedEnd = units.count
  while trimmedEnd > trimmedStart, isJSWhitespace(units[trimmedEnd - 1]) {
    trimmedEnd -= 1
  }
  let leadingSpace = String(decoding: units[..<trimmedStart], as: UTF16.self)
  let trimmedOutput = String(decoding: units[trimmedStart..<trimmedEnd], as: UTF16.self)
  let trailingSpace = String(decoding: units[trimmedEnd...], as: UTF16.self)
  pushPiece(pieces, .closer, closingTagsBefore)
  pushPiece(pieces, .text, leadingSpace)
  pushPiece(pieces, .opener, openingTags)
  // Inline code is a literal span: a character reference would show as is.
  pushPiece(pieces, isCode ? .raw : .text, trimmedOutput)
  pushPiece(pieces, .closer, closingTagsAfter)
  pushPiece(pieces, .text, trailingSpace)
  return pieces
}

/// `PieceKind`. A character reference can stand in for a character of
/// `text`, which is text and decorator content; `raw` is anything else, such
/// as code spans, line breaks and the output of transformers, whose syntax a
/// reference could break. `opener` and `closer` are format markers.
private enum PieceKind {
  case text, raw, opener, closer
}

/// `MarkdownPieces`: exported inline Markdown, with the format markers kept
/// apart from the text until `joinPieces` can see the characters on both
/// sides of each delimiter run: `values[i]` is a piece of the output and
/// `kinds[i]` what it is. A class, because JS passes the one object along to
/// be filled.
private final class MarkdownPieces {
  var kinds: [PieceKind] = []
  var values: [String] = []
}

/// `pushPiece`: adjacent text (and adjacent raw output) is merged, so that
/// each run of markers sits between the pieces whose characters a reader
/// sees beside it.
private func pushPiece(_ pieces: MarkdownPieces, _ kind: PieceKind, _ value: String) {
  let last = pieces.kinds.count - 1
  if value.isEmpty {
    return
  } else if last >= 0 && pieces.kinds[last] == kind && !isMarker(kind) {
    pieces.values[last] += value
  } else {
    pieces.kinds.append(kind)
    pieces.values.append(value)
  }
}

private func isMarker(_ kind: PieceKind?) -> Bool {
  kind == .opener || kind == .closer
}

/// `kinds[index]`, `nil` out of bounds as JS reads `undefined`.
private func kind(_ kinds: [PieceKind], _ index: Int) -> PieceKind? {
  index >= 0 && index < kinds.count ? kinds[index] : nil
}

/// The code unit of an ASCII character.
private func unit(_ character: Unicode.Scalar) -> UInt16 { UInt16(character.value) }

/// `characterAt(units, index).length`.
private func characterLength(_ units: [UInt16], _ index: Int) -> Int {
  index + 1 < units.count && UTF16.isLeadSurrogate(units[index])
    && UTF16.isTrailSurrogate(units[index + 1]) ? 2 : 1
}

/// `characterBefore(units, end).length`.
private func characterLengthBefore(_ units: [UInt16], _ end: Int) -> Int {
  end >= 2 && UTF16.isLeadSurrogate(units[end - 2]) && UTF16.isTrailSurrogate(units[end - 1])
    ? 2 : 1
}

/// `isLinkCharacter(character)`: `!LINK_END.test(character)`, where
/// `LINK_END` is `/[\s\p{Cc}<]/u`. Characters a URL or email can hold: GFM
/// ends an autolink at whitespace or `<`, and a URL's host and path may hold
/// any other character. This takes in the backslash that escapes a marker
/// too. (The JS tests one code unit at a time, so this tests the unit.)
private func isLinkCharacter(_ character: UInt16) -> Bool {
  !(isJSWhitespace(character) || character <= 0x1F || (0x7F...0x9F).contains(character)
    || character == unit("<"))
}

/// `linkRunStart(text)`: where the run of URL characters that ends `text`
/// starts.
private func linkRunStart(_ text: [UInt16]) -> Int {
  var start = text.count
  while start > 0 && isLinkCharacter(text[start - 1]) {
    start -= 1
  }
  return start
}

// What starts a URL: `http://`, `https://` (and cmark-gfm's `ftp://`) with
// a host after it, which may start with an emoji but not with other ASCII
// punctuation than `-` and `_`, and `www.`, which micromark links with
// anything after it. GFM readers start `www.` only at the start of a run or
// after `*_~(`, and a scheme after anything but a letter.
nonisolated(unsafe) private let LINK_PREFIX = JSRegExp(
  #"(?:https?|ftp):\/\/(?![\s\p{Cc}!-,./:-@[-^`{-~])|www\."#, "giu")
nonisolated(unsafe) private let STARTS_WITH_W = JSRegExp(#"^w"#, "i")
nonisolated(unsafe) private let ASCII_LETTER = JSRegExp(#"[a-z]"#, "i")

/// `startsLink(prefix, before)`, with `before` `nil` for `''`.
private func startsLink(_ prefix: String, _ before: UInt16?) -> Bool {
  guard let before else {
    // `'*_~('.includes('')` and `!/[a-z]/i.test('')` are both true.
    return true
  }
  let character = String(decoding: [before], as: UTF16.self)
  return STARTS_WITH_W.test(prefix)
    ? "*_~(".utf16.contains(before) : !ASCII_LETTER.test(character)
}

// Characters an email can hold (word characters, `.+@-`, and the backslash
// that escapes one), and what makes one: GFM needs a dot in its domain. An
// email takes in no format, and cmark-gfm finds it in the text with its
// references decoded, but micromark only where its `@` is written as is.
nonisolated(unsafe) private let ADDRESS = JSRegExp(#"[\w.+-]@[\w-]+\.[\w-]"#)
nonisolated(unsafe) private let LINKISH = JSRegExp(#"www\.|:\/\/|@"#, "i")
nonisolated(unsafe) private let BACKSLASHES = JSRegExp(#"\\"#, "g")
nonisolated(unsafe) private let DOT_OR_COLON = JSRegExp(#"[.:]"#)
// Characters that GFM readers leave out at the end of a URL.
private let LINK_TRAIL = Array("?!.,:*_~".utf16)

// Characters that a reference does not stand in for everywhere: CommonMark
// readers and HTML decode references to control characters, unpaired
// surrogates and noncharacters as U+FFFD (or remap them).
nonisolated(unsafe) private let UNREFERENCEABLE = JSRegExp(
  #"[\p{Cc}\p{Cs}\p{Noncharacter_Code_Point}]"#, "u")

/// `isReferenceable(character)`: whether a character reference can stand in
/// for `character` beside a marker.
private func isReferenceable(_ character: String?) -> Bool {
  guard let character else { return false }
  return isLetterForSomeReader(character) && !UNREFERENCEABLE.test(character)
}

/// `MarkerRun`: adjacent markers between the pieces at `before` and
/// `after`: whether the first delimiter run opens a tag, the marker
/// character after it within the markers (if any), whether the last
/// delimiter run closes a tag, and the marker character before it.
private struct MarkerRun {
  var before: Int
  var after: Int
  var opens: Bool
  var afterFirst: String?
  var closes: Bool
  var beforeLast: String?
}

// What a backslash before it escapes: ASCII punctuation or a line ending.
nonisolated(unsafe) private let ESCAPABLE = JSRegExp(#"^[!-/:-@[-`{-~\n\r]"#)

/// `joinPieces(pieces)`: joins exported pieces into Markdown. A delimiter
/// run between a word character and punctuation can only open or close on
/// the punctuation side (CommonMark flanking), so plain `a` before a bold `*`
/// cannot go out as `a**\***`. The word character beside such a run goes out
/// as a numeric reference instead, whose `&` or `;` is punctuation:
/// `&#97;**\***`.
private func joinPieces(_ pieces: MarkdownPieces) -> String {
  let kinds = pieces.kinds
  let values = pieces.values
  let text = values.joined()
  if kinds.allSatisfy({ $0 == .text }) {
    return text
  }
  let valueUnits = values.map { Array($0.utf16) }
  let (encodeStart, encodeEnd) = decideReferences(kinds, valueUnits)
  let textUnits = Array(text.utf16)
  // The code units of `text` that go out as references.
  var isReference = [UInt8](repeating: 0, count: textUnits.count)
  var base = 0
  for index in kinds.indices {
    let value = valueUnits[index]
    let start = encodeStart[index] != 0 ? characterLength(value, 0) : 0
    var end = value.count
    if encodeEnd[index] != 0 && end > start {
      end -= characterLengthBefore(value, end)
    }
    // A backslash left unescaped (when preserving newlines, or in decorator
    // text) would escape the reference, marker or raw punctuation written
    // after it, or end the line with a hard break.
    let next = kind(kinds, index + 1)
    if kinds[index] == .text
      && (isMarker(next) || (next == .raw && ESCAPABLE.test(values[index + 1])))
      && end > start && isEscaped(values[index], end)
    {
      end -= 1
    }
    for i in base..<(base + start) { isReference[i] = 1 }
    if end < value.count {
      for i in (base + end)..<(base + value.count) { isReference[i] = 1 }
    }
    base += value.count
  }
  spoilLinks(kinds, values, &isReference)
  var markdown: [UInt16] = []
  markdown.reserveCapacity(textUnits.count)
  var at = 0
  while at < textUnits.count {
    if isReference[at] == 1 {
      let length = characterLength(textUnits, at)
      let character = String(decoding: textUnits[at..<(at + length)], as: UTF16.self)
      markdown += toCharacterReference(character).utf16
      at += length
    } else {
      markdown.append(textUnits[at])
      at += 1
    }
  }
  return String(decoding: markdown, as: UTF16.self)
}

/// `maskRaw(value)`: raw output as the bare-URL scan sees it. It keeps
/// whitespace and `<`, which end a URL, `*_~(`, after which `www.` starts
/// one, and ASCII letters, before which no scheme starts one, as `x`, and is
/// `#` elsewhere. So no URL starts inside it, but one before it runs on into
/// it, as GFM readers run a URL on through a code span or a link.
/// (`value.replace(/[a-z]/gi, 'x').replace(/[^\sx<*_~(]/g, '#')`, one code
/// unit at a time as those regexes match.)
private func maskRaw(_ value: String) -> [UInt16] {
  let kept = Array("x<*_~(".utf16)
  return value.utf16.map { character in
    if (unit("a")...unit("z")).contains(character) || (unit("A")...unit("Z")).contains(character) {
      return unit("x")
    }
    return isJSWhitespace(character) || kept.contains(character) ? character : unit("#")
  }
}

/// `spoilLinks(kinds, values, isReference)`: finds the bare URLs and `www.`
/// hosts that GFM readers would link in the joined pieces, and that a
/// reference would land in or that would take in the markers, code span or
/// other raw output after them: the reader would keep the reference's
/// characters in the link, or the markers and code as text. Such a URL is
/// spoiled instead, by writing the `.` of `www.` or the `:` after its scheme
/// as a reference, so that no reader links it and its text and formats come
/// through. So is a URL that a reference before it would start.
/// `isReference` marks the code units of the joined text that go out as
/// references, and the ones that spoil a link are marked in it too.
private func spoilLinks(_ kinds: [PieceKind], _ values: [String], _ isReference: inout [UInt8]) {
  // Every URL and email starts with one of these. With no reference written,
  // only a URL that runs on to the end of its text, into the markers or raw
  // output after it, is spoiled.
  let hasReferences = isReference.contains(1)
  let anyLinkish = values.indices.contains { i in
    kinds[i] == .text && LINKISH.test(values[i])
      && (hasReferences
        || LINKISH.test(values[i].jsSlice(linkRunStart(Array(values[i].utf16)))))
  }
  if !anyLinkish {
    return
  }
  var units: [UInt16] = []
  // Whether each code unit is not text.
  var isFormat: [Bool] = []
  for i in values.indices {
    let value = kinds[i] == .raw ? maskRaw(values[i]) : Array(values[i].utf16)
    units += value
    isFormat += repeatElement(kinds[i] != .text, count: value.count)
  }
  let text = String(decoding: units, as: UTF16.self)
  let length = units.count
  // Running counts of referenced code units and of code units that are not
  // text, so that whether any lie in a span is a subtraction.
  var references = [Int](repeating: 0, count: length + 1)
  var formats = [Int](repeating: 0, count: length + 1)
  for i in 0..<length {
    references[i + 1] = references[i] + Int(isReference[i])
    formats[i + 1] = formats[i] + (isFormat[i] ? 1 : 0)
  }
  func has(_ sums: [Int], _ from: Int, _ to: Int) -> Bool {
    from < to && sums[to] > sums[from]
  }
  // `ADDRESS_CHARACTER.test(text[end])`, `/[\w.+@\\-]/`, on the code unit.
  func isAddressCharacter(_ character: UInt16) -> Bool {
    (unit("a")...unit("z")).contains(character) || (unit("A")...unit("Z")).contains(character)
      || isDigit(character) || "_.+@\\-".utf16.contains(character)
  }
  // A reference in an email would cut a micromark reader's link short, so
  // each `@` of one that holds a reference is spoiled.
  func spoilEmails(_ from: Int, _ to: Int) {
    var start = from
    while start < to {
      var end = start
      while end < to && isAddressCharacter(units[end]) {
        end += 1
      }
      if has(references, start, end)
        && ADDRESS.test(
          String(decoding: units[start..<end], as: UTF16.self).jsReplace(
            BACKSLASHES, template: ""))
      {
        for i in start..<end {
          // A raw backslash before an `@` would escape the reference's `&`.
          if units[i] == unit("@") && isReference[i] == 0 && !isFormat[i]
            && !isEscaped(text, i)
          {
            isReference[i] = 1
          }
        }
      }
      start = max(end, start + 1)
    }
  }
  var start = 0
  while start < length {
    var end = start
    while end < length && (isLinkCharacter(units[end]) || isReference[end] == 1) {
      end += 1
    }
    if end == start {
      start += 1
      continue
    }
    // A URL runs to the end of the run, less the punctuation at its end. It
    // takes in a backslash before that punctuation, which then no longer
    // escapes it, so the punctuation can join the markers after it.
    var trimmed = end
    while trimmed > start && LINK_TRAIL.contains(units[trimmed - 1]) {
      trimmed -= 1
    }
    if isEscaped(text, trimmed) && has(formats, trimmed, end) {
      trimmed = end
    }
    // Where the text that is not part of a URL starts, which may hold an
    // email, and where the next URL may start.
    var rest = start
    let run = String(decoding: units[start..<end], as: UTF16.self)
    for prefix in run.jsMatchAll(LINK_PREFIX) {
      let at = start + prefix.index
      if at < rest {
        continue
      }
      // A reference before the prefix, or a spoiled `.` or `:`, puts `;`
      // there.
      let before: UInt16? = at > start ? units[at - 1] : nil
      let wasLink = at == start || startsLink(prefix.text, before)
      if at > start && !startsLink(prefix.text, isReference[at - 1] == 1 ? unit(";") : before) {
        continue
      }
      spoilEmails(rest, at)
      if wasLink && !has(references, at, end) && !has(formats, at, trimmed) {
        rest = end
        break
      }
      // A reference in the prefix spoils it already. A URL whose `.` or `:`
      // is a custom format's marker is left as it is.
      let spoil = at + prefix.text.jsSearch(DOT_OR_COLON)
      if !has(references, at, at + prefix.length) {
        if isFormat[spoil] {
          rest = end
          break
        }
        isReference[spoil] = 1
      }
      rest = at + prefix.length
    }
    spoilEmails(rest, end)
    start = end
  }
}

/// `scanDelimiterRun(kinds, values, from, to, step, kind)`: walks the
/// delimiter run at the edge of the markers from `from` to `to`, forward
/// when `step` is 1 and backward when it is -1. Returns whether a marker of
/// `kind` is part of that run, and the character past it within the
/// markers, if any.
private func scanDelimiterRun(
  _ kinds: [PieceKind], _ values: [[UInt16]], _ from: Int, _ to: Int, _ step: Int,
  _ kind: PieceKind
) -> (Bool, String?) {
  let edge = values[from]
  let delimiter = step == 1 ? edge.first : edge.last
  var hasKind = false
  var i = from
  while i != to + step {
    let value = values[i]
    var c = step == 1 ? 0 : value.count - 1
    while c >= 0 && c < value.count {
      if value[c] != delimiter {
        return (hasKind, String(decoding: [value[c]], as: UTF16.self))
      }
      hasKind = hasKind || kinds[i] == kind
      c += step
    }
    i += step
  }
  return (hasKind, nil)
}

/// `decideReferences(kinds, values)`: marks the text pieces whose first (in
/// the first array) or last character has to go out as a reference for the
/// marker run beside it to open or close.
private func decideReferences(_ kinds: [PieceKind], _ values: [[UInt16]]) -> ([UInt8], [UInt8]) {
  let count = kinds.count
  var encodeStart = [UInt8](repeating: 0, count: count)
  var encodeEnd = [UInt8](repeating: 0, count: count)
  var runs: [MarkerRun] = []
  // The run that follows and the run that precedes each piece, or -1. (JS
  // writes these through a typed array, which ignores a write out of its
  // bounds.)
  var runAfter = [Int](repeating: -1, count: count)
  var runBefore = [Int](repeating: -1, count: count)
  var start = 0
  while start < count {
    if isMarker(kinds[start]) {
      var end = start
      while isMarker(kind(kinds, end + 1)) {
        end += 1
      }
      if start - 1 >= 0 { runAfter[start - 1] = runs.count }
      if end + 1 < count { runBefore[end + 1] = runs.count }
      // Markers of one character form one delimiter run, and only the first
      // and the last delimiter run touch text.
      let (opens, afterFirst) = scanDelimiterRun(kinds, values, start, end, 1, .opener)
      let (closes, beforeLast) = scanDelimiterRun(kinds, values, end, start, -1, .closer)
      runs.append(
        MarkerRun(
          before: start - 1, after: end + 1, opens: opens, afterFirst: afterFirst,
          closes: closes, beforeLast: beforeLast))
      start = end
    }
    start += 1
  }
  func firstChar(_ index: Int) -> String {
    let value = values[index]
    return String(decoding: value[0..<characterLength(value, 0)], as: UTF16.self)
  }
  func lastChar(_ index: Int) -> String {
    let value = values[index]
    return String(
      decoding: value[(value.count - characterLengthBefore(value, value.count))...],
      as: UTF16.self)
  }
  // A one-character piece written as a reference reads as punctuation at
  // both ends: `&#97;`.
  func isOneCharacter(_ index: Int) -> Bool {
    characterLength(values[index], 0) == values[index].count
  }
  func isEncoded(_ index: Int) -> Bool {
    (encodeStart[index] != 0 || encodeEnd[index] != 0) && isOneCharacter(index)
  }
  func startChar(_ index: Int) -> String? {
    index >= count || index < 0
      ? nil : encodeStart[index] != 0 || isEncoded(index) ? "&" : firstChar(index)
  }
  func endChar(_ index: Int) -> String? {
    index < 0 || index >= count
      ? nil : encodeEnd[index] != 0 || isEncoded(index) ? ";" : lastChar(index)
  }
  // Whether the last (or first) character of the piece at `index` can go out
  // as a reference.
  func canReference(_ index: Int, _ atEnd: Bool) -> Bool {
    kind(kinds, index) == .text && isReferenceable(atEnd ? endChar(index) : startChar(index))
  }
  // Writing a one-character piece as a reference makes its other edge
  // punctuation too. That can stop the delimiter run touching that edge from
  // flanking unless the piece past it is written as a reference as well, and
  // so on along a chain of one-character pieces. Where the chain ends at
  // something that cannot be (raw output, say), writing the first reference
  // would only trade one lost format for another, so it is not written.
  // Each piece's answer is remembered for each direction, so each piece is
  // walked at most twice. Raw output and unreferenceable characters stay
  // what they are. A remembered block could only lift if a piece on its
  // chain were later written as a reference from the other side, which takes
  // a run of one marker character that both closes and opens; then a
  // reference that would have kept a format is not written.
  // 0 while unknown, then 1 when it spreads and 2 when it is blocked.
  var spreads = [[UInt8](repeating: 0, count: count), [UInt8](repeating: 0, count: count)]
  func canSpread(_ index: Int, _ leftward: Bool) -> Bool {
    let direction = leftward ? 1 : 0
    var visited: [Int] = []
    var result = true
    var piece = index
    while true {
      let known = spreads[direction][piece]
      if known != 0 {
        result = known == 1
        break
      }
      visited.append(piece)
      let runIndex = (leftward ? runBefore : runAfter)[piece]
      if runIndex == -1 {
        break
      }
      let run = runs[runIndex]
      let next = leftward ? run.before : run.after
      // Only a run of a single delimiter character touches both pieces.
      if !(leftward
        ? run.opens && run.afterFirst == nil : run.closes && run.beforeLast == nil)
        || isPortablyFlankedBy(
          leftward ? "&" : ";", leftward ? endChar(next) : startChar(next))
      {
        break
      }
      if !canReference(next, leftward) {
        result = false
        break
      }
      if !isOneCharacter(next) {
        break
      }
      piece = next
    }
    for piece in visited {
      spreads[direction][piece] = result ? 1 : 2
    }
    return result
  }
  // Whether a reference beside the run, before it when `opening` and after it
  // otherwise, lets its first delimiter run open (or its last close) where it
  // could not, and whether that reference can be written. `_` between two
  // letters cannot open either way.
  func isHelpedBy(_ run: MarkerRun, _ opening: Bool) -> Bool {
    let piece = opening ? run.before : run.after
    if !(opening ? run.opens : run.closes) || !canReference(piece, opening) {
      return false
    }
    func side(_ sides: EmphasisSides) -> Bool { opening ? sides.canOpen : sides.canClose }
    let markerUnit = opening ? values[run.before + 1].first! : values[run.after - 1].last!
    let marker = String(decoding: [markerUnit], as: UTF16.self)
    let previous = (opening ? nil : run.beforeLast) ?? endChar(run.before)
    let next = (opening ? run.afterFirst : nil) ?? startChar(run.after)
    return !side(getPortableEmphasisSides(marker, previous, next))
      && side(
        getPortableEmphasisSides(marker, opening ? ";" : previous, opening ? next : "&"))
      && (!isOneCharacter(piece) || canSpread(piece, opening))
  }
  // Text does not escape every tag character (the `=` of `==`, say), and one
  // right beside a marker of the same character would join its delimiter
  // run, so it goes out as a reference.
  for i in 0..<count where kinds[i] == .text {
    let value = values[i]
    if isMarker(kind(kinds, i + 1)) && value.last == values[i + 1].first
      && !isEscaped(String(decoding: value, as: UTF16.self), value.count - 1)
    {
      encodeEnd[i] = 1
    }
    if isMarker(kind(kinds, i - 1)) && value.first == values[i - 1].last {
      encodeStart[i] = 1
    }
  }
  var pending = Array(runs.indices)
  // Writing a one-character piece as a reference changes what the run on its
  // other side sees, so that run is decided again. Each piece changes at most
  // twice, so this stays linear.
  while let index = pending.popLast() {
    let run = runs[index]
    if isHelpedBy(run, true) {
      encodeEnd[run.before] = 1
      if isEncoded(run.before) && runBefore[run.before] != -1 {
        pending.append(runBefore[run.before])
      }
    }
    if isHelpedBy(run, false) {
      encodeStart[run.after] = 1
      // An opener that sees the same character, of a run that both closes
      // and opens, may now need its own reference.
      if run.opens && run.afterFirst == nil {
        pending.append(index)
      }
      if isEncoded(run.after) && runAfter[run.after] != -1 {
        pending.append(runAfter[run.after])
      }
    }
  }
  return (encodeStart, encodeEnd)
}

nonisolated(unsafe) private let BRACKET = JSRegExp(#"[[\]]"#)

/// `$escapeLiteralBrackets(node, text, selection)`: escape only the literal
/// brackets that would otherwise turn into Markdown syntax, and leave every
/// other bracket as typed, so `[1]` citations and `array[0]` keep their
/// exported text.
///
/// Outside a link a bracket matters in three places, and escaping the `]` is
/// enough in each:
///
/// - a `]` followed by `(` closes the text of an inline link or image
///   (https://spec.commonmark.org/0.31.2/#inline-link);
/// - a `]` followed by `:` closes the label of a link reference definition
///   when its `[` starts a line after at most three spaces, with no bracket in
///   between (https://spec.commonmark.org/0.31.2/#link-reference-definitions);
/// - `[ ]`, `[x]` or `[]` at the start of a line, followed by whitespace, is
///   read back as a check list item by `CHECK_LIST`, and as a task list item
///   by GFM.
///
/// Inside a link's text, brackets are allowed only as matched pairs
/// (https://spec.commonmark.org/0.31.2/#link-text), and a pair followed by `(`
/// would read as a nested link. Those brackets, and unmatched ones, are
/// escaped; other matched pairs stay as typed.
///
/// The characters around this node come from the exported text next to it in
/// the same block, read through inline elements other than links, so a
/// construct split across text nodes or around a comment mark is still caught.
/// Where that text is unknown (a decorator) the bracket is escaped.
private func escapeLiteralBrackets(
  _ node: TextNode, _ text: String, _ selection: BaseSelection?
)
  throws -> String
{
  if !BRACKET.test(text) {
    return text
  }
  let isInLink = try isInsideLink(node)
  let escaped: Set<Int>
  if isInLink {
    escaped = linkTextBracketsToEscape(text, try nextExportedChar(node, selection))
  } else {
    let before = try lineStateBefore(node, selection)
    escaped = try literalBracketsToEscape(
      before, text, try nextExportedChar(node, selection),
      { budget in try labelClosesInCodeAfter(node, selection, budget) })
  }
  if escaped.isEmpty {
    return text
  }
  var output: [UInt16] = []
  for (index, character) in text.utf16.enumerated() {
    if escaped.contains(index) {
      output += "&#\(character);".utf16
    } else {
      output.append(character)
    }
  }
  return String(decoding: output, as: UTF16.self)
}

/// `BARRIER`: stands for exported text that cannot be part of a label: a
/// link.
private let BARRIER: UInt16 = 0x01
/// `UNKNOWN`: stands for exported text this pass cannot see: a decorator, or
/// a node a text match transformer writes in its own syntax.
private let UNKNOWN: UInt16 = 0x02
/// `MAX_LABEL_DISTANCE`: how far a label's `]` can be from its `[`: 999
/// characters of label (https://spec.commonmark.org/0.31.2/#link-label) and
/// the `[` itself.
private let MAX_LABEL_DISTANCE = 1000

/// `asLiteral(character)` for one code unit: typed text that matches a
/// sentinel is read as an ordinary character.
private func asLiteral(_ character: UInt16) -> UInt16 {
  character == BARRIER || character == UNKNOWN ? 0xFFFD : character
}

/// `asLiteral(text)`.
private func asLiteral(_ text: String) -> String {
  text.utf16.contains(where: { $0 == BARRIER || $0 == UNKNOWN })
    ? String(decoding: text.utf16.map(asLiteral), as: UTF16.self) : text
}

/// `LinePhase`: where a line of exported text stands so far: only whitespace
/// and the markers a block container can start with (`>`, `-`, `*`, `+`,
/// `1.`, `1)`), each followed by whitespace, in the middle of such a marker,
/// or past them. A `[` that comes while the line is still at its start may
/// begin a link reference definition or a task marker, here or inside the
/// container the markers would open.
private enum LinePhase {
  case start, bullet, digits, text
}

/// `LineState`: what the exported text so far in a block says about the
/// next character: the phase of its line, and how far back the nearest `[`,
/// link or unknown text is, with the character after it and whether it came
/// at a line start. `openerChar` and `openerNext` are `nil` for `''`.
private struct LineState {
  var phase: LinePhase
  /// -1 when there is none within MAX_LABEL_DISTANCE.
  var openerDistance: Int
  var openerChar: UInt16?
  var openerNext: UInt16?
  var openerAtLineStart: Bool
}

private let BLOCK_START = LineState(
  phase: .start, openerDistance: -1, openerChar: nil, openerNext: nil,
  openerAtLineStart: false)

private func isDigit(_ character: UInt16) -> Bool {
  character >= unit("0") && character <= unit("9")
}

/// `nextLinePhase(phase, character)`. A character of more than one code
/// unit is passed as its first, which compares as the JS string does.
private func nextLinePhase(_ phase: LinePhase, _ character: UInt16) -> LinePhase {
  if character == UNKNOWN {
    // Unknown text may end with a line break, so what follows may start a line.
    return .start
  }
  // `CHECK_LIST` and the list transformers accept any `\s` as indentation.
  let isSpace = isJSWhitespace(character)
  switch phase {
  case .start:
    return isSpace || character == unit(">")
      ? .start
      : character == unit("-") || character == unit("*") || character == unit("+")
        ? .bullet : isDigit(character) ? .digits : .text
  case .bullet:
    return isSpace ? .start : .text
  case .digits:
    return isDigit(character)
      ? .digits : character == unit(".") || character == unit(")") ? .bullet : .text
  case .text:
    return .text
  }
}

/// `advanceLineState(state, character)`: advance `state` over one character
/// of exported text, in place.
private func advanceLineState(_ state: inout LineState, _ character: UInt16) {
  if state.openerDistance != -1 {
    state.openerDistance += 1
    if state.openerDistance == 1 {
      state.openerNext = character
    } else if state.openerDistance > MAX_LABEL_DISTANCE {
      state.openerDistance = -1
    }
  }
  if character == unit("\n") {
    state.phase = .start
    return
  }
  if character == unit("[") || character == BARRIER || character == UNKNOWN {
    // Unknown text inside a label that starts a line keeps its line start.
    let keepsLineStart =
      character == UNKNOWN && state.openerDistance != -1 && state.openerChar != BARRIER
      && state.openerAtLineStart
    state.openerDistance = 0
    state.openerChar = character
    state.openerNext = nil
    state.openerAtLineStart = state.phase == .start || keepsLineStart
  }
  state.phase = nextLinePhase(state.phase, character)
}

/// `linkTextBracketsToEscape(text, nextChar)`.
private func linkTextBracketsToEscape(_ text: String, _ nextChar: UInt16?) -> Set<Int> {
  let units = Array(text.utf16)
  var escaped = Set<Int>()
  var openers: [Int] = []
  for index in units.indices {
    let character = units[index]
    if character == unit("[") {
      openers.append(index)
    } else if character == unit("]") {
      let opener = openers.popLast()
      let after = index + 1 < units.count ? units[index + 1] : nextChar
      if let opener {
        if after == unit("(") || after == UNKNOWN {
          escaped.insert(opener)
          escaped.insert(index)
        }
      } else {
        escaped.insert(index)
      }
    }
  }
  for opener in openers {
    escaped.insert(opener)
  }
  return escaped
}

/// `$literalBracketsToEscape(before, text, nextChar, $labelClosesAfter)`:
/// the indexes in `text` of the `]` characters to escape outside a link,
/// given the state of the exported text before it in its block and the
/// exported character after it.
private func literalBracketsToEscape(
  _ before: LineState, _ text: String, _ nextChar: UInt16?,
  _ labelClosesAfter: (Int) throws -> Bool
) throws -> Set<Int> {
  let units = Array(text.utf16)
  var escaped = Set<Int>()
  var lineStartOpeners: [Int] = []
  var state = before
  for index in units.indices {
    let character = asLiteral(units[index])
    if character == unit("]") {
      let after = index + 1 < units.count ? units[index + 1] : nextChar
      // The distance to the nearest opener, counting this `]`.
      let distance = state.openerDistance == -1 ? -1 : state.openerDistance + 1
      let opensLine = distance != -1 && state.openerChar != BARRIER && state.openerAtLineStart
      let isUnknown = state.openerChar == UNKNOWN
      let afterIsSpace = after.map(isJSWhitespace) ?? false
      let nextIsTaskMark =
        state.openerNext.map { isJSWhitespace($0) || $0 == unit("x") || $0 == unit("X") }
        ?? false
      if after == unit("(") || after == UNKNOWN || (opensLine && after == unit(":"))
        || (opensLine && afterIsSpace
          && (isUnknown || distance == 1 || (distance == 2 && nextIsTaskMark)))
      {
        escaped.insert(index)
      }
    } else if character == unit("[") && state.phase == .start {
      lineStartOpeners.append(index)
    }
    advanceLineState(&state, character)
  }
  for index in lineStartOpeners {
    if try labelClosesInCode(units, index, escaped, labelClosesAfter) {
      // The `]` that would close this label is in inline code, where it
      // cannot be escaped, so the `[` is.
      escaped.insert(index)
    }
  }
  return escaped
}

/// `$labelClosesInCode(text, index, escaped, $rest)`: whether the label
/// opened by the `[` at `index` in `text` would close at a `]` inside inline
/// code followed by `:`, making a link reference definition. A label ends at
/// the first `]` that is not escaped and cannot hold another `[`
/// (https://spec.commonmark.org/0.31.2/#link-label), and definitions are
/// read before code spans. `escaped` holds the `]` this pass escapes, which
/// stay inside the label. `rest` reads past the end of `text`.
private func labelClosesInCode(
  _ text: [UInt16], _ index: Int, _ escaped: Set<Int>, _ rest: (Int) throws -> Bool
) throws -> Bool {
  for next in (index + 1)..<max(index + 1, text.count) {
    if text[next] == unit("[") || (text[next] == unit("]") && !escaped.contains(next)) {
      return false
    }
  }
  let budget = MAX_LABEL_DISTANCE - (text.count - index)
  return try budget > 0 && rest(budget)
}

/// `$labelClosesInCodeAfter(node, selection, budget)`: continues
/// `labelClosesInCode` past `node`: reads the leaves after it in its block
/// until inline code holding a bracket, a `[`, a link, the end of the block
/// or `budget` characters. Unknown text may hold the `]`, so it answers yes.
private func labelClosesInCodeAfter(
  _ node: LexicalNode, _ selection: BaseSelection?, _ budget: Int
) throws -> Bool {
  var remaining = budget
  var leaf = try nextLeaf(node, selection)
  while let current = leaf, remaining > 0 {
    let kind = try exportKind(current)
    if kind == .link {
      return false
    } else if kind == .unknown {
      return true
    } else if let textNode = current as? TextNode {
      let text = asLiteral(try exportedText(textNode, selection))
      if try textNode.hasFormat("code") {
        let bracket = text.jsSearch(BRACKET)
        if bracket != -1 {
          return text.utf16Unit(at: bracket) == unit("]")
            && text.utf16Unit(at: bracket + 1) == unit(":")
        }
      } else if text.utf16.contains(unit("[")) {
        // A `]` in plain text may be one this pass escapes, which leaves the
        // label open, so only a `[` ends the search here.
        return false
      }
      remaining -= text.utf16.count
    } else if current is LineBreakNode {
      remaining -= 1
    }
    leaf = try nextLeaf(current, selection)
  }
  return false
}

/// `BracketContext`: the state kept for the export in progress: its text
/// match transformers, how each element and text node they might write is
/// exported, and the line state after each leaf read so far, so that each
/// leaf is read once however many brackets follow it. JS keeps it in module
/// scope; here it is per thread, in `ThreadContext.markdownBracketContext`.
private final class BracketContext {
  let textMatchTransformers: [TextMatchTransformer]
  var exportKinds: [NodeKey: ExportKind] = [:]
  var lineStates: [NodeKey: LineState] = [:]

  init(_ textMatchTransformers: [TextMatchTransformer]) {
    self.textMatchTransformers = textMatchTransformers
  }
}

/// `bracketContext`.
private var bracketContext: BracketContext? {
  ThreadContext.current.markdownBracketContext as? BracketContext
}

/// `withBracketContext(textMatchTransformers, callback)`.
private func withBracketContext<T>(
  _ textMatchTransformers: [TextMatchTransformer], _ callback: () throws -> T
) rethrows -> T {
  let thread = ThreadContext.current
  let previous = thread.markdownBracketContext
  thread.markdownBracketContext = BracketContext(textMatchTransformers)
  defer { thread.markdownBracketContext = previous }
  return try callback()
}

/// `ExportKind`: how a node's exported text reads around a bracket: a link
/// written as `[text](url)`, text this pass reads, an inline element whose
/// children are exported in place, or something written in a syntax it
/// cannot see.
private enum ExportKind {
  case link, text, transparent, unknown
}

/// `$exportKind(node)`.
private func exportKind(_ node: LexicalNode) throws -> ExportKind {
  if node is LineBreakNode {
    return .text
  }
  if let element = node as? ElementNode, try !element.isInline() {
    return .text
  }
  if !(node is TextNode) && !(node is ElementNode) {
    // A decorator is exported as its text content, or by a transformer.
    return .unknown
  }
  let context = bracketContext
  if let cached = context?.exportKinds[node.getKey()] {
    return cached
  }
  // What a transformer writes for the node, with its children left out, or
  // nil when none writes it.
  var written: String?
  if let context {
    for transformer in context.textMatchTransformers {
      guard let export = transformer.export else {
        continue
      }
      do {
        written = try export(node, { _ in "" }, { _, _ in "" })
      } catch {
        written = String(decoding: [UNKNOWN], as: UTF16.self)
      }
      if written != nil {
        break
      }
    }
  } else if isLinkNode(node) && !isAutoLinkNode(node) {
    written = "[]()"
  }
  // An element written as `[children](...)` reads like a link, whatever its
  // class; anything else a transformer writes is unknown.
  let kind: ExportKind =
    if let written {
      node is ElementNode && written.jsStartsWith("[") ? .link : .unknown
    } else {
      node is TextNode ? .text : .transparent
    }
  context?.exportKinds[node.getKey()] = kind
  return kind
}

/// `$isInsideLink(node)`.
private func isInsideLink(_ node: LexicalNode) throws -> Bool {
  var parent = try node.getParent()
  while let current = parent {
    if try exportKind(current) == .link {
      return true
    }
    if try !current.isInline() {
      return false
    }
    parent = try current.getParent()
  }
  return false
}

/// `$isInlineElement(node)`.
private func isInlineElement(_ node: LexicalNode?) throws -> Bool {
  guard let element = node as? ElementNode else { return false }
  return try element.isInline()
}

/// `$isTransparentInline(node)`: whether `node` is an inline element whose
/// exported text is its children's.
private func isTransparentInline(_ node: LexicalNode?) throws -> Bool {
  guard let element = node as? ElementNode else { return false }
  return try element.isInline() && exportKind(element) == .transparent
}

/// `$nextExportedChar(node, selection)`: the first character of the exported
/// text after `node` in its block: text read through inline elements whose
/// children are exported in place, `]` at the end of a link's text, `[` where
/// a link starts, a newline for a line break, a backtick for inline code, and
/// UNKNOWN for anything written in a syntax this pass cannot see. `nil`
/// (JS `''`) at the end of the block or of the exported selection.
private func nextExportedChar(_ node: LexicalNode, _ selection: BaseSelection?) throws -> UInt16? {
  var current: LexicalNode = node
  while true {
    var sibling = try current.getNextSibling()
    while sibling == nil {
      let parent = try current.getParent()
      if let parent, try exportKind(parent) == .link {
        return unit("]")
      }
      guard let parent, try isTransparentInline(parent) else {
        // Leaving an inline element written in its own syntax leads into
        // text this pass cannot see.
        return try isInlineElement(parent) ? UNKNOWN : nil
      }
      current = parent
      sibling = try current.getNextSibling()
    }
    var next = sibling!
    while let element = next as? ElementNode, try isTransparentInline(element),
      try element.getFirstChild() != nil
    {
      next = try element.getFirstChildOrThrow()
    }
    if let selection, try !next.isSelected(selection) {
      return nil
    }
    let kind = try exportKind(next)
    if kind == .link {
      return unit("[")
    } else if kind == .unknown {
      return UNKNOWN
    } else if let textNode = next as? TextNode {
      let exported = try exportedText(textNode, selection)
      if let first = exported.utf16.first {
        return try textNode.hasFormat("code") ? unit("`") : asLiteral(first)
      }
    } else if next is LineBreakNode {
      return unit("\n")
    }
    current = next
  }
}

/// `$nextLeaf(node, selection)`: the leaf exported just after `node` in its
/// block, read through inline elements whose children are exported in place,
/// or nil at the end of the block or of the exported selection.
private func nextLeaf(_ node: LexicalNode, _ selection: BaseSelection?) throws -> LexicalNode? {
  var current: LexicalNode = node
  var sibling = try current.getNextSibling()
  while sibling == nil {
    let parent = try current.getParent()
    guard let parent, try isTransparentInline(parent) else {
      // An inline element written in its own syntax stands for that syntax.
      return try isInlineElement(parent) ? parent : nil
    }
    current = parent
    sibling = try current.getNextSibling()
  }
  var next = sibling!
  while let element = next as? ElementNode, try isTransparentInline(element),
    try element.getFirstChild() != nil
  {
    next = try element.getFirstChildOrThrow()
  }
  if let selection, try !next.isSelected(selection) {
    return nil
  }
  return next
}

/// `$previousLeaf(node, selection)`: the leaf exported just before `node` in
/// its block, read through inline elements whose children are exported in
/// place, or nil at the start of the block or of the exported selection.
private func previousLeaf(
  _ node: LexicalNode, _ selection: BaseSelection?
) throws
  -> LexicalNode?
{
  var current: LexicalNode = node
  var sibling = try current.getPreviousSibling()
  while sibling == nil {
    let parent = try current.getParent()
    guard let parent, try isTransparentInline(parent) else {
      // An inline element written in its own syntax stands for that syntax.
      return try isInlineElement(parent) ? parent : nil
    }
    current = parent
    sibling = try current.getPreviousSibling()
  }
  var previous = sibling!
  while let element = previous as? ElementNode, try isTransparentInline(element),
    try element.getLastChild() != nil
  {
    previous = try element.getLastChildOrThrow()
  }
  if let selection, try !previous.isSelected(selection) {
    return nil
  }
  return previous
}

/// `$leafText(leaf, selection)`: a leaf's exported text in the terms
/// LineState reads: a line break as a newline, inline code with its brackets
/// and spaces masked, a link as BARRIER and anything this pass cannot see as
/// UNKNOWN.
private func leafText(_ leaf: LexicalNode, _ selection: BaseSelection?) throws -> String {
  let kind = try exportKind(leaf)
  if kind == .link {
    return String(decoding: [BARRIER], as: UTF16.self)
  } else if kind == .unknown {
    return String(decoding: [UNKNOWN], as: UTF16.self)
  } else if let textNode = leaf as? TextNode {
    let text = asLiteral(try exportedText(textNode, selection))
    guard try textNode.hasFormat("code") else { return text }
    // `text.replace(/[[\] \t]/g, '`')`
    let masked = Array("[] \t".utf16)
    return String(
      decoding: text.utf16.map { masked.contains($0) ? unit("`") : $0 }, as: UTF16.self)
  } else if leaf is LineBreakNode {
    return "\n"
  }
  return ""
}

/// `$lineStateBefore(node, selection)`.
private func lineStateBefore(
  _ node: LexicalNode, _ selection: BaseSelection?
) throws
  -> LineState
{
  // Walk back to a leaf whose state is known, then read forward from it.
  let context = bracketContext
  var pending: [LexicalNode] = []
  var state = BLOCK_START
  var leaf = try previousLeaf(node, selection)
  while let current = leaf {
    if let cached = context?.lineStates[current.getKey()] {
      state = cached
      break
    }
    pending.append(current)
    leaf = try previousLeaf(current, selection)
  }
  for current in pending.reversed() {
    var next = state
    // `for (const character of text)` steps by code point; a character is
    // passed as its first code unit.
    let units = Array(try leafText(current, selection).utf16)
    var index = 0
    while index < units.count {
      advanceLineState(&next, units[index])
      index += characterLength(units, index)
    }
    context?.lineStates[current.getKey()] = next
    state = next
  }
  return state
}

/// `getTextSibling(node, backward)`.
private func getTextSibling(_ node: TextNode, _ backward: Bool) throws -> TextNode? {
  let sibling = try backward ? node.getPreviousSibling() : node.getNextSibling()
  return sibling as? TextNode
}

/// `$continuesTag(node, tag, selection)`: whether `tag` carries on past
/// `node`, which has its format, to exported text that can hold it.
/// Whitespace-only siblings cannot (the tag must close before the space), so
/// they are looked past while they keep the format. Text outside the
/// exported selection does not count. Every whitespace node the walk passes
/// over shares its answer, so that is remembered on the tag and each run is
/// walked once, whatever order its nodes are asked in.
private func continuesTag(
  _ node: TextNode, _ tag: OpenTag, _ selection: BaseSelection?
) throws
  -> Bool
{
  if let known = tag.answers[node.getKey()] {
    return known
  }
  var passed: [TextNode] = []
  var next = try getTextSibling(node, false)
  var result = false
  while let current = next, try current.hasFormat(tag.format),
    try selection.map({ try current.isSelected($0) }) ?? true
  {
    let text = try exportedText(current, selection)
    if canHoldTag(text) {
      result = true
      break
    }
    // Markdown import matches a tag within one line.
    if text.utf16.contains(unit("\n")) {
      break
    }
    // A whitespace node already answered for shares the answer.
    if let passedAnswer = tag.answers[current.getKey()] {
      result = passedAnswer
      break
    }
    passed.append(current)
    next = try getTextSibling(current, false)
  }
  tag.answers[node.getKey()] = result
  for passedNode in passed {
    tag.answers[passedNode.getKey()] = result
  }
  return result
}

/// `$exportedText(node, selection)`: the text of `node` that the export
/// emits: its selected slice, if any.
private func exportedText(_ node: TextNode, _ selection: BaseSelection?) throws -> String {
  guard let selection else { return try node.getTextContent() }
  return try sliceSelectedTextNodeContent(selection, node, .clone).getTextContent()
}

/// `hasFormat(node, format)`.
private func hasFormat(_ node: LexicalNode?, _ format: TextFormatType) throws -> Bool {
  guard let text = node as? TextNode else { return false }
  return try text.hasFormat(format)
}

/// `canHoldTag(text)`: whether exported text can carry a format's tag:
/// CommonMark flanking rules keep tags off whitespace. (`/\S/.test(text)`.)
private func canHoldTag(_ text: String) -> Bool {
  text.utf16.contains { !isJSWhitespace($0) }
}

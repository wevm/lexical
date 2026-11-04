import Lexical

// The transformer types of packages/lexical-markdown/src/MarkdownTransformers.ts.
// They are classes so that a transformer has an identity (import keys its
// text matches by transformer, as JS keys a Map by object), and their
// properties are `var`s so that a variant can be made from a copy, as JS
// spreads one (`{...LINK, importRegExp}`).

/// `Transformer`: one of `ElementTransformer`, `MultilineElementTransformer`,
/// `TextFormatTransformer` and `TextMatchTransformer`.
public protocol Transformer: AnyObject {
  /// `'element'`, `'multiline-element'`, `'text-format'` or `'text-match'`.
  var type: String { get }
}

/// `(node: ElementNode) => string`: exports an element's children.
public typealias ExportChildren = (ElementNode) throws -> String

/// `ElementTransformer`: a block that a line's leading markdown makes,
/// such as a heading or a list item.
public final class ElementTransformer: Transformer {
  public var dependencies: [LexicalNode.Type]
  /// Called by `$convertToMarkdownString`; `nil` cancels the export and
  /// the next transformer is tried. `selection` filters the content when
  /// given.
  public var export: (LexicalNode, ExportChildren, BaseSelection?) throws -> String?
  public var regExp: JSRegExp
  /// Called when markdown is imported or typed; `false` cancels the
  /// transform and the next transformer is tried (`nil` is JS `void`).
  /// The last argument is whether the match is from an import.
  public var replace: (ElementNode, [LexicalNode], JSRegExpMatch, Bool) throws -> Bool?
  /// Whether `registerMarkdownShortcuts` may run this transformer on Enter
  /// at the end of the line, without the trailing space.
  public var triggerOnEnter: Bool
  public let type = "element"

  public init(
    dependencies: [LexicalNode.Type],
    export: @escaping (LexicalNode, ExportChildren, BaseSelection?) throws -> String?,
    regExp: JSRegExp,
    replace: @escaping (ElementNode, [LexicalNode], JSRegExpMatch, Bool) throws -> Bool?,
    triggerOnEnter: Bool = false
  ) {
    self.dependencies = dependencies
    self.export = export
    self.regExp = regExp
    self.replace = replace
    self.triggerOnEnter = triggerOnEnter
  }

  /// A copy, to change some properties of (`{...transformer}`).
  public func copy() -> ElementTransformer {
    ElementTransformer(
      dependencies: dependencies, export: export, regExp: regExp, replace: replace,
      triggerOnEnter: triggerOnEnter)
  }
}

/// The arguments of `handleImportAfterStartMatch`.
public struct MultilineImportArgs {
  public var lines: [String]
  public var rootNode: ElementNode
  public var startLineIndex: Int
  public var startMatch: JSRegExpMatch
  public var transformer: MultilineElementTransformer
}

/// `regExpEnd`: a regex, or `{optional: true, regExp}`.
public struct MultilineRegExpEnd {
  /// Whether the end match is optional: if so, the text from the start
  /// match to the end of the document matches when no end does.
  public var optional: Bool
  public var regExp: JSRegExp

  public init(optional: Bool = false, regExp: JSRegExp) {
    self.optional = optional
    self.regExp = regExp
  }
}

/// The result of `handleImportAfterStartMatch`.
public enum MultilineImportResult {
  /// `[imported, lastLineIndex]`: whether a multiline element was imported,
  /// and the index of the last line processed.
  case handled(Bool, Int)
  /// `null`: try the next multiline element transformer.
  case skip
  /// `undefined`: the default behavior.
  case useDefault
}

/// `MultilineElementTransformer`: a block that spans lines between a start
/// and an end match, such as a fenced code block.
public final class MultilineElementTransformer: Transformer {
  /// Handles the import once `regExpStart` matched. Without it, lines are
  /// matched up to `regExpEnd` (or the end of the document when it is
  /// optional).
  public var handleImportAfterStartMatch: ((MultilineImportArgs) throws -> MultilineImportResult)?
  public var dependencies: [LexicalNode.Type]
  public var export: ((LexicalNode, ExportChildren, BaseSelection?) throws -> String?)?
  public var regExpStart: JSRegExp
  public var regExpEnd: MultilineRegExpEnd?
  /// Called only on import, not when typed: `(rootNode, children,
  /// startMatch, endMatch, linesInBetween, isImport)`. `children` is given
  /// (and `linesInBetween` is not) for a markdown shortcut.
  public var replace:
    (ElementNode, [LexicalNode]?, JSRegExpMatch, JSRegExpMatch?, [String]?, Bool) throws -> Bool?
  public let type = "multiline-element"

  public init(
    handleImportAfterStartMatch: ((MultilineImportArgs) throws -> MultilineImportResult)? = nil,
    dependencies: [LexicalNode.Type],
    export: ((LexicalNode, ExportChildren, BaseSelection?) throws -> String?)? = nil,
    regExpStart: JSRegExp,
    regExpEnd: MultilineRegExpEnd? = nil,
    replace:
      @escaping (
        ElementNode, [LexicalNode]?, JSRegExpMatch, JSRegExpMatch?, [String]?, Bool
      ) throws -> Bool?
  ) {
    self.handleImportAfterStartMatch = handleImportAfterStartMatch
    self.dependencies = dependencies
    self.export = export
    self.regExpStart = regExpStart
    self.regExpEnd = regExpEnd
    self.replace = replace
  }

  public func copy() -> MultilineElementTransformer {
    MultilineElementTransformer(
      handleImportAfterStartMatch: handleImportAfterStartMatch, dependencies: dependencies,
      export: export, regExpStart: regExpStart, regExpEnd: regExpEnd, replace: replace)
  }
}

/// `TextFormatTransformer`: a text format between a pair of tags, such as
/// `**bold**`.
public final class TextFormatTransformer: Transformer {
  public var format: [TextFormatType]
  public var tag: String
  public var intraword: Bool?
  public let type = "text-format"

  public init(format: [TextFormatType], tag: String, intraword: Bool? = nil) {
    self.format = format
    self.tag = tag
    self.intraword = intraword
  }

  public func copy() -> TextFormatTransformer {
    TextFormatTransformer(format: format, tag: tag, intraword: intraword)
  }
}

/// `TextMatchTransformer`: inline text that a regex matches, such as a
/// link.
public final class TextMatchTransformer: Transformer {
  public var dependencies: [LexicalNode.Type]
  /// How a node exports, or `nil` when this transformer does not export
  /// it: `(node, exportChildren, exportFormat)`. Export may call it with
  /// placeholder callbacks only to learn whether it writes the node, so it
  /// should have no side effects.
  public var export:
    ((LexicalNode, ExportChildren, (TextNode, String) throws -> String) throws -> String?)?
  /// What text is matched on import.
  public var importRegExp: JSRegExp?
  /// What text is matched for a markdown shortcut while typing.
  public var regExp: JSRegExp
  /// Turns the matched text into a node on import. A returned TextNode
  /// gets text format matching applied.
  public var replace: ((TextNode, JSRegExpMatch) throws -> TextNode?)?
  /// The end index of the match on import, after `importRegExp` matched;
  /// `nil` (JS `false`) when it does not match after all.
  public var getEndIndex: ((TextNode, JSRegExpMatch) throws -> Int?)?
  /// The character whose typing triggers the transformer in the editor.
  public var trigger: String?
  public let type = "text-match"

  public init(
    dependencies: [LexicalNode.Type],
    export: (
      (LexicalNode, ExportChildren, (TextNode, String) throws -> String) throws -> String?
    )? = nil,
    importRegExp: JSRegExp? = nil,
    regExp: JSRegExp,
    replace: ((TextNode, JSRegExpMatch) throws -> TextNode?)? = nil,
    getEndIndex: ((TextNode, JSRegExpMatch) throws -> Int?)? = nil,
    trigger: String? = nil
  ) {
    self.dependencies = dependencies
    self.export = export
    self.importRegExp = importRegExp
    self.regExp = regExp
    self.replace = replace
    self.getEndIndex = getEndIndex
    self.trigger = trigger
  }

  public func copy() -> TextMatchTransformer {
    TextMatchTransformer(
      dependencies: dependencies, export: export, importRegExp: importRegExp, regExp: regExp,
      replace: replace, getEndIndex: getEndIndex, trigger: trigger)
  }
}

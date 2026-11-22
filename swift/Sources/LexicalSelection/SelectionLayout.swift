import Lexical

/// Layout queries a root element answers where JS code measures the
/// rendered DOM with `getBoundingClientRect`: the rect of a range
/// selection's caret and of a rendered block. The native text views adopt
/// it; rects are in the root element's coordinates, with y growing
/// downward. `@lexical/table`'s arrow keys read it when `TableConfig.layout`
/// is not set (DESIGN.md, The table view). A main-actor view conforms
/// with an isolated conformance, as it does to `LexicalRootElement`.
public protocol SelectionLayout: AnyObject {
  /// The rect of `selection`'s focus caret, or `nil` where it isn't shown.
  func selectionRect(_ selection: RangeSelection) -> SelectionRect?
  /// The rect of the rendered block `key`, or `nil` where it isn't shown.
  func nodeRect(_ key: NodeKey) -> SelectionRect?
}

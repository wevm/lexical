import Lexical
import LexicalExtension
import LexicalHTML

// Port of packages/lexical-table/src/LexicalTableExtension.ts.
//
// `registerStickyScrollbar` attaches scroll and resize listeners to each
// table's DOM; the sticky scrollbar belongs to the views (DESIGN.md, The
// table view), so only its config, the re-render when it toggles and
// `isStickyScrollbarActive` are ported.

/// `TableConfig`.
public struct TableConfig {
  /// When `false` (default `true`), merged cell support (colspan and
  /// rowspan) will be disabled and all tables will be forced into a regular
  /// grid with 1x1 table cells.
  public var hasCellMerge: Bool
  /// When `false` (default `true`), the background color of TableCellNode
  /// will always be removed.
  public var hasCellBackgroundColor: Bool
  /// When `true` (default `true`), the tab key can be used to navigate
  /// table cells.
  public var hasTabHandler: Bool
  /// When `true` (default `true`), tables will be wrapped in a `<div>` to
  /// enable horizontal scrolling.
  public var hasHorizontalScroll: Bool
  /// When `true` (default `false`), a sticky scrollbar is rendered below
  /// each table that overflows horizontally. Requires
  /// `hasHorizontalScroll`.
  public var hasStickyScrollbar: Bool
  /// When `true` (default `false`), nested tables will be allowed.
  /// `@experimental`: nested tables are not officially supported.
  public var hasNestedTables: Bool
  /// Not in JS: the rect queries of the arrow keys, which JS answers with
  /// `getBoundingClientRect`. The views set it (DESIGN.md, The table
  /// view); without one, up and down inside a cell are the view's.
  public var layout: (any TableLayout)?

  public init(
    hasCellMerge: Bool = true, hasCellBackgroundColor: Bool = true, hasTabHandler: Bool = true,
    hasHorizontalScroll: Bool = true, hasStickyScrollbar: Bool = false,
    hasNestedTables: Bool = false, layout: (any TableLayout)? = nil
  ) {
    self.hasCellMerge = hasCellMerge
    self.hasCellBackgroundColor = hasCellBackgroundColor
    self.hasTabHandler = hasTabHandler
    self.hasHorizontalScroll = hasHorizontalScroll
    self.hasStickyScrollbar = hasStickyScrollbar
    self.hasNestedTables = hasNestedTables
    self.layout = layout
  }
}

/// `TableExtension`: configures `TableNode`, `TableRowNode` and
/// `TableCellNode` and registers the table behaviors (see `TableConfig`).
public let TableExtension = defineExtension(
  name: "@lexical/table/Table", config: safeCast(TableConfig()),
  dependencies: [
    // DOMImportExtension support for the nodes registered here. Inert
    // unless the editor routes HTML through the pipeline (e.g. via
    // ClipboardDOMImportExtension or $generateNodesFromDOMViaExtension).
    CoreImportExtension,
    configExtension(DOMImportExtension) { $0.rules = TableImportRules.map(\.entry) },
  ],
  nodes: { [.klass(TableNode.self), .klass(TableRowNode.self), .klass(TableCellNode.self)] },
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let stores = state.getOutput()
    var prevStickyScrollbar = false
    return mergeRegister(
      try effect { () -> (() -> Void)? in
        let hasHorizontalScroll = stores.hasHorizontalScroll.value
        let hasStickyScrollbar = stores.hasStickyScrollbar.value && hasHorizontalScroll
        let hadHorizontalScroll = try isScrollableTablesActive(editor)
        if hadHorizontalScroll != hasHorizontalScroll {
          setScrollableTablesActive(editor, hasHorizontalScroll)
        }
        if hadHorizontalScroll != hasHorizontalScroll || prevStickyScrollbar != hasStickyScrollbar {
          // Re-render existing tables through the new scroll-wrapper config
          // without cloning every TableNode. A full reconcile marks no nodes
          // dirty, so it's deferred and produces no history entry.
          try editor.update { fullReconcile() }
        }
        prevStickyScrollbar = hasStickyScrollbar
        return nil
      },
      try registerTablePlugin(editor, hasNestedTables: stores.hasNestedTables),
      try effect { () -> (() -> Void)? in
        try registerTableSelectionObserver(
          editor, stores.hasTabHandler.value, layout: stores.layout.value)
      },
      try effect { () -> (() -> Void)? in
        stores.hasCellMerge.value ? nil : try registerTableCellUnmergeTransform(editor)
      },
      try effect { () -> (() -> Void)? in
        stores.hasCellBackgroundColor.value
          ? nil
          : try editor.registerNodeTransform(TableCellNode.self) { node in
            if try node.getBackgroundColor() != nil {
              try node.setBackgroundColor(nil)
            }
          }
      })
  })

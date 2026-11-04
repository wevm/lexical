import Lexical

// Port of packages/lexical-extension/src/NormalizeTripleClickSelectionExtension.ts.

/// `$fixFocusOverselection`: a triple click selects a whole block, and the
/// selection's focus lands at the start of the next block, so formatting
/// the selection would reach into that block. Moves the focus back to the
/// end of the clicked block's content. The config's default.
public func defaultFixFocusOverselection() throws {
  guard let selection = try getSelection() as? RangeSelection, !selection.isCollapsed() else {
    return
  }
  let range = try getCaretRangeInDirection(caretRangeFromSelection(selection), .next)
  var focusCaret = range.focus
  // Move it out of the next TextNode if none of it is selected
  if focusCaret.isText, range.anchor.origin !== focusCaret.origin, focusCaret.offset == 0 {
    focusCaret = try rewindSiblingCaret(focusCaret.getSiblingCaret())
  }
  // Move it behind a single LineBreakNode
  if focusCaret.kind == .sibling, range.anchor.origin !== focusCaret.origin,
    focusCaret.origin is LineBreakNode
  {
    focusCaret = try rewindSiblingCaret(focusCaret)
  }
  // Move the focus out of the start of any elements
  while focusCaret.kind == .child, range.anchor.origin !== focusCaret.origin {
    focusCaret = try rewindSiblingCaret(getSiblingCaret(focusCaret.origin, .next))
  }
  // Move it inside the containing element
  if focusCaret.kind == .sibling, let element = focusCaret.origin as? ElementNode {
    focusCaret = try normalizeCaret(getChildCaret(element, .previous)).getFlipped()
  }
  focusCaret = try normalizeCaret(focusCaret)
  if !focusCaret.isSamePointCaret(range.focus) {
    _ = try setSelectionFromCaretRange(getCaretRange(range.anchor, focusCaret))
  }
}

/// `NormalizeTripleClickSelectionConfig`. JS's deprecated `thresholdMsec`
/// and `dateNow`, which nothing reads, are not ported.
public struct NormalizeTripleClickSelectionConfig {
  /// `true` to disable this extension.
  public var disabled: Bool
  /// `$fixFocusOverselection`: the update function to call when a triple
  /// click is detected.
  public var fixFocusOverselection: () throws -> Void

  public init(
    disabled: Bool = false,
    fixFocusOverselection: @escaping () throws -> Void = defaultFixFocusOverselection
  ) {
    self.disabled = disabled
    self.fixFocusOverselection = fixFocusOverselection
  }
}

/// `NormalizeTripleClickSelectionExtension`: trims the block selection a
/// triple (or later) click makes so it ends inside the clicked block.
///
/// JS arms on a `mousedown` whose `detail` is 3 or more and trims the next
/// selection change that matches the DOM, because the browser reports its
/// selection asynchronously. A native view has synced its selection by the
/// time it reports the click (`NSTextView` tracks the mouse inside
/// `mouseDown(with:)`), and reports it as `CLICK_COMMAND` with the click
/// count as `detail`, so the extension trims the selection when that
/// command arrives.
public let NormalizeTripleClickSelectionExtension = defineExtension(
  name: "@lexical/NormalizeTripleClickSelection", config: NormalizeTripleClickSelectionConfig(),
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let output = state.getOutput()
    return try effect { () -> (() -> Void)? in
      if output.disabled.value {
        return nil
      }
      return editor.registerCommand(
        CLICK_COMMAND,
        { event, _ in
          if event.detail >= 3 {
            try output.fixFocusOverselection.peek()()
          }
          return false
        }, COMMAND_PRIORITY_BEFORE_CRITICAL)
    }
  })

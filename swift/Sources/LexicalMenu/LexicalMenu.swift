import Lexical

// Port of the platform-neutral parts of
// packages/lexical-react/src/shared/LexicalMenu.tsx and
// LexicalTypeaheadMenuPlugin.tsx: finding the query before the caret,
// opening and closing the menu, the highlighted option and the keyboard
// commands. The menu itself is drawn by the app (a SwiftUI popover or
// panel), which reads `MenuState` and positions it at `resolution`'s range
// through the view's rect for a model range, where JS positions an anchor
// `div` at `range.getBoundingClientRect()`.

/// `MenuResolution`: what an open menu is anchored at. JS carries a
/// `getRect` closure over a DOM `Range`; natively the anchor is the model
/// range from the trigger to the caret (`anchorKey`, UTF-16 offsets), which
/// the view measures. A node menu has no match and anchors at its node.
public struct MenuResolution: Equatable, Sendable {
  public var match: MenuTextMatch?
  /// The text node the range is in, or the node a node menu is for.
  public var anchorKey: NodeKey
  public var startOffset: Int
  public var endOffset: Int

  public init(match: MenuTextMatch?, anchorKey: NodeKey, startOffset: Int, endOffset: Int) {
    self.match = match
    self.anchorKey = anchorKey
    self.startOffset = startOffset
    self.endOffset = endOffset
  }
}

/// `getFullMatchOffset`: walks back along the typed text and forward
/// through the entry's text, to replace as much of what was typed as
/// matches the entry.
func getFullMatchOffset(_ documentText: String, _ entryText: String, _ offset: Int) -> Int {
  let document = Array(documentText.utf16)
  let entry = Array(entryText.utf16)
  var triggerOffset = offset
  var i = triggerOffset
  while i <= entry.count {
    // `documentText.slice(-i)`: for i == 0 that is the whole string.
    let suffix = i == 0 ? document[...] : document[max(0, document.count - i)...]
    if suffix.elementsEqual(entry[0..<i]) {
      triggerOffset = i
    }
    i += 1
  }
  return triggerOffset
}

/// `$splitNodeContainingQuery`: splits the text node at the caret so that
/// the query (with its trigger) is a node of its own, and returns it.
public func splitNodeContainingQuery(_ match: MenuTextMatch) throws -> TextNode? {
  guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed() else {
    return nil
  }
  let anchor = selection.anchor
  guard anchor.type == .text, let anchorNode = try anchor.getNode() as? TextNode,
    try anchorNode.isSimpleText()
  else { return nil }
  let selectionOffset = anchor.offset
  let textContent = try anchorNode.getTextContent().jsSlice(0, selectionOffset)
  let characterOffset = match.replaceableString.utf16.count
  let queryOffset = getFullMatchOffset(textContent, match.matchingString, characterOffset)
  let startOffset = selectionOffset - queryOffset
  if startOffset < 0 { return nil }
  if startOffset == 0 {
    return try anchorNode.splitText(selectionOffset).first
  }
  let parts = try anchorNode.splitText(startOffset, selectionOffset)
  return parts.count > 1 ? parts[1] : nil
}

/// `getTextUpToAnchor`.
func getTextUpToAnchor(_ selection: RangeSelection) throws -> String? {
  let anchor = selection.anchor
  guard anchor.type == .text, let anchorNode = try anchor.getNode() as? TextNode,
    try anchorNode.isSimpleText()
  else { return nil }
  return try anchorNode.getTextContent().jsSlice(0, anchor.offset)
}

/// `isSelectionOnEntityBoundary`.
func isSelectionOnEntityBoundary(_ offset: Int) throws -> Bool {
  if offset != 0 { return false }
  guard let selection = try getSelection() as? RangeSelection else { return false }
  let prev = try selection.anchor.getNode().getPreviousSibling() as? TextNode
  return prev?.isTextEntity() ?? false
}

/// The callback a menu runs when an option is chosen (`onSelectOption`):
/// the option, the text node holding the query (split off when the menu
/// splits it), a function that closes the menu, and the query.
public typealias MenuSelectOption<Option: MenuOption> = (
  _ option: Option, _ textNodeContainingQuery: TextNode?, _ closeMenu: @escaping () -> Void,
  _ matchingString: String
) throws -> Void

/// An open or closed menu and its options: the state `LexicalMenu` and
/// `LexicalTypeaheadMenuPlugin` keep in React, and their command handlers.
/// The view that draws the menu observes `onChange` and calls
/// `selectOptionAndCleanUp` and `setHighlightedIndex` for pointer input.
@MainActor
public final class MenuState<Option: MenuOption> {
  public let editor: Editor
  public private(set) var resolution: MenuResolution?
  public private(set) var options: [Option] = []
  private var rawSelectedIndex: Int?
  public let commandPriority: Int
  public let preselectFirstItem: Bool
  let shouldSplitNodeWithQuery: Bool
  public var onSelectOption: MenuSelectOption<Option>
  public var onOpen: ((MenuResolution) -> Void)?
  public var onClose: (() -> Void)?
  /// Called whenever the resolution, the options or the highlighted index
  /// change, for the view to draw the menu again.
  public var onChange: (() -> Void)?
  private var isOpen = false
  private var unregisterCommands: (() -> Void)?

  init(
    editor: Editor, commandPriority: Int, preselectFirstItem: Bool,
    shouldSplitNodeWithQuery: Bool, onSelectOption: @escaping MenuSelectOption<Option>
  ) {
    self.editor = editor
    self.commandPriority = commandPriority
    self.preselectFirstItem = preselectFirstItem
    self.shouldSplitNodeWithQuery = shouldSplitNodeWithQuery
    self.onSelectOption = onSelectOption
  }

  /// The highlighted option's index, clamped when the list shrinks.
  public var selectedIndex: Int? {
    rawSelectedIndex.map { min(options.count - 1, $0) }
  }

  /// Whether the app's view draws the menu even when it has no options, as
  /// a JS `menuRenderFn` is called whatever the option list looks like and
  /// may show a "no results" panel. While it is false, an empty menu draws
  /// nothing, as the default renderer, and its keys pass through.
  public var drawsEmptyMenu = false

  /// Whether the menu puts nothing on screen, so the keys pass through to
  /// the editor (`rendersNothing`).
  var rendersNothing: Bool { !drawsEmptyMenu && options.isEmpty }

  /// Replaces the options (JS re-renders with new `options`).
  public func setOptions(_ options: [Option]) {
    self.options = options
    if rawSelectedIndex == nil, preselectFirstItem, resolution != nil {
      rawSelectedIndex = 0
    }
    onChange?()
  }

  public func setHighlightedIndex(_ index: Int?) {
    rawSelectedIndex = index
    onChange?()
  }

  func open(_ resolution: MenuResolution) {
    let queryChanged = resolution.match?.matchingString != self.resolution?.match?.matchingString
    self.resolution = resolution
    if queryChanged, preselectFirstItem { rawSelectedIndex = 0 }
    if !isOpen {
      isOpen = true
      registerCommands()
      onOpen?(resolution)
    }
    onChange?()
  }

  /// `closeTypeahead` / `closeNodeMenu`.
  public func close() {
    guard isOpen else { return }
    isOpen = false
    unregisterCommands?()
    unregisterCommands = nil
    onClose?()
    resolution = nil
    rawSelectedIndex = nil
    onChange?()
  }

  /// `selectOptionAndCleanUp`.
  public func selectOptionAndCleanUp(_ option: Option) {
    let resolution = resolution
    // An error goes to the editor's `onError`, as one thrown in a JS
    // `editor.update` does.
    do {
      try editor.update { [self] in
        let textNodeContainingQuery =
          try resolution?.match != nil && shouldSplitNodeWithQuery
          ? splitNodeContainingQuery(resolution!.match!) : nil
        try onSelectOption(
          option, textNodeContainingQuery, { [weak self] in self?.close() },
          resolution?.match?.matchingString ?? "")
      }
    } catch {
      try? editor._onError(error)
    }
  }

  private func move(_ forward: Bool, _ event: KeyboardEvent) -> Bool {
    if rendersNothing { return false }
    let last = options.count - 1
    let newIndex: Int
    if let index = selectedIndex {
      newIndex = forward ? (index != last ? index + 1 : 0) : (index != 0 ? index - 1 : last)
    } else {
      newIndex = forward ? 0 : last
    }
    rawSelectedIndex = newIndex
    // A menu drawn with no options has nothing to highlight.
    guard options.indices.contains(newIndex) else {
      rawSelectedIndex = -1
      onChange?()
      event.preventDefault()
      return true
    }
    _ = try? editor.dispatchCommand(
      SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND, (index: newIndex, option: options[newIndex]))
    onChange?()
    event.preventDefault()
    return true
  }

  /// The keyboard commands of `LexicalMenu`, at `commandPriority`, while the
  /// menu is open.
  private func registerCommands() {
    let removers = [
      editor.registerCommand(
        KEY_ARROW_DOWN_COMMAND,
        { [weak self] event, _ in
          MainActor.assumeIsolated { self?.move(true, event) ?? false }
        }, commandPriority),
      editor.registerCommand(
        KEY_ARROW_UP_COMMAND,
        { [weak self] event, _ in
          MainActor.assumeIsolated { self?.move(false, event) ?? false }
        }, commandPriority),
      editor.registerCommand(
        KEY_ESCAPE_COMMAND,
        { [weak self] event, _ in
          MainActor.assumeIsolated {
            guard let self, !self.rendersNothing else { return false }
            event.preventDefault()
            self.close()
            return true
          }
        }, commandPriority),
      editor.registerCommand(
        KEY_TAB_COMMAND,
        { [weak self] event, _ in
          MainActor.assumeIsolated {
            guard let self, let index = self.selectedIndex, self.options.indices.contains(index)
            else { return false }
            event.preventDefault()
            self.selectOptionAndCleanUp(self.options[index])
            return true
          }
        }, commandPriority),
      editor.registerCommand(
        KEY_ENTER_COMMAND,
        { [weak self] event, _ in
          MainActor.assumeIsolated {
            guard let self, let index = self.selectedIndex, self.options.indices.contains(index),
              event?.shiftKey != true
            else { return false }
            event?.preventDefault()
            self.selectOptionAndCleanUp(self.options[index])
            return true
          }
        }, commandPriority),
    ]
    unregisterCommands = { removers.forEach { $0() } }
  }

  isolated deinit {
    unregisterCommands?()
  }
}

/// `LexicalTypeaheadMenuPlugin`: opens a menu while `triggerFn` matches the
/// text before a collapsed caret, reports the query to `onQueryChange`, and
/// replaces the query when an option is chosen. Returns the menu and a
/// function that unregisters it.
@MainActor
public func registerTypeaheadMenu<Option: MenuOption>(
  _ editor: Editor, triggerFn: @escaping TriggerFn,
  onQueryChange: @escaping (String?) -> Void,
  onSelectOption: @escaping MenuSelectOption<Option>,
  commandPriority: Int = COMMAND_PRIORITY_LOW, preselectFirstItem: Bool = true,
  ignoreEntityBoundary: Bool = false
) -> (menu: MenuState<Option>, unregister: () -> Void) {
  let menu = MenuState<Option>(
    editor: editor, commandPriority: commandPriority, preselectFirstItem: preselectFirstItem,
    shouldSplitNodeWithQuery: true, onSelectOption: onSelectOption)
  let updateListener = { [weak menu] in
    guard let menu else { return }
    let closeUnlessComposing = { if !editor.isComposing() { menu.close() } }
    if !editor.isEditable() {
      menu.close()
      return
    }
    let next: (MenuTextMatch?, MenuResolution?)? = try? editor.read(.latest) {
      guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed(),
        let text = try getTextUpToAnchor(selection)
      else { return nil }
      let match = triggerFn(text, editor)
      guard let match else { return (nil, nil) }
      if !ignoreEntityBoundary, try isSelectionOnEntityBoundary(match.leadOffset) {
        return (match, nil)
      }
      // `tryToPositionRange`: the range from the trigger to the caret.
      return (
        match,
        MenuResolution(
          match: match, anchorKey: selection.anchor.key, startOffset: match.leadOffset,
          endOffset: selection.anchor.offset)
      )
    }
    guard let next else {
      closeUnlessComposing()
      return
    }
    onQueryChange(next.0?.matchingString)
    if let resolution = next.1 {
      menu.open(resolution)
    } else {
      closeUnlessComposing()
    }
  }
  let removeUpdate = editor.registerUpdateListener { _ in
    MainActor.assumeIsolated { updateListener() }
  }
  let removeEditable = editor.registerEditableListener { [weak menu] isEditable in
    if !isEditable { MainActor.assumeIsolated { menu?.close() } }
  }
  return (
    menu,
    { [weak menu] in
      removeUpdate()
      removeEditable()
      MainActor.assumeIsolated { menu?.close() }
    }
  )
}

/// `LexicalNodeMenuPlugin`: a menu anchored at the node `nodeKey`, open
/// while it is set. Call `setNodeKey` as the React prop changes.
@MainActor
public final class NodeMenu<Option: MenuOption> {
  public let menu: MenuState<Option>
  public private(set) var nodeKey: NodeKey?

  public init(
    _ editor: Editor, onSelectOption: @escaping MenuSelectOption<Option>,
    commandPriority: Int = COMMAND_PRIORITY_LOW
  ) {
    menu = MenuState(
      editor: editor, commandPriority: commandPriority, preselectFirstItem: true,
      shouldSplitNodeWithQuery: false, onSelectOption: onSelectOption)
  }

  /// `positionOrCloseMenu`.
  public func setNodeKey(_ key: NodeKey?) {
    nodeKey = key
    guard let key else {
      menu.close()
      return
    }
    let exists = (try? menu.editor.read(.latest) { try getNodeByKey(key) != nil }) ?? false
    if exists, menu.resolution == nil {
      menu.open(MenuResolution(match: nil, anchorKey: key, startOffset: 0, endOffset: 0))
    }
  }
}

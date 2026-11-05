import Lexical
import LexicalMenu
import LexicalRichText
import LexicalTesting
import Testing

// The typeahead menu's contract: trigger matching (expected values from
// upstream's `useBasicTypeaheadTriggerMatch` regex, run in Node), opening at
// the query, keyboard navigation at the menu's priority, and replacing the
// query with the chosen option.

private struct TriggerCase: CustomTestStringConvertible, Sendable {
  let trigger: String
  let text: String
  let expected: MenuTextMatch?
  var testDescription: String { "\(trigger) in \(text.debugDescription)" }
}

nonisolated(unsafe) private let componentPicker = basicTypeaheadTriggerMatch(
  "/", minLength: 0, allowWhitespace: true)
nonisolated(unsafe) private let mentions = basicTypeaheadTriggerMatch("@")
nonisolated(unsafe) private let emoji = basicTypeaheadTriggerMatch(
  ":", minLength: 1, punctuation: #"\.,\+\*\?\$\@\|#{}\(\)\^\[\]\\/!%'"~=<>:;"#)

private func m(_ lead: Int, _ query: String, _ replaceable: String) -> MenuTextMatch {
  MenuTextMatch(leadOffset: lead, matchingString: query, replaceableString: replaceable)
}

private let cases: [TriggerCase] = [
  .init(trigger: "/", text: "/", expected: m(0, "", "/")),
  .init(trigger: "/", text: "hi /", expected: m(3, "", "/")),
  .init(trigger: "/", text: "a/b", expected: nil),
  .init(trigger: "/", text: "/hea", expected: m(0, "hea", "/hea")),
  .init(trigger: "/", text: "/head ing", expected: m(0, "head ing", "/head ing")),
  .init(trigger: "/", text: "(/x", expected: m(1, "x", "/x")),
  .init(trigger: "/", text: "/x.y", expected: nil),
  .init(trigger: "@", text: "@", expected: nil),
  .init(trigger: "@", text: "@ab", expected: m(0, "ab", "@ab")),
  .init(trigger: "@", text: "x @ab", expected: m(2, "ab", "@ab")),
  .init(trigger: "@", text: "x@ab", expected: nil),
  .init(trigger: "@", text: "@a b", expected: nil),
  .init(trigger: "@", text: "@é👍", expected: m(0, "é👍", "@é👍")),
  .init(trigger: ":", text: ":", expected: nil),
  .init(trigger: ":", text: ":smi", expected: m(0, "smi", ":smi")),
  .init(trigger: ":", text: ":thumbs_up", expected: m(0, "thumbs_up", ":thumbs_up")),
  .init(trigger: ":", text: ":+1", expected: nil),
  .init(trigger: ":", text: "a :-)", expected: nil),
]

@Test(arguments: cases)
private func triggerMatch(_ c: TriggerCase) throws {
  let editor = try createTestEditor()
  let fn = c.trigger == "/" ? componentPicker : c.trigger == "@" ? mentions : emoji
  #expect(fn(c.text, editor) == c.expected)
}

@Test func leadOffsetCountsUTF16() throws {
  let editor = try createTestEditor()
  // "👍" is two UTF-16 code units, as in JS.
  #expect(mentions("👍 @jo", editor) == m(3, "jo", "@jo"))
}

private final class Item: MenuOption {
  override init(_ key: String) {
    super.init(key)
    title = key
  }
}

nonisolated(unsafe) private let slash = basicTypeaheadTriggerMatch("/", minLength: 0)

/// Moves a text point one UTF-16 unit, within its text node.
private final class CharacterMovement: SelectionMovement {
  nonisolated(unsafe) static let shared = CharacterMovement()
  func landedPoint(
    from point: Point, isBackward: Bool, granularity: SelectionGranularity
  ) throws -> (NodeKey, Int, PointType)? {
    guard point.type == .text, let text = try point.getNode() as? TextNode else { return nil }
    let offset = point.offset + (isBackward ? -1 : 1)
    guard offset >= 0, try offset <= text.getTextContentSize() else { return nil }
    return (text.getKey(), offset, .text)
  }
}

@MainActor
private struct Fixture {
  let editor: Editor
  let menu: MenuState<Item>
  let unregister: () -> Void
  let box: Box
  var chosen: [String] { box.values }

  final class Box {
    var values: [String] = []
    var queries: [String?] = []
    var opened = 0
    var closed = 0
    /// Whether the menu still had its resolution when `onClose` ran.
    var openWhenClosing: [Bool] = []
  }

  init(
    options: [Item] = [Item("a"), Item("b"), Item("c")], richText: Bool = true,
    triggerFn: @escaping TriggerFn = componentPicker
  ) throws {
    let editor = try createMountedTestEditor()
    // Stands in for the DOM `Selection.modify` a backspace goes through.
    editor.selectionMovement = CharacterMovement.shared
    if richText { _ = registerRichText(editor) }
    let box = Box()
    let (menu, unregister) = registerTypeaheadMenu(
      editor, triggerFn: triggerFn, onQueryChange: { box.queries.append($0) },
      onSelectOption: { (option: Item, node, close, _) in
        box.values.append(option.key)
        try node?.remove()
        close()
      })
    menu.onOpen = { _ in box.opened += 1 }
    menu.onClose = { [weak menu] in
      box.closed += 1
      box.openWhenClosing.append(menu?.resolution != nil)
    }
    menu.setOptions(options)
    self.editor = editor
    self.menu = menu
    self.unregister = unregister
    self.box = box
  }

  func type(_ text: String) throws {
    try update(editor) {
      let paragraph = try createParagraphNode()
      let node = try createTextNode(text)
      try paragraph.append(node)
      try getRoot().clear().append(paragraph)
      _ = try node.select()
    }
  }

  /// Inserts `text` at the selection, as typing it does.
  func insertText(_ text: String) throws {
    try update(editor) { try #require(try getSelection() as? RangeSelection).insertText(text) }
  }

  func text() throws -> String { try editor.read { try getRoot().getTextContent() } }

  /// Dispatches `command` with a fallback handler below the menu's
  /// priority, as LexicalMenuEmptyOptions.test.tsx's `pressKey` does.
  func press(
    _ command: LexicalCommand<KeyboardEvent>
  ) throws -> (
    handled: Bool, defaultPrevented: Bool, reachedEditor: Bool
  ) {
    var reachedEditor = false
    let remove = editor.registerCommand(
      command,
      { _, _ in
        reachedEditor = true
        return false
      }, COMMAND_PRIORITY_EDITOR)
    defer { remove() }
    let event = KeyboardEvent(key: "")
    let handled = try editor.dispatchCommand(command, event)
    return (handled, event.defaultPrevented, reachedEditor)
  }
}

@MainActor @Test func opensAtTheQueryAndClosesWithoutIt() throws {
  let f = try Fixture()
  defer { f.unregister() }
  try f.type("hello /he")
  let resolution = try #require(f.menu.resolution)
  #expect(resolution.match == m(6, "he", "/he"))
  #expect((resolution.startOffset, resolution.endOffset) == (6, 9))
  #expect(f.menu.selectedIndex == 0)
  try f.type("hello")
  #expect(f.menu.resolution == nil)
}

@MainActor @Test func arrowsWrapAndEnterChooses() throws {
  let f = try Fixture()
  defer { f.unregister() }
  try f.type("x /q")
  let up = KeyboardEvent(key: "ArrowUp")
  #expect(try f.editor.dispatchCommand(KEY_ARROW_UP_COMMAND, up))
  #expect(up.defaultPrevented)
  #expect(f.menu.selectedIndex == 2)
  #expect(try f.editor.dispatchCommand(KEY_ARROW_DOWN_COMMAND, KeyboardEvent(key: "ArrowDown")))
  #expect(f.menu.selectedIndex == 0)
  _ = try f.editor.dispatchCommand(KEY_ARROW_DOWN_COMMAND, KeyboardEvent(key: "ArrowDown"))
  #expect(try f.editor.dispatchCommand(KEY_ENTER_COMMAND, KeyboardEvent(key: "Enter")))
  drainMicrotasks()
  #expect(f.chosen == ["b"])
  // The query, trigger included, was split off and removed.
  #expect(try f.text() == "x ")
  #expect(f.menu.resolution == nil)
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > default rendering (without menuRenderFn) > should not select an option when Enter is pressed with Shift (line break / fall-through)"
  ))
func shiftEnterIsLeftToRichText() throws {
  let f = try Fixture()
  defer { f.unregister() }
  try f.type("/q")
  _ = try f.editor.dispatchCommand(KEY_ENTER_COMMAND, KeyboardEvent(key: "Enter", shiftKey: true))
  drainMicrotasks()
  #expect(f.chosen.isEmpty)
}

@MainActor @Test func escapeClosesAndKeysPassThroughWhenClosedOrEmpty() throws {
  let f = try Fixture()
  defer { f.unregister() }
  try f.type("/")
  #expect(try f.editor.dispatchCommand(KEY_ESCAPE_COMMAND, KeyboardEvent(key: "Escape")))
  #expect(f.menu.resolution == nil)
  #expect(try !f.editor.dispatchCommand(KEY_TAB_COMMAND, KeyboardEvent(key: "Tab")))
  try f.type("/z")
  // With no options nothing is drawn, so the keys are not the menu's:
  // Escape leaves it open (rich text takes the key).
  f.menu.setOptions([])
  _ = try f.editor.dispatchCommand(KEY_ESCAPE_COMMAND, KeyboardEvent(key: "Escape"))
  #expect(f.menu.resolution != nil)
}

// MARK: - Ports of lexical-react's menu tests
//
// The React tests render `LexicalMenu` and read the DOM it draws, or what a
// `menuRenderFn` is given (`options`, `selectedIndex`, `matchingString`).
// Natively the app's view draws the menu from `MenuState`, so these assert
// the state that view reads, and the commands the menu handles.

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPluginUtils.test.ts::SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND > is one command, whichever module it is reached through"
  ))
func scrollCommandIsTheOneTheMenuDispatches() throws {
  // Commands match by identity: the command an app registers for is the one
  // the menu dispatches as the highlight moves.
  let f = try Fixture()
  defer { f.unregister() }
  var scrolled: [(Int, String)] = []
  let remove = f.editor.registerCommand(
    SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND,
    { payload, _ in
      scrolled.append((payload.index, payload.option.key))
      return true
    }, COMMAND_PRIORITY_CRITICAL)
  defer { remove() }
  try f.type("/q")
  _ = try f.editor.dispatchCommand(KEY_ARROW_DOWN_COMMAND, KeyboardEvent(key: "ArrowDown"))
  #expect(scrolled.map(\.0) == [1])
  #expect(scrolled.map(\.1) == ["b"])
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPluginUtils.test.ts::MenuOption > is one class, whichever module it is reached through"
  ))
func typeaheadAndNodeMenusShareMenuOption() throws {
  // The typeahead menu and the node menu take the same `MenuOption`, so an
  // option chosen in either is the app's subclass.
  let f = try Fixture()
  defer { f.unregister() }
  var chosen: [MenuOption] = []
  let nodeMenu = NodeMenu<Item>(f.editor) { option, _, close, _ in
    chosen.append(option)
    close()
  }
  let typeaheadOption: MenuOption = f.menu.options[0]
  #expect(typeaheadOption is Item)
  try f.type("x")
  let key = try f.editor.read { try getRoot().getFirstChild()!.getKey() }
  nodeMenu.setNodeKey(key)
  nodeMenu.menu.setOptions([Item("n")])
  _ = try f.editor.dispatchCommand(KEY_ENTER_COMMAND, KeyboardEvent(key: "Enter"))
  #expect(chosen.count == 1)
  #expect(chosen.first is Item)
}

@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::MenuOption > should set key from constructor"
  ),
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > exports > should export MenuOption class"
  ),
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalNodeMenuPlugin.test.tsx::LexicalNodeMenuPlugin > exports > should export MenuOption class"
  ))
func menuOptionKeepsItsKey() {
  #expect(MenuOption("test-key").key == "test-key")
}

@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::MenuOption > should support optional title property"
  ))
func menuOptionHasAnOptionalTitle() {
  let option = MenuOption("test-key")
  #expect(option.title == nil)
  option.title = "Test Title"
  #expect(option.title == "Test Title")
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > default rendering (without menuRenderFn) > should apply selected class to preselected first item"
  ),
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > custom rendering (with menuRenderFn) > should pass selectedIndex to menuRenderFn"
  ))
func preselectsTheFirstOption() throws {
  let f = try Fixture(options: [Item("First"), Item("Second")])
  defer { f.unregister() }
  try f.type("/test")
  #expect(f.menu.selectedIndex == 0)
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > default rendering (without menuRenderFn) > should select an option when Enter is pressed without Shift"
  ))
func enterSelectsTheHighlightedOption() throws {
  let f = try Fixture(options: [Item("Option A"), Item("Option B")])
  defer { f.unregister() }
  try f.type("/test")
  #expect(try f.editor.dispatchCommand(KEY_ENTER_COMMAND, KeyboardEvent(key: "Enter")))
  #expect(f.chosen == ["Option A"])
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > custom rendering (with menuRenderFn) > should use menuRenderFn when provided"
  ),
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > custom rendering (with menuRenderFn) > should pass options array to menuRenderFn"
  ))
func exposesTheOptionsAndTheQueryToTheView() throws {
  let f = try Fixture(options: [Item("X"), Item("Y"), Item("Z")])
  defer { f.unregister() }
  try f.type("/hello")
  #expect(f.menu.options.map(\.title) == ["X", "Y", "Z"])
  #expect(f.menu.resolution?.match?.matchingString == "hello")
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > custom rendering (with menuRenderFn) > should pass empty string as matchingString when no match"
  ))
func aMenuWithoutAMatchHasAnEmptyQuery() throws {
  let editor = try createMountedTestEditor()
  var queries: [String] = []
  let nodeMenu = NodeMenu<Item>(editor) { _, _, close, matchingString in
    queries.append(matchingString)
    close()
  }
  try update(editor) { try getRoot().append(createParagraphNode()) }
  nodeMenu.setNodeKey(try editor.read { try getRoot().getFirstChild()!.getKey() })
  nodeMenu.menu.setOptions([Item("A")])
  #expect(nodeMenu.menu.resolution?.match == nil)
  _ = try editor.dispatchCommand(KEY_ENTER_COMMAND, KeyboardEvent(key: "Enter"))
  #expect(queries == [""])
}

private let menuKeys: [(String, LexicalCommand<KeyboardEvent>)] = [
  ("ArrowDown", KEY_ARROW_DOWN_COMMAND), ("ArrowUp", KEY_ARROW_UP_COMMAND),
  ("Escape", KEY_ESCAPE_COMMAND),
]

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > lets ArrowDown through when there are no options"
  ),
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > lets ArrowUp through when there are no options"
  ),
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > lets Escape through when there are no options"
  ))
func keysPassThroughAnEmptyMenu() throws {
  for (name, command) in menuKeys {
    // An empty option list draws no menu, so the key still reaches what
    // would otherwise move the caret.
    let f = try Fixture(options: [], richText: false)
    defer { f.unregister() }
    try f.type("/zz")
    let result = try f.press(command)
    #expect(!result.handled, "\(name)")
    #expect(!result.defaultPrevented, "\(name)")
    #expect(result.reachedEditor, "\(name)")
  }
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > still consumes ArrowDown when there are options"
  ),
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > still consumes ArrowUp when there are options"
  ),
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > still consumes Escape when there are options"
  ))
func aMenuWithOptionsConsumesItsKeys() throws {
  for (name, command) in menuKeys {
    let f = try Fixture(options: [Item("a"), Item("b")], richText: false)
    defer { f.unregister() }
    try f.type("/zz")
    let result = try f.press(command)
    #expect(result.handled, "\(name)")
    #expect(result.defaultPrevented, "\(name)")
    #expect(!result.reachedEditor, "\(name)")
  }
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > still consumes ArrowDown when a custom renderer draws an empty menu"
  ),
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > still consumes ArrowUp when a custom renderer draws an empty menu"
  ),
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > still consumes Escape when a custom renderer draws an empty menu"
  ))
func aViewThatDrawsAnEmptyMenuKeepsItsKeys() throws {
  for (name, command) in menuKeys {
    // A view that shows a "no results" panel (a `menuRenderFn`) still has
    // to be moved through and dismissed.
    let f = try Fixture(options: [], richText: false)
    defer { f.unregister() }
    f.menu.drawsEmptyMenu = true
    try f.type("/zz")
    let result = try f.press(command)
    #expect(result.handled, "\(name)")
    #expect(result.defaultPrevented, "\(name)")
    #expect(!result.reachedEditor, "\(name)")
  }
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > clears an uncommitted query when onClose is synchronous"
  ))
func closesWhenTheQueryEnds() throws {
  let f = try Fixture(triggerFn: slash)
  defer { f.unregister() }
  try f.type("/old")
  try f.insertText("er")
  try f.insertText(" ")
  #expect(f.box.closed == 1)
  #expect(f.menu.resolution == nil)
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > closes a first query that has not rendered yet"
  ))
func closesAFirstQuery() throws {
  let f = try Fixture(triggerFn: slash)
  defer { f.unregister() }
  try update(f.editor) { _ = try getRoot().clear().append(createParagraphNode()).select() }
  try f.insertText("/a")
  try f.insertText(" ")
  #expect(f.box.opened == 1)
  #expect(f.box.closed == 1)
  #expect(f.menu.resolution == nil)
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > runs synchronous onClose before clearing the menu"
  ))
func runsOnCloseBeforeClearingTheMenu() throws {
  let f = try Fixture(triggerFn: slash)
  defer { f.unregister() }
  try update(f.editor) { _ = try getRoot().clear().append(createParagraphNode()).select() }
  try f.insertText("/")
  #expect(f.menu.resolution != nil)
  try update(f.editor) { _ = try f.editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true) }
  #expect(f.box.closed == 1)
  #expect(f.box.openWhenClosing == [true])
  #expect(f.menu.resolution == nil)
}

@MainActor
private func composingFixture() throws -> Fixture {
  let f = try Fixture(triggerFn: slash)
  try update(f.editor) { _ = try getRoot().clear().append(createParagraphNode()).select() }
  try f.insertText("/")
  try update(f.editor) {
    let selection = try #require(try getSelection() as? RangeSelection)
    try selection.insertText("햄")
    try setCompositionKey(selection.anchor.key)
  }
  return f
}

/// `$dropTriggerWhileComposing`: the composition replaces the query,
/// trigger and all.
private func dropTriggerWhileComposing() throws {
  let text = try #require(try getRoot().getFirstDescendant() as? TextNode)
  try text.setTextContent("햄")
  try setCompositionKey(text.getKey())
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > IME composition > reports the query while composing without closing the menu"
  ))
func reportsTheQueryWhileComposing() throws {
  let f = try composingFixture()
  defer { f.unregister() }
  #expect(f.editor.isComposing())
  #expect(f.box.queries.last == "햄")
  #expect(f.menu.resolution?.match?.matchingString == "햄")
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > IME composition > keeps the menu open when the trigger match is lost while composing"
  ))
func staysOpenWhileComposingWithoutAMatch() throws {
  let f = try composingFixture()
  defer { f.unregister() }
  try update(f.editor) { try dropTriggerWhileComposing() }
  #expect(f.editor.isComposing())
  #expect(f.box.closed == 0)
  #expect(f.menu.resolution != nil)
}

@MainActor
@Test(
  .upstream(
    "lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > IME composition > closes the menu once composition ends without a trigger match"
  ))
func closesOnceCompositionEndsWithoutAMatch() throws {
  let f = try composingFixture()
  defer { f.unregister() }
  try update(f.editor) { try dropTriggerWhileComposing() }
  #expect(f.menu.resolution != nil)
  try update(f.editor) {
    try setCompositionKey(nil)
    try getRoot().getFirstDescendant()?.markDirty()
  }
  #expect(!f.editor.isComposing())
  #expect(f.box.closed == 1)
  #expect(f.menu.resolution == nil)
}

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

private final class Item: MenuOption {}

@MainActor
private struct Fixture {
  let editor: Editor
  let menu: MenuState<Item>
  let unregister: () -> Void
  var chosen: [String] { chosenBox.values }
  let chosenBox: Box

  final class Box { var values: [String] = [] }

  init() throws {
    let editor = try createMountedTestEditor()
    _ = registerRichText(editor)
    let box = Box()
    let (menu, unregister) = registerTypeaheadMenu(
      editor, triggerFn: componentPicker, onQueryChange: { _ in },
      onSelectOption: { (option: Item, node, close, _) in
        box.values.append(option.key)
        try node?.remove()
        close()
      })
    menu.setOptions([Item("a"), Item("b"), Item("c")])
    self.editor = editor
    self.menu = menu
    self.unregister = unregister
    self.chosenBox = box
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

  func text() throws -> String { try editor.read { try getRoot().getTextContent() } }
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

@MainActor @Test func shiftEnterIsLeftToRichText() throws {
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

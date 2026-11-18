import Lexical
import LexicalExtension
import LexicalLink
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts.
//
// Upstream spies on `window.open` and dispatches `click` and `auxclick` on
// the mounted `<a>`. The native views report a click as `CLICK_COMMAND`
// with the node under the pointer, so the port dispatches that, with the
// link's text as the target, and records what the configured `LinkWindow`
// opens.

private let URL = "https://lexical.dev/"

private final class RecordingWindow: LinkWindow {
  var calls: [[String]] = []
  func open(_ url: String, _ target: String, _ features: String?) {
    calls.append([url, target] + (features.map { [$0] } ?? []))
  }
}

private func extension_(
  _ window: RecordingWindow, newTab: Bool = false
)
  -> any AnyLexicalExtensionArgument
{
  defineExtension(
    name: "[root]",
    dependencies: [
      configExtension(ClickableLinkExtension) {
        $0.getWindow = { _ in window }
        if newTab { $0.newTab = true }
      },
      RichTextExtension,
    ],
    editorConfig: InitialEditorConfig(
      initialEditorState: .update { _ in
        let link = try createLinkNode(URL).append(createTextNode("Hello"))
        try getRoot().append(createParagraphNode().append(link))
      }))
}

/// Builds the editor, and runs `fn` with a function that dispatches a click
/// of `type` and `button` on the link's text and returns the event.
private func withEditor(
  newTab: Bool = false, _ fn: (RecordingWindow, (String, Int) throws -> MouseEvent) throws -> Void
) throws {
  let window = RecordingWindow()
  let editor = try buildEditorFromExtensions(extension_(window, newTab: newTab))
  defer { editor.dispose() }
  try editor.setRootElement(TestRootElement())
  let textKey = try editor.read { try getRoot().getLastDescendant()!.getKey() }
  try fn(window) { type, button in
    let event = MouseEvent(type, button: button, targetKey: textKey)
    _ = try editor.dispatchCommand(CLICK_COMMAND, event)
    return event
  }
}

@Suite struct ClickableLinkExtensionTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a left click opens the link in the same tab"
    ))
  func leftClick() throws {
    try withEditor { window, dispatch in
      _ = try dispatch("click", 0)
      #expect(window.calls == [[URL, "_self"]])
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a middle click opens the link in a new tab"
    ))
  func middleClick() throws {
    try withEditor { window, dispatch in
      _ = try dispatch("auxclick", 1)
      #expect(window.calls == [[URL, "_blank", "noopener"]])
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > with newTab, a left click opens the link in a new tab without an opener"
    ))
  func newTabLeftClick() throws {
    try withEditor(newTab: true) { window, dispatch in
      _ = try dispatch("click", 0)
      #expect(window.calls == [[URL, "_blank", "noopener"]])
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a middle click cancels the browser opening the link itself"
    ))
  func middleClickPreventsDefault() throws {
    try withEditor { _, dispatch in
      let event = try dispatch("auxclick", 1)
      #expect(event.defaultPrevented)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a right click does not open the link"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a back click does not open the link"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a forward click does not open the link"
    ),
    arguments: [2, 3, 4])
  func otherButtons(_ button: Int) throws {
    try withEditor { window, dispatch in
      let event = try dispatch("auxclick", button)
      // `auxclick` fires for every non-primary button; only the middle one
      // opens the link.
      #expect(window.calls.isEmpty)
      #expect(!event.defaultPrevented)
    }
  }
}

/// A root element that records what it is asked to open, as a native view
/// opens it with the system.
private final class OpeningRoot: LexicalRootElement {
  var isConnected: Bool { true }
  var calls: [[String]] = []
  func open(_ url: String, _ target: String, _ features: String?) {
    calls.append([url, target] + (features.map { [$0] } ?? []))
  }
}

@Suite struct ClickableLinkDefaultWindowTests {
  /// JS opens links in the global window by default; Swift in the root
  /// element's, so a view opens them without the app configuring one.
  @Test func defaultOpensWithTheRootElement() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[root]", dependencies: [ClickableLinkExtension, RichTextExtension],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            let link = try createLinkNode(URL).append(createTextNode("Hello"))
            try getRoot().append(createParagraphNode().append(link))
          })))
    defer { editor.dispose() }
    let textKey = try editor.read { try getRoot().getLastDescendant()!.getKey() }
    let click = { (button: Int) in
      _ = try editor.dispatchCommand(
        CLICK_COMMAND,
        MouseEvent(button == 0 ? "click" : "auxclick", button: button, targetKey: textKey))
    }
    // A root that is not a LexicalRootElement has no window: nothing opens.
    try editor.setRootElement(TestRootElement())
    try click(0)
    let root = OpeningRoot()
    try editor.setRootElement(root)
    try click(0)
    try click(1)
    #expect(root.calls == [[URL, "_self"], [URL, "_blank", "noopener"]])
  }
}

import XCTest

/// Smoke tests of the playground on macOS, for what differs from iOS: the
/// hardware keyboard's shortcuts and the panels over the text view.
final class LexicalPlaygroundMacUITests: XCTestCase {
  private var app: XCUIApplication!

  override func setUp() {
    continueAfterFailure = false
  }

  /// The toolbar's Insert Link button links the selection and puts the
  /// caret in the floating link editor's URL field, so the URL can be typed
  /// without a click.
  @MainActor
  func testInsertLinkFocusesTheLinkURLField() {
    assertLinksHello { _ in app.buttons["Insert Link"].firstMatch.click() }
  }

  /// ⌘K, the shortcut for Insert Link, focuses the URL field the same way.
  @MainActor
  func testCommandKFocusesTheLinkURLField() {
    assertLinksHello { editor in editor.typeKey("k", modifierFlags: .command) }
  }

  /// Types "Hello", selects it, runs `insertLink`, then types a URL with no
  /// click and checks that it became the link's.
  @MainActor
  private func assertLinksHello(_ insertLink: (XCUIElement) -> Void) {
    app = XCUIApplication()
    app.launch()
    // The identifier is on the editor's scroll view.
    let editor = app.descendants(matching: .any)["editor"].textViews.firstMatch
    XCTAssertTrue(editor.waitForExistence(timeout: 10), app.debugDescription)
    app.buttons["Clear"].click()
    editor.click()
    editor.typeText("Hello")
    editor.typeKey(.leftArrow, modifierFlags: [.shift, .option])
    insertLink(editor)
    // The panel draws in the text view, which does not expose it to
    // accessibility; what the typing did shows it. No click: the URL
    // field has the focus, with its `https://` selected.
    sleep(1)
    app.typeText("https://lexical.dev\r")
    XCTAssertEqual(editor.value as? String, "Hello")
    app.descendants(matching: .any)["Export DOM"].firstMatch.click()
    let linked = NSPredicate(
      format: "label CONTAINS %@ OR value CONTAINS %@", #"href="https://lexical.dev""#,
      #"href="https://lexical.dev""#)
    let tree = app.descendants(matching: .any)["tree-view"].firstMatch
    expectation(for: linked, evaluatedWith: tree)
    waitForExpectations(timeout: 5)
  }
}

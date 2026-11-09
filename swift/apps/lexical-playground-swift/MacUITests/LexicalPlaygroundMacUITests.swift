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

  /// The floating format toolbar and the "/" menu, panels over the text
  /// view, move with their selection and query as the text scrolls.
  @MainActor
  func testFormatToolbarAndMenuFollowTheScrolledText() {
    app = XCUIApplication()
    app.launch()
    let editor = app.descendants(matching: .any)["editor"].textViews.firstMatch
    XCTAssertTrue(editor.waitForExistence(timeout: 10), app.debugDescription)
    app.buttons["Clear"].click()
    editor.click()
    editor.typeText((1...40).map { "Line \($0)" }.joined(separator: "\r"))
    // "Line" in line 6, then a "/" query in a line after it.
    editor.typeKey(.upArrow, modifierFlags: .command)
    for _ in 1..<6 { editor.typeKey(.downArrow, modifierFlags: []) }
    editor.typeKey(.rightArrow, modifierFlags: [.shift, .option])
    let toolbar = app.descendants(matching: .any)["floating-text-format-toolbar"].firstMatch
    XCTAssertTrue(toolbar.waitForExistence(timeout: 5), app.debugDescription)
    assertFollowsTheScrolledText(toolbar, editor)
    editor.typeKey(.rightArrow, modifierFlags: .command)
    editor.typeText("\r/")
    let menu = app.descendants(matching: .any)["typeahead-menu"].firstMatch
    XCTAssertTrue(menu.waitForExistence(timeout: 5), app.debugDescription)
    assertFollowsTheScrolledText(menu, editor)
  }

  /// Scrolls the text up a little and checks that `panel` moved as far.
  @MainActor
  private func assertFollowsTheScrolledText(_ panel: XCUIElement, _ editor: XCUIElement) {
    let panelTop = panel.frame.minY
    let textTop = editor.frame.minY
    // The wheel goes to the top of the editor's visible part (its scroll
    // view), above the panels, which would take it.
    app.descendants(matching: .any)["editor"].firstMatch
      .coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.05))
      .scroll(byDeltaX: 0, deltaY: -30)
    sleep(1)
    let scrolled = editor.frame.minY - textTop
    XCTAssertLessThan(scrolled, -10, "The text did not scroll: \(editor.frame).")
    XCTAssertEqual(
      panel.frame.minY - panelTop, scrolled, accuracy: 1,
      "\(panel.identifier) did not move with the text.")
  }

  /// A typeahead menu stays inside the window: in a 640 point window the
  /// editor is narrower than the "/" menu, which is pulled in at the
  /// editor's right edge and so went past the window's left edge.
  @MainActor
  func testTypeaheadMenuStaysInsideANarrowWindow() {
    app = XCUIApplication()
    app.launch()
    let window = app.windows.firstMatch
    let editor = app.descendants(matching: .any)["editor"].textViews.firstMatch
    XCTAssertTrue(editor.waitForExistence(timeout: 10), app.debugDescription)
    // Drags the window's bottom right corner to make it `width` wide.
    func resize(to width: CGFloat) {
      let corner = window.coordinate(withNormalizedOffset: CGVector(dx: 1, dy: 1))
        .withOffset(CGVector(dx: -2, dy: -2))
      corner.click(
        forDuration: 0.3,
        thenDragTo: corner.withOffset(CGVector(dx: width - window.frame.width, dy: 0)))
    }
    let width = window.frame.width
    addTeardownBlock { @MainActor in resize(to: width) }
    resize(to: 640)
    app.buttons["Clear"].click()
    editor.click()
    editor.typeText("/")
    let menu = app.descendants(matching: .any)["typeahead-menu"].firstMatch
    XCTAssertTrue(menu.waitForExistence(timeout: 5), app.debugDescription)
    sleep(1)
    XCTAssertLessThan(
      editor.frame.width, menu.frame.width,
      "The editor is not narrower than the menu in \(window.frame).")
    XCTAssertGreaterThanOrEqual(
      menu.frame.minX, window.frame.minX, "menu \(menu.frame), window \(window.frame)")
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

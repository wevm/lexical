import XCTest

/// Smoke tests of the playground on the iOS Simulator: the editor and the
/// tree view load, the toolbar formats the selection, Markdown shortcuts
/// and check lists work, and JSON export shows the document.
final class LexicalPlaygroundUITests: XCTestCase {
  private var app: XCUIApplication!

  override func setUp() {
    continueAfterFailure = false
  }

  @MainActor
  private func launch() {
    app = XCUIApplication()
    app.launch()
  }

  @MainActor private var tree: String { app.staticTexts["tree-view"].label }

  @MainActor
  func testShowsTheWelcomeDocumentInTheTreeView() {
    launch()
    XCTAssertTrue(app.textViews["editor"].waitForExistence(timeout: 10))
    XCTAssertTrue(tree.contains(") heading"), tree)
    XCTAssertTrue(tree.contains("\"Welcome to the playground\""), tree)
    XCTAssertTrue(tree.contains(" selection: range"), tree)
  }

  @MainActor
  func testBoldButtonFormatsTheSelection() {
    launch()
    let editor = app.textViews["editor"]
    XCTAssertTrue(editor.waitForExistence(timeout: 10))
    editor.tap()
    editor.typeText("Hello")
    // Select the word just typed.
    for _ in 0..<5 { app.typeKey(.leftArrow, modifierFlags: .shift) }
    app.buttons["Bold"].tap()
    let formatted = NSPredicate(format: "label CONTAINS %@", "\"Hello\" { format: bold }")
    expectation(for: formatted, evaluatedWith: app.staticTexts["tree-view"])
    waitForExpectations(timeout: 5)
  }

  @MainActor
  func testExportsTheDocumentAsJSON() {
    launch()
    XCTAssertTrue(app.textViews["editor"].waitForExistence(timeout: 10))
    app.buttons["JSON"].tap()
    let text = app.textViews["import-export-text"]
    XCTAssertTrue(text.waitForExistence(timeout: 5))
    let value = text.value as? String ?? ""
    XCTAssertTrue(value.contains("\"type\": \"heading\""), value)
    XCTAssertTrue(value.contains("\"text\": \"Welcome to the playground\""), value)
  }

  /// Clears the document and puts the caret in it.
  @MainActor
  private func clearedEditor() -> XCUIElement {
    let editor = app.textViews["editor"]
    XCTAssertTrue(editor.waitForExistence(timeout: 10))
    app.buttons["Clear"].tap()
    editor.tap()
    return editor
  }

  @MainActor
  private func waitForTree(_ substring: String) {
    let predicate = NSPredicate(format: "label CONTAINS %@", substring)
    expectation(for: predicate, evaluatedWith: app.staticTexts["tree-view"])
    waitForExpectations(timeout: 5)
  }

  @MainActor
  func testMarkdownShortcutMakesAHeading() {
    launch()
    let editor = clearedEditor()
    editor.typeText("# Title")
    waitForTree(") heading")
    waitForTree("\"Title\"")
  }

  @MainActor
  func testTappingACheckBoxTogglesTheItem() {
    launch()
    let editor = clearedEditor()
    app.buttons["Check List"].tap()
    editor.typeText("Task")
    waitForTree(") listitem")
    // The box sits at the start of the item's first line, past the text
    // view's 5-point line padding.
    editor.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 18, dy: 22)).tap()
    app.buttons["JSON"].tap()
    let text = app.textViews["import-export-text"]
    XCTAssertTrue(text.waitForExistence(timeout: 5))
    let value = text.value as? String ?? ""
    XCTAssertTrue(value.contains("\"checked\": true"), value)
  }

  /// The floating link editor (`FloatingLinkEditorPlugin`): the toolbar's
  /// link button opens its URL field, confirming links the selection, the
  /// panel shows the URL while the caret is in the link, and its trash
  /// button unlinks it.
  @MainActor
  func testFloatingLinkEditorLinksAndUnlinks() {
    launch()
    let editor = clearedEditor()
    editor.typeText("Hello")
    for _ in 0..<5 { app.typeKey(.leftArrow, modifierFlags: .shift) }
    // The toolbar scrolls sideways on a phone.
    let linkButton = app.buttons["Insert Link"]
    let bold = app.buttons["Bold"].coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
    bold.press(forDuration: 0.1, thenDragTo: bold.withOffset(CGVector(dx: -250, dy: 0)))
    linkButton.tap()
    let input = app.textFields["link-input"]
    XCTAssertTrue(input.waitForExistence(timeout: 5))
    input.typeText("lexical.dev\n")
    waitForTree(") link")
    // A tap in the link puts the caret in it, which shows the panel.
    editor.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 20, dy: 22)).tap()
    let view = app.buttons["link-view"]
    XCTAssertTrue(view.waitForExistence(timeout: 5))
    XCTAssertEqual(view.label, "https://lexical.dev")
    app.buttons["Remove link"].tap()
    let unlinked = NSPredicate(format: "NOT (label CONTAINS %@)", ") link")
    expectation(for: unlinked, evaluatedWith: app.staticTexts["tree-view"])
    waitForExpectations(timeout: 5)
    XCTAssertFalse(view.exists)
  }
}

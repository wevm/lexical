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

  /// A toolbar button, scrolling the toolbar until it is on screen: like
  /// the web toolbar, it is wider than a phone.
  @MainActor
  private func toolbarButton(_ label: String) -> XCUIElement {
    let button = app.buttons[label]
    let window = app.windows.firstMatch.frame
    let start = app.scrollViews["toolbar"].coordinate(
      withNormalizedOffset: CGVector(dx: 0.7, dy: 0.5))
    // Short drags that hold at the end, so the toolbar has no momentum
    // and stops with the button on screen.
    for _ in 0..<8 where !window.contains(button.frame) {
      start.press(
        forDuration: 0.05, thenDragTo: start.withOffset(CGVector(dx: -100, dy: 0)),
        withVelocity: .slow, thenHoldForDuration: 0.2)
    }
    return button
  }

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
    toolbarButton("Bold").tap()
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

  /// The web playground's keyboard shortcuts, from a hardware keyboard.
  @MainActor
  func testKeyboardShortcutsFormatTheBlock() {
    launch()
    let editor = clearedEditor()
    editor.typeText("hello")
    app.typeKey("1", modifierFlags: [.command, .alternate])
    waitForTree(") heading")
    app.typeKey("7", modifierFlags: [.command, .shift])
    waitForTree(") list")
    waitForTree("listitem")
    app.typeKey("e", modifierFlags: [.command, .shift])
    waitForTree("center")
    app.typeKey("q", modifierFlags: [.control, .shift])
    waitForTree(") quote")
    XCTAssertTrue(tree.contains("\"hello\""), tree)
  }

  /// A hardware keyboard's Tab indents a code line and Shift+Tab outdents
  /// it, as in the web playground.
  @MainActor
  func testShiftTabOutdentsACodeLine() {
    launch()
    let editor = clearedEditor()
    editor.typeText("``` ")
    waitForTree(") code")
    app.typeKey(.tab, modifierFlags: [])
    editor.typeText("x")
    waitForTree(") tab")
    app.typeKey(.tab, modifierFlags: .shift)
    let outdented = NSPredicate(format: "NOT (label CONTAINS %@)", ") tab")
    expectation(for: outdented, evaluatedWith: app.staticTexts["tree-view"])
    waitForExpectations(timeout: 5)
    XCTAssertTrue(tree.contains("\"x\""), tree)
  }

  @MainActor
  func testFontControlsPatchTheSelectionStyle() {
    launch()
    let editor = clearedEditor()
    editor.typeText("Hello")
    for _ in 0..<5 { app.typeKey(.leftArrow, modifierFlags: .shift) }
    app.buttons["Increase font size"].tap()
    XCTAssertEqual(app.textFields["font-size-input"].value as? String, "18")
    app.buttons["Formatting options for font family"].tap()
    app.buttons["Georgia"].tap()
    app.buttons["JSON"].tap()
    let text = app.textViews["import-export-text"]
    XCTAssertTrue(text.waitForExistence(timeout: 5))
    let value = text.value as? String ?? ""
    XCTAssertTrue(value.contains("font-size: 18px;"), value)
    XCTAssertTrue(value.contains("font-family: Georgia;"), value)
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
    toolbarButton("Insert Link").tap()
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

  /// The "/" menu (`ComponentPickerPlugin`): typing filters it, a tap on
  /// an option applies it and removes the query, and the software
  /// keyboard's Return chooses the highlighted option.
  @MainActor
  func testComponentPickerInsertsBlocks() {
    launch()
    let editor = clearedEditor()
    editor.typeText("/head")
    let second = app.descendants(matching: .any)["typeahead-item-1"]
    XCTAssertTrue(second.waitForExistence(timeout: 5))
    XCTAssertTrue(second.label.contains("Heading 2"), second.label)
    second.tap()
    waitForTree(") heading")
    XCTAssertFalse(tree.contains("/head"), tree)
    XCTAssertFalse(app.descendants(matching: .any)["typeahead-menu"].exists)
    editor.typeText("Title\n/quo")
    XCTAssertTrue(
      app.descendants(matching: .any)["typeahead-item-0"].waitForExistence(timeout: 5))
    editor.typeText("\n")
    waitForTree(") quote")
    XCTAssertFalse(tree.contains("/quo"), tree)
  }
}

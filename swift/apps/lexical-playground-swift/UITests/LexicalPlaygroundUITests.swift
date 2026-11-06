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
    // A plain hardware key first: on the iOS 26 Simulator, after an earlier
    // test's app got chords from XCUITest (Shift+Left in
    // testFontControlsPatchTheSelectionStyle), the next app's chords arrive
    // without their modifiers until a key without one does. At the end of
    // the text, Right Arrow changes nothing.
    app.typeKey(.rightArrow, modifierFlags: [])
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

  /// The toolbar's language dropdown sets the code block's language, which
  /// the code action menu at the block shows (CodeActionMenuPlugin).
  @MainActor
  func testCodeLanguagePickerAndActionMenu() {
    launch()
    let editor = clearedEditor()
    editor.typeText("``` let x")
    waitForTree(") code")
    let label = app.staticTexts["code-highlight-language"]
    XCTAssertTrue(label.waitForExistence(timeout: 5))
    XCTAssertEqual(label.label, "(No language)")
    XCTAssertTrue(app.buttons["code-copy"].exists)
    let picker = app.buttons["code-language"]
    XCTAssertTrue(picker.waitForExistence(timeout: 5))
    picker.tap()
    app.buttons["JavaScript"].tap()
    expectation(for: NSPredicate(format: "label == 'JavaScript'"), evaluatedWith: label)
    waitForExpectations(timeout: 5)
    // Out of the block, the menu hides.
    app.buttons["code-copy"].tap()
    editor.typeText("\n\n\n")
    waitForTree(") paragraph")
    expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: label)
    waitForExpectations(timeout: 5)
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

  /// `MentionsPlugin`: "@" and a name list the people the dummy lookup
  /// finds, and Return turns the query into a mention.
  @MainActor
  func testMentionsInsertAMentionNode() {
    launch()
    let editor = clearedEditor()
    editor.typeText("Hi @Luk")
    let first = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(first.waitForExistence(timeout: 5))
    XCTAssertTrue(first.label.contains("Luke Skywalker"), first.label)
    editor.typeText("\n")
    waitForTree(") mention \"Luke Skywalker\"")
    XCTAssertFalse(tree.contains("@Luk"), tree)
  }

  /// `EmojiPickerPlugin`: ":" and a keyword list matching emoji, and a tap
  /// inserts one in place of the query.
  @MainActor
  func testEmojiPickerInsertsAnEmoji() {
    launch()
    let editor = clearedEditor()
    editor.typeText("I am :hap")
    let first = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(first.waitForExistence(timeout: 5))
    XCTAssertTrue(first.label.contains("grinning"), first.label)
    first.tap()
    waitForTree("\"I am \u{1F600}")
    XCTAssertFalse(tree.contains(":hap"), tree)
  }

  /// `AutocompleteExtension`, turned on in the settings: the rest of the
  /// word shows as ghost text, and a swipe right accepts it.
  @MainActor
  func testAutocompleteSuggestsAndSwipeAccepts() {
    launch()
    app.buttons["Settings"].tap()
    let toggle = app.switches["Autocomplete"]
    XCTAssertTrue(toggle.waitForExistence(timeout: 5))
    toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
    app.buttons["Done"].tap()
    let editor = clearedEditor()
    editor.typeText("Some informa")
    let ghost = app.staticTexts["autocomplete-ghost"]
    XCTAssertTrue(ghost.waitForExistence(timeout: 5))
    XCTAssertTrue(ghost.label.hasPrefix("tion"), ghost.label)
    editor.swipeRight()
    waitForTree("\"Some information\"")
  }

  /// `FloatingTextFormatToolbarPlugin`, which on iOS adds the popup's
  /// formats to the edit menu: selecting a word and choosing Bold there
  /// formats it.
  @MainActor
  func testEditMenuFormatsTheSelection() {
    launch()
    let editor = clearedEditor()
    editor.typeText("Hello")
    editor.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 25, dy: 18))
      .doubleTap()
    let bold = app.menuItems["Bold"]
    XCTAssertTrue(bold.waitForExistence(timeout: 5), app.debugDescription)
    bold.tap()
    waitForTree("\"Hello\" { format: bold }")
  }

  /// `ImagesExtension`: the "/" menu's Image option opens the insert
  /// dialog, whose sample inserts an image node that draws in the editor.
  @MainActor
  func testInsertsAnImageFromTheComponentPicker() {
    launch()
    let editor = clearedEditor()
    editor.typeText("/imag")
    let option = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(option.waitForExistence(timeout: 5))
    XCTAssertTrue(option.label.contains("Image"), option.label)
    option.tap()
    let sample = app.buttons["image-modal-option-sample"]
    XCTAssertTrue(sample.waitForExistence(timeout: 5))
    sample.tap()
    waitForTree(") image")
    XCTAssertFalse(tree.contains("/imag"), tree)
    let image = app.descendants(matching: .any)["editor-image"]
    XCTAssertTrue(image.waitForExistence(timeout: 5), app.debugDescription)
    // A tap selects it, and dragging its top right handle makes it wider
    // (`ImageResizer`). The top one, since a keyboard onboarding card on a
    // fresh Simulator can leave the image's bottom under the debug view.
    image.tap()
    waitForTree("selection: node")
    let width = image.frame.width
    let corner = image.coordinate(withNormalizedOffset: CGVector(dx: 1, dy: 0))
      .withOffset(CGVector(dx: -4, dy: 4))
    corner.press(
      forDuration: 0.1, thenDragTo: corner.withOffset(CGVector(dx: 60, dy: -10)),
      withVelocity: .slow, thenHoldForDuration: 0.1)
    let wider = NSPredicate { _, _ in image.frame.width > width + 30 }
    expectation(for: wider, evaluatedWith: nil)
    waitForExpectations(timeout: 5)
  }

  /// `ImageResizer`'s Add Caption button shows the image's caption, a
  /// nested editor that takes the typing, and an empty caption hides again
  /// when it loses focus (`DisableCaptionOnBlur`).
  @MainActor
  func testAddsACaptionToAnImage() {
    launch()
    let editor = clearedEditor()
    editor.typeText("/imag")
    let option = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(option.waitForExistence(timeout: 5))
    option.tap()
    let sample = app.buttons["image-modal-option-sample"]
    XCTAssertTrue(sample.waitForExistence(timeout: 5))
    sample.tap()
    let image = app.descendants(matching: .any)["editor-image"]
    XCTAssertTrue(image.waitForExistence(timeout: 5), app.debugDescription)
    image.tap()
    waitForTree("selection: node")
    let button = app.buttons["image-caption-button"]
    XCTAssertTrue(button.waitForExistence(timeout: 5), app.debugDescription)
    button.tap()
    let caption = app.textViews["image-caption"]
    XCTAssertTrue(caption.waitForExistence(timeout: 5), app.debugDescription)
    caption.typeText("A yellow flower")
    let typed = NSPredicate(format: "value CONTAINS %@", "A yellow flower")
    expectation(for: typed, evaluatedWith: caption)
    waitForExpectations(timeout: 5)
    attachScreenshot("image-caption")
    // Emptied and left, the caption hides.
    caption.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 15))
    editor.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.95)).tap()
    let gone = NSPredicate(format: "exists == false")
    expectation(for: gone, evaluatedWith: caption)
    waitForExpectations(timeout: 5)
  }

  /// The playground's text entities and the "/" menu's Page Break and date
  /// options: a keyword becomes a keyword node, brackets a special text
  /// node once the setting is on, and the options insert their nodes.
  @MainActor
  func testInsertsPlaygroundNodes() {
    launch()
    app.buttons["Settings"].tap()
    let toggle = app.switches["Use Brackets for Highlighting"]
    XCTAssertTrue(toggle.waitForExistence(timeout: 5))
    toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
    app.buttons["Done"].tap()
    let editor = clearedEditor()
    editor.typeText("Big congrats and [special] text ")
    waitForTree(") keyword \"congrats\"")
    waitForTree(") specialText \"special\"")
    editor.typeText("\n/tom")
    let date = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(date.waitForExistence(timeout: 5))
    XCTAssertTrue(date.label.contains("Tomorrow"), date.label)
    editor.typeText("\n")
    waitForTree(") datetime")
    attachScreenshot("playground-text-nodes")
    editor.typeText("\n/page")
    let pageBreak = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(pageBreak.waitForExistence(timeout: 5))
    XCTAssertTrue(pageBreak.label.contains("Page Break"), pageBreak.label)
    pageBreak.tap()
    waitForTree(") page-break")
    XCTAssertFalse(tree.contains("/page"), tree)
    attachScreenshot("playground-page-break")
  }

  /// Keeps a screenshot in the result bundle, to compare with the web.
  @MainActor
  private func attachScreenshot(_ name: String) {
    let screenshot = XCTAttachment(screenshot: app.screenshot())
    screenshot.name = name
    screenshot.lifetime = .keepAlways
    add(screenshot)
  }
}

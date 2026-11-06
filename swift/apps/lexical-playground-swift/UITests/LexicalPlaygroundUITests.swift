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

  /// Picking a language highlights the block with tree-sitter: its code
  /// splits into typed CodeHighlightNodes, which the theme colors.
  @MainActor
  func testPickingACodeLanguageHighlightsTheBlock() {
    launch()
    let editor = clearedEditor()
    editor.typeText("``` const x = 'a'")
    waitForTree(") code")
    let picker = app.buttons["code-language"]
    XCTAssertTrue(picker.waitForExistence(timeout: 5))
    picker.tap()
    app.buttons["JavaScript"].tap()
    app.buttons["JSON"].tap()
    let text = app.textViews["import-export-text"]
    XCTAssertTrue(text.waitForExistence(timeout: 5))
    let value = text.value as? String ?? ""
    XCTAssertTrue(value.contains("\"highlightType\": \"keyword\""), value)
    XCTAssertTrue(value.contains("\"highlightType\": \"string\""), value)
  }

  /// The opt-in grammars the playground links: SQL highlights its keywords.
  @MainActor
  func testPickingSQLHighlightsTheBlock() {
    launch()
    let editor = clearedEditor()
    editor.typeText("``` SELECT id FROM users")
    waitForTree(") code")
    let picker = app.buttons["code-language"]
    XCTAssertTrue(picker.waitForExistence(timeout: 5))
    picker.tap()
    app.buttons["SQL"].tap()
    app.buttons["JSON"].tap()
    let text = app.textViews["import-export-text"]
    XCTAssertTrue(text.waitForExistence(timeout: 5))
    let value = text.value as? String ?? ""
    XCTAssertTrue(value.contains("\"language\": \"sql\""), value)
    XCTAssertTrue(value.contains("\"highlightType\": \"keyword\""), value)
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
    // A tap in the still-selected link shows the panel and opens the edit
    // menu, which goes below the link, where the panel was: the panel moves
    // out of its way, so its buttons take their taps.
    editor.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 20, dy: 22)).tap()
    let view = app.buttons["link-view"]
    XCTAssertTrue(view.waitForExistence(timeout: 5))
    XCTAssertEqual(view.label, "https://lexical.dev")
    XCTAssertTrue(app.menuItems.firstMatch.waitForExistence(timeout: 5), app.debugDescription)
    let menu = app.menuItems.allElementsBoundByIndex.map(\.frame).reduce(CGRect.null) {
      $0.union($1)
    }
    let remove = app.buttons["Remove link"]
    for button in [view, app.buttons["Edit link"], remove] {
      XCTAssertFalse(button.frame.intersects(menu), "\(button.frame) under the menu \(menu)")
    }
    attachScreenshot("link-editor-below-edit-menu")
    remove.tap()
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
  /// word shows as ghost text in the editor's text, outside the model, and
  /// a swipe right accepts it.
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
    expectation(
      for: NSPredicate(format: "value BEGINSWITH %@", "Some information (SWIPE"),
      evaluatedWith: editor)
    waitForExpectations(timeout: 5)
    waitForTree("\"Some informa\"")
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

  /// `CommentPlugin`: the edit menu's Insert comment opens the input box,
  /// whose Comment wraps the selection in a mark and opens the panel with
  /// the thread, which takes a reply.
  @MainActor
  func testCommentsOnTheSelection() {
    launch()
    let editor = clearedEditor()
    editor.typeText("Hello world")
    editor.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 25, dy: 18))
      .doubleTap()
    XCTAssertTrue(app.menuItems["Bold"].waitForExistence(timeout: 5), app.debugDescription)
    // Insert comment follows the formats, on a later page of the menu.
    let insert = editMenuItem("Insert comment")
    XCTAssertTrue(insert.exists, app.debugDescription)
    insert.tap()
    let input = app.descendants(matching: .any)["comment-input"]
    XCTAssertTrue(input.waitForExistence(timeout: 5), app.debugDescription)
    // The box takes focus as it opens, which a slower run may not have
    // finished, so the tap makes sure.
    input.tap()
    input.typeText("Nice greeting")
    attachScreenshot("playground-comment-input")
    app.buttons["comment-submit"].tap()
    waitForTree(") mark")
    XCTAssertTrue(element(containing: "Nice greeting").waitForExistence(timeout: 5))
    let reply = app.descendants(matching: .any)["comment-reply"]
    reply.tap()
    reply.typeText("Thanks")
    app.buttons["Send"].tap()
    XCTAssertTrue(element(containing: "Thanks").waitForExistence(timeout: 5))
    attachScreenshot("playground-comments-panel")
  }

  /// `FindReplaceExtension`: the panel counts the matches, steps through
  /// them and replaces them all.
  @MainActor
  func testFindAndReplace() {
    launch()
    let editor = clearedEditor()
    editor.typeText("one two one three one")
    app.buttons["Find and Replace"].tap()
    let search = app.textFields["Search text"]
    XCTAssertTrue(search.waitForExistence(timeout: 5), app.debugDescription)
    search.tap()
    search.typeText("one")
    let count = app.staticTexts["find-replace-count"]
    XCTAssertTrue(count.waitForExistence(timeout: 5))
    XCTAssertEqual(count.label, "1 / 3")
    app.buttons["Next match"].tap()
    XCTAssertEqual(count.label, "2 / 3")
    attachScreenshot("playground-find-replace")
    let replace = app.textFields["Replace text"]
    replace.tap()
    replace.typeText("1")
    app.buttons["Replace all matches"].tap()
    waitForTree(#"text "1 two 1 three 1""#)
    XCTAssertEqual(count.label, "No results")
  }

  /// `TableOfContentsPlugin`: the setting lists the headings as they are
  /// typed, and a tap on one selects it.
  @MainActor
  func testTableOfContentsListsTheHeadings() {
    launch()
    app.buttons["Settings"].tap()
    let toggle = app.switches["Table Of Contents"]
    XCTAssertTrue(toggle.waitForExistence(timeout: 5))
    toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
    app.buttons["Done"].tap()
    let editor = clearedEditor()
    editor.typeText("# Getting started\nSome text.\n## Install\n### From the registry\n")
    let list = app.descendants(matching: .any)["table-of-contents"]
    XCTAssertTrue(list.waitForExistence(timeout: 5), app.debugDescription)
    let install = list.buttons["Install"]
    XCTAssertTrue(install.waitForExistence(timeout: 5), app.debugDescription)
    XCTAssertTrue(list.buttons["Getting started"].exists)
    XCTAssertTrue(list.buttons["From the registry"].exists)
    install.tap()
    XCTAssertTrue(install.isSelected)
    attachScreenshot("playground-table-of-contents")
  }

  /// The first element whose label contains `text`.
  @MainActor
  private func element(containing text: String) -> XCUIElement {
    app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text))
      .firstMatch
  }

  /// The embeds (`AutoEmbedPlugin`): the "/" menu's Embed Youtube Video
  /// opens its dialog, whose Embed is enabled once the URL parses, and
  /// inserts a YouTube node in place of the query.
  @MainActor
  func testEmbedsAYouTubeVideoFromTheComponentPicker() {
    launch()
    let editor = clearedEditor()
    editor.typeText("/youtube")
    let option = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(option.waitForExistence(timeout: 5))
    XCTAssertTrue(option.label.contains("Embed Youtube Video"), option.label)
    option.tap()
    let url = app.textFields["youtube-video-embed-modal-url"]
    XCTAssertTrue(url.waitForExistence(timeout: 5))
    let embed = app.buttons["youtube-video-embed-modal-submit-btn"]
    XCTAssertFalse(embed.isEnabled)
    url.tap()
    url.typeText("https://www.youtube.com/watch?v=jNQXAC9IVRw")
    expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: embed)
    waitForExpectations(timeout: 5)
    embed.tap()
    waitForTree(") youtube")
    XCTAssertFalse(tree.contains("/youtube"), tree)
  }

  /// `TweetComponent`: the widget is at most 550 points wide, as on the
  /// web, so on a phone it fits the editor instead of overflowing it.
  @MainActor
  func testEmbedsATweetThatFitsTheWindow() {
    launch()
    let editor = clearedEditor()
    editor.typeText("/tweet")
    let option = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(option.waitForExistence(timeout: 5))
    XCTAssertTrue(option.label.contains("Embed X(Tweet)"), option.label)
    option.tap()
    let url = app.textFields["tweet-embed-modal-url"]
    XCTAssertTrue(url.waitForExistence(timeout: 5))
    url.tap()
    url.typeText("https://x.com/jack/status/20")
    let embed = app.buttons["tweet-embed-modal-submit-btn"]
    expectation(for: NSPredicate(format: "isEnabled == true"), evaluatedWith: embed)
    waitForExpectations(timeout: 5)
    embed.tap()
    waitForTree(") tweet")
    let tweet = app.descendants(matching: .any)["Tweet"].firstMatch
    XCTAssertTrue(tweet.waitForExistence(timeout: 5))
    let window = app.windows.firstMatch.frame
    XCTAssertGreaterThan(tweet.frame.width, 0)
    XCTAssertGreaterThanOrEqual(tweet.frame.minX, window.minX, "\(tweet.frame)")
    XCTAssertLessThanOrEqual(tweet.frame.maxX, window.maxX, "\(tweet.frame)")
  }

  /// `PollExtension`: the "/" menu's Poll asks for the question and
  /// inserts a poll with two options; a vote counts, and a third option
  /// can be added.
  @MainActor
  func testInsertsAnEquationFromTheComponentPickerAndEditsIt() {
    launch()
    let editor = clearedEditor()
    editor.typeText("/equation")
    let option = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(option.waitForExistence(timeout: 5))
    XCTAssertTrue(option.label.contains("Equation"), option.label)
    option.tap()
    let input = app.textFields["equation-input"]
    XCTAssertTrue(input.waitForExistence(timeout: 5))
    input.tap()
    input.typeText("e^{i\\pi} + 1 = 0")
    app.buttons["equation-submit-btn"].tap()
    waitForTree(") equation")
    XCTAssertFalse(tree.contains("/equation"), tree)
    // KaTeX drew it: the view takes the size it reported, wider than the
    // placeholder's.
    let equation = app.descendants(matching: .any)["equation"].firstMatch
    XCTAssertTrue(equation.waitForExistence(timeout: 5))
    let deadline = Date().addingTimeInterval(10)
    while equation.frame.width <= 60, Date() < deadline { usleep(200_000) }
    XCTAssertGreaterThan(equation.frame.width, 60)
    // A double tap edits its LaTeX in place.
    equation.doubleTap()
    let field = app.descendants(matching: .any)["equation-editor"].firstMatch
    XCTAssertTrue(field.waitForExistence(timeout: 5), app.debugDescription)
    XCTAssertEqual(field.value as? String, "e^{i\\pi} + 1 = 0")
  }

  @MainActor
  func testInsertsAPollAndVotesFromTheComponentPicker() {
    launch()
    let editor = clearedEditor()
    editor.typeText("/poll")
    let option = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(option.waitForExistence(timeout: 5))
    XCTAssertTrue(option.label.contains("Poll"), option.label)
    option.tap()
    let question = app.textFields["poll-question-input"]
    XCTAssertTrue(question.waitForExistence(timeout: 5))
    let confirm = app.buttons["poll-confirm"]
    XCTAssertFalse(confirm.isEnabled)
    question.tap()
    question.typeText("Lunch?")
    confirm.tap()
    waitForTree(") poll")
    XCTAssertFalse(tree.contains("/poll"), tree)
    let vote = app.buttons["poll-option-0-vote"]
    XCTAssertTrue(vote.waitForExistence(timeout: 5), app.debugDescription)
    XCTAssertFalse(app.buttons["poll-option-1-remove"].isEnabled)
    vote.tap()
    XCTAssertTrue(app.staticTexts["1 vote"].waitForExistence(timeout: 5))
    app.buttons["poll-add-option"].tap()
    XCTAssertTrue(app.buttons["poll-option-2-vote"].waitForExistence(timeout: 5))
    XCTAssertTrue(app.buttons["poll-option-2-remove"].isEnabled)
  }

  /// The Insert menu's Sticky Note floats a yellow note over the editor;
  /// its buttons toggle its color and delete it.
  @MainActor
  func testInsertsAStickyNoteFromTheInsertMenu() {
    launch()
    _ = clearedEditor()
    toolbarButton("Insert specialized editor node").tap()
    app.buttons["Sticky Note"].firstMatch.tap()
    let note = app.descendants(matching: .any)["sticky-note"]
    XCTAssertTrue(note.waitForExistence(timeout: 5), app.debugDescription)
    waitForTree(") sticky")
    app.buttons["sticky-color"].tap()
    app.buttons["sticky-delete"].tap()
    let gone = NSPredicate(format: "exists == false")
    expectation(for: gone, evaluatedWith: note)
    waitForExpectations(timeout: 5)
    expectation(
      for: NSPredicate(format: "NOT (label CONTAINS ') sticky')"),
      evaluatedWith: app.staticTexts["tree-view"])
    waitForExpectations(timeout: 5)
  }

  /// `ContextMenuPlugin`, which on iOS adds the web's items to the edit
  /// menu: Delete Node there removes the selection's block, and UIKit's own
  /// items stay.
  @MainActor
  func testEditMenuDeletesTheNode() {
    launch()
    let editor = clearedEditor()
    editor.typeText("Hello\nWorld")
    waitForTree("\"World\"")
    editor.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: 25, dy: 18))
      .doubleTap()
    // The formats come first, so Delete Node is on a later page.
    let delete = editMenuItem("Delete Node")
    XCTAssertTrue(delete.exists, app.debugDescription)
    attachScreenshot("edit-menu-context-items")
    delete.tap()
    let gone = NSPredicate(format: "NOT (label CONTAINS %@)", "\"Hello\"")
    expectation(for: gone, evaluatedWith: app.staticTexts["tree-view"])
    waitForExpectations(timeout: 5)
    XCTAssertTrue(tree.contains("\"World\""), tree)
  }

  /// `ImagesExtension`: the "/" menu's Image option opens the insert
  /// dialog, whose sample inserts an image node that draws in the editor.
  @MainActor
  func testInsertsAnImageFromTheComponentPicker() throws {
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
    // A tap selects it, and dragging a handle on its right edge makes it
    // wider (`ImageResizer`). With the keyboard up the image is taller than
    // the editor's visible part, which scrolls to the caret after it, so
    // the handle pressed is one the editor shows: its top right, right or
    // bottom right one.
    image.tap()
    waitForTree("selection: node")
    let width = image.frame.width
    let visible = app.textViews.firstMatch.frame.intersection(app.windows.firstMatch.frame)
    let dy = [4.0, image.frame.height / 2, image.frame.height - 4].first {
      visible.contains(CGPoint(x: image.frame.maxX - 4, y: image.frame.minY + $0))
    }
    let corner = image.coordinate(withNormalizedOffset: .zero)
      .withOffset(CGVector(dx: image.frame.width - 4, dy: try XCTUnwrap(dy, "\(visible)")))
    corner.press(
      forDuration: 0.1, thenDragTo: corner.withOffset(CGVector(dx: 60, dy: -10)),
      withVelocity: .slow, thenHoldForDuration: 0.1)
    let wider = NSPredicate { _, _ in image.frame.width > width + 30 }
    expectation(for: wider, evaluatedWith: nil)
    waitForExpectations(timeout: 5)
  }

  /// A selected image, long-pressed and dragged into the line above, moves
  /// there (`ImagesExtension`'s `DRAGSTART`/`DROP_COMMAND` handlers), as
  /// the web playground drags it. The debug view is off, so the editor is
  /// tall enough for both lines above the keyboard.
  @MainActor
  func testDraggingASelectedImageMovesIt() {
    launch()
    app.buttons["Settings"].tap()
    let toggle = app.switches["Debug View"]
    XCTAssertTrue(toggle.waitForExistence(timeout: 5))
    toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
    app.buttons["Done"].tap()
    let editor = clearedEditor()
    editor.typeText("Hello world\n/imag")
    let option = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(option.waitForExistence(timeout: 5))
    option.tap()
    let sample = app.buttons["image-modal-option-sample"]
    XCTAssertTrue(sample.waitForExistence(timeout: 5))
    sample.tap()
    let image = app.descendants(matching: .any)["editor-image"]
    XCTAssertTrue(image.waitForExistence(timeout: 5), app.debugDescription)
    image.tap()
    // Between "Hello" and " world", on the line above the image's.
    let target = image.coordinate(withNormalizedOffset: .zero)
      .withOffset(CGVector(dx: 42, dy: -14))
    image.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.3)).press(
      forDuration: 1, thenDragTo: target, withVelocity: .slow, thenHoldForDuration: 0.5)
    // The image now splits the first line's text.
    let moved = NSPredicate(
      format: "value BEGINSWITH %@ AND NOT (value BEGINSWITH %@)", "Hello", "Hello world")
    expectation(for: moved, evaluatedWith: editor)
    waitForExpectations(timeout: 5)
    attachScreenshot("image-dragged")
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

  /// `EmojisExtension` turns an emoticon into an emoji in the document and
  /// in an image's caption, and the caption has the mentions menu, as its
  /// nested editor does on the web.
  @MainActor
  func testCaptionHasMentionsAndEmoji() {
    launch()
    let editor = clearedEditor()
    editor.typeText("Smile :) ")
    waitForTree(") emoji \"\u{1F642}\"")
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
    caption.typeText("By @Luk")
    let first = app.descendants(matching: .any)["typeahead-item-0"]
    XCTAssertTrue(first.waitForExistence(timeout: 5), app.debugDescription)
    XCTAssertTrue(first.label.contains("Luke Skywalker"), first.label)
    first.tap()
    caption.typeText(" <3")
    let typed = NSPredicate(format: "value CONTAINS %@", "By Luke Skywalker \u{2764}")
    expectation(for: typed, evaluatedWith: caption)
    waitForExpectations(timeout: 5)
    attachScreenshot("caption-mention-emoji")
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

  /// The open edit menu's action labelled `label`, paging the menu until
  /// it shows (it does not exist when no page has it). A compact menu lists
  /// its actions as menu items in one row; paging past its last one expands
  /// it into a full menu that lists them as buttons.
  @MainActor
  private func editMenuItem(_ label: String) -> XCUIElement {
    let item = app.descendants(matching: .any).matching(
      NSPredicate(
        format: "label == %@ AND (elementType == %d OR elementType == %d)", label,
        XCUIElement.ElementType.menuItem.rawValue, XCUIElement.ElementType.button.rawValue)
    ).firstMatch
    for _ in 0..<8 where !item.waitForExistence(timeout: 2) {
      guard let next = editMenuNextPageButton() else { break }
      next.tap()
    }
    return item
  }

  /// The compact edit menu's next-page button: the first button past the
  /// trailing end of its row of menu items. Found by where it is, since its
  /// label differs across iOS versions ("Forward", "Next Page").
  @MainActor
  private func editMenuNextPageButton() -> XCUIElement? {
    let items = app.menuItems.allElementsBoundByIndex.map(\.frame).filter { !$0.isEmpty }
    guard let first = items.first else { return nil }
    let row = items.filter { abs($0.midY - first.midY) < first.height / 2 }
      .reduce(first) { $0.union($1) }
    return app.buttons.allElementsBoundByIndex
      .filter {
        let frame = $0.frame
        return frame.midX > row.maxX && frame.minY < row.maxY && frame.maxY > row.minY
      }
      .min { $0.frame.minX < $1.frame.minX }
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

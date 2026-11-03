import XCTest

// The demo app through the real keyboard: typing, Return and delete on the
// software keyboard, hardware keyboard shortcuts, paste from another
// process and marked-text IME on the Japanese, Chinese and Korean
// keyboards. Each check reads the editor's model through the app's probe
// (`-uitest`), not the text view. They run on the simulator and on a
// device.

final class LexicalDemoUITests: XCTestCase {
  private var app: XCUIApplication!

  override func setUp() {
    continueAfterFailure = false
  }

  @MainActor
  private func launch(_ arguments: [String] = ["-empty"]) {
    app = XCUIApplication()
    app.launchArguments = arguments + ["-uitest"]
    app.launch()
    editor.tap()
    XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 5), "no software keyboard")
    // The keyboard a previous test switched to stays; start from English.
    try? switchKeyboard(to: "English (US)")
  }

  @MainActor private var editor: XCUIElement {
    app.textViews.firstMatch
  }

  /// Waits until the model's text content is `expected`.
  @MainActor
  private func expectText(
    _ expected: String, file: StaticString = #filePath, line: UInt = #line
  ) {
    let probe = app.staticTexts["model-text"]
    let matches = NSPredicate(format: "label == %@", expected)
    let result = XCTWaiter.wait(
      for: [XCTNSPredicateExpectation(predicate: matches, object: probe)], timeout: 5)
    if result != .completed { attachScreenshot() }
    XCTAssertEqual(
      result, .completed,
      "model text is \(probe.label.debugDescription), JSON \(app.staticTexts["model-json"].label), "
        + "log \(app.staticTexts["model-log"].label)",
      file: file, line: line)
  }

  /// Waits until the model's JSON contains (or not) `fragment`.
  @MainActor
  private func expectJSON(
    _ fragment: String, _ contains: Bool = true, file: StaticString = #filePath,
    line: UInt = #line
  ) {
    let probe = app.staticTexts["model-json"]
    let matches = NSPredicate(
      format: contains ? "label CONTAINS %@" : "NOT (label CONTAINS %@)", fragment)
    let result = XCTWaiter.wait(
      for: [XCTNSPredicateExpectation(predicate: matches, object: probe)], timeout: 5)
    if result != .completed { attachScreenshot() }
    XCTAssertEqual(
      result, .completed,
      "model JSON is \(probe.label), log \(app.staticTexts["model-log"].label)", file: file,
      line: line)
  }

  @MainActor
  private func attachScreenshot() {
    let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
    attachment.lifetime = .keepAlways
    add(attachment)
  }

  /// Starts using the hardware keyboard with a key that changes nothing (the
  /// caret is at the end): the simulator drops the first hardware key after
  /// software keyboard input.
  @MainActor
  private func startHardwareKeyboard() {
    editor.typeKey(XCUIKeyboardKey.rightArrow.rawValue, modifierFlags: [])
  }

  /// Taps the software keyboard's keys for `text`, one by one, so the IME
  /// sees them (`typeText` inserts the text without it).
  @MainActor
  private func tapKeys(_ text: String) {
    for character in text {
      let key = app.keyboards.keys[String(character)]
      (key.exists ? key : app.keyboards.keys[character.uppercased()]).tap()
    }
  }

  /// The keyboard's Return key, which commits a composition.
  @MainActor private var returnKey: XCUIElement {
    app.keyboards.buttons["Return"]
  }

  /// Switches the software keyboard to `name` from the globe key's list,
  /// or skips the test when that keyboard is not enabled (enable it in
  /// Settings > General > Keyboard > Keyboards, or on the simulator with
  /// `defaults write -g AppleKeyboards`).
  @MainActor
  private func switchKeyboard(to name: String) throws {
    let next = app.buttons["Next keyboard"]
    guard next.waitForExistence(timeout: 2) else { throw XCTSkip("no globe key") }
    next.press(forDuration: 1.2)
    let cell = app.tables["InputSwitcherTable"].cells[name]
    guard cell.waitForExistence(timeout: 2) else {
      let names = app.tables["InputSwitcherTable"].cells.allElementsBoundByIndex.map(\.label)
      app.otherElements["PopoverDismissRegion"].tap()
      throw XCTSkip("the \(name) keyboard is not enabled; there are \(names)")
    }
    cell.tap()
  }

  @MainActor
  func testTypesDeletesAndEntersOnTheSoftwareKeyboard() {
    launch()
    editor.typeText("Hello")
    expectText("Hello")
    returnKey.tap()
    editor.typeText("world")
    expectText("Hello\n\nworld")
    app.keyboards.keys["delete"].tap()
    expectText("Hello\n\nworl")
    expectJSON(#""text":"worl""#)
  }

  @MainActor
  func testHardwareKeyboardFormatsUndoesAndRedoes() {
    launch()
    editor.typeText("Bold")
    startHardwareKeyboard()
    editor.typeKey("a", modifierFlags: .command)
    editor.typeKey("b", modifierFlags: .command)
    expectJSON(#""format":1"#)
    editor.typeKey("z", modifierFlags: .command)
    expectJSON(#""format":1"#, false)
    expectText("Bold")
    editor.typeKey("z", modifierFlags: [.command, .shift])
    expectJSON(#""format":1"#)
  }

  @MainActor
  func testUndoesTypingWithTheHardwareKeyboard() {
    launch()
    editor.typeText("One")
    expectText("One")
    startHardwareKeyboard()
    editor.typeKey("z", modifierFlags: .command)
    expectText("")
  }

  @MainActor
  func testPastesTextFromAnotherApp() {
    launch()
    // The test runner is another process, so this is another app's copy.
    UIPasteboard.general.string = "From elsewhere"
    startHardwareKeyboard()
    editor.typeKey("v", modifierFlags: .command)
    expectText("From elsewhere")
  }

  @MainActor
  func testComposesJapaneseThroughMarkedText() throws {
    launch()
    try switchKeyboard(to: "日本語ローマ字")
    tapKeys("nihon")
    // Composing text is in the model, as in JS; a last n waits for the
    // next key.
    expectText("にほn")
    tapKeys("n")
    expectText("にほん")
    // Return commits the composition (it reads 確定 meanwhile).
    returnKey.tap()
    expectText("にほん")
    tapKeys("go")
    expectText("にほんご")
    expectJSON(#""text":"にほんご""#)
  }

  @MainActor
  func testComposesPinyinThroughMarkedText() throws {
    launch()
    try switchKeyboard(to: "简体拼音")
    tapKeys("nihao")
    // The composing pinyin is in the model (its syllables apart by a
    // space the keyboard picks); space commits the first candidate.
    let composing = app.staticTexts["model-text"].label
    XCTAssertEqual(composing.filter { !$0.isWhitespace }, "nihao", composing.debugDescription)
    app.keyboards.keys["space"].tap()
    expectText("你好")
  }

  @MainActor
  func testComposesKoreanSyllables() throws {
    launch()
    try switchKeyboard(to: "한국어")
    for jamo in ["ㅎ", "ㅏ", "ㄴ"] {
      app.keyboards.keys[jamo].tap()
    }
    expectText("한")
  }

  @MainActor
  func testOpensTheDemoDocument() {
    launch([])
    let probe = app.staticTexts["model-text"]
    XCTAssertTrue(probe.label.hasPrefix("Lexical for Swift"))
  }
}

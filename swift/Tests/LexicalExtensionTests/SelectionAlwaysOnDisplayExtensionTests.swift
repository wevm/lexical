import Lexical
import LexicalExtension
import Testing

// Upstream has no unit test for SelectionAlwaysOnDisplayExtension (those of
// markSelection are dom-only skips); these check the port's contract with
// the root element.

/// A root that counts the marks in effect, as a view shows its selection
/// without the focus while one is.
private final class MarkingRoot: LexicalRootElement {
  var isConnected: Bool { true }
  var marks = 0
  func selectionAlwaysOnDisplay() -> () -> Void {
    marks += 1
    return { self.marks -= 1 }
  }
}

@Suite struct SelectionAlwaysOnDisplayExtensionTests {
  @Test func marksEachRootWhileEnabled() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "selection-test", dependencies: [SelectionAlwaysOnDisplayExtension]))
    defer { editor.dispose() }
    let first = MarkingRoot()
    try editor.setRootElement(first)
    #expect(first.marks == 1)
    // A new root is marked and the old one released.
    let second = MarkingRoot()
    try editor.setRootElement(second)
    #expect((first.marks, second.marks) == (0, 1))
    let disabled = try getExtensionDependencyFromEditor(
      editor, SelectionAlwaysOnDisplayExtension
    ).output.disabled
    disabled.value = true
    #expect(second.marks == 0)
    disabled.value = false
    #expect(second.marks == 1)
    editor.dispose()
    #expect(second.marks == 0)
  }

  @Test func disabledMarksNothing() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "selection-test",
        dependencies: [configExtension(SelectionAlwaysOnDisplayExtension) { $0.disabled = true }]))
    defer { editor.dispose() }
    let root = MarkingRoot()
    try editor.setRootElement(root)
    #expect(root.marks == 0)
  }
}

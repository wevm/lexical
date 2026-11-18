import Lexical
import LexicalExtension
import Testing

// WatchEditableExtension has no upstream test; these check its contract.

@Suite struct WatchEditableExtensionTests {
  @Test func signalFollowsEditableWhileWatched() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "watch-editable-test", dependencies: [WatchEditableExtension]))
    defer { editor.dispose() }
    let editable = try getExtensionDependencyFromEditor(editor, WatchEditableExtension).output
    var seen: [Bool] = []
    let stop = try effect { seen.append(editable.value) }
    try editor.setEditable(false)
    try editor.setEditable(false)
    try editor.setEditable(true)
    #expect(seen == [true, false, true])
    stop()
    // Unwatched, it takes the editor's state when watched again.
    try editor.setEditable(false)
    #expect(editable.peek() == true)
    let restart = try effect { _ = editable.value }
    defer { restart() }
    #expect(editable.peek() == false)
  }
}

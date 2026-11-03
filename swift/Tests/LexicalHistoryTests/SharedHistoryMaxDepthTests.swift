import Lexical
import LexicalExtension
import LexicalHistory
import Testing

// Port of packages/lexical-history/src/__tests__/unit/SharedHistoryMaxDepth.test.ts.

private let PARENT_MAX_DEPTH = 7
private let PARENT_DELAY: Double = 123

private func buildParent() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "shared-max-depth-parent",
      dependencies: [
        configExtension(HistoryExtension) {
          $0.delay = PARENT_DELAY
          $0.maxDepth = PARENT_MAX_DEPTH
        }
      ]))
}

private func buildChild() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "shared-max-depth-child", dependencies: [SharedHistoryExtension]))
}

@Suite struct SharedHistoryMaxDepthTests {
  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/SharedHistoryMaxDepth.test.ts::SharedHistoryExtension forwards the parent history signals > the child adopts the parent maxDepth"
    ))
  func childAdoptsParentMaxDepth() throws {
    let parent = try buildParent()
    defer { parent.dispose() }
    let child = try buildChild()
    defer { child.dispose() }

    let shared = try getExtensionDependencyFromEditor(child, SharedHistoryExtension)
    try shared.output.parentEditor.set(parent)

    let parentHistory = try getExtensionDependencyFromEditor(parent, HistoryExtension)
    let childHistory = try getExtensionDependencyFromEditor(child, HistoryExtension)

    // Premise: the parent really carries the configured cap, and the child's
    // own default differs from it.
    #expect(parentHistory.output.maxDepth.peek() == PARENT_MAX_DEPTH)

    // The child pushes onto the parent's shared undoStack, so it has to apply
    // the parent's cap rather than its own default.
    #expect(childHistory.output.maxDepth.peek() == PARENT_MAX_DEPTH)
  }

  @Test(
    .upstream(
      "lexical-history/src/__tests__/unit/SharedHistoryMaxDepth.test.ts::SharedHistoryExtension forwards the parent history signals > the already-forwarded signals still arrive"
    ))
  func forwardedSignalsArrive() throws {
    let parent = try buildParent()
    defer { parent.dispose() }
    let child = try buildChild()
    defer { child.dispose() }

    let shared = try getExtensionDependencyFromEditor(child, SharedHistoryExtension)
    try shared.output.parentEditor.set(parent)

    let parentOutput = try getExtensionDependencyFromEditor(parent, HistoryExtension).output
    let childOutput = try getExtensionDependencyFromEditor(child, HistoryExtension).output
    #expect(childOutput.delay.peek() == parentOutput.delay.peek())
    #expect(childOutput.historyState.peek() === parentOutput.historyState.peek())
    // Functions have no identity in Swift, so the clock is compared by what
    // it reads; the parent's default clock is monotonic.
    #expect(childOutput.now.peek()() >= parentOutput.now.peek()() - 1000)
    #expect(childOutput.maxDepth.peek() == parentOutput.maxDepth.peek())
    #expect(childOutput.disabled.peek() == parentOutput.disabled.peek())
  }
}

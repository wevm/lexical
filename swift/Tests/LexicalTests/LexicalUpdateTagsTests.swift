import Lexical
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/LexicalUpdateTags.test.ts.
@Suite struct LexicalUpdateTagsTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > Built-in update tags work correctly"
    ))
  func builtInTags() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let builtInTags = [
        HISTORIC_TAG, HISTORY_PUSH_TAG, HISTORY_MERGE_TAG, PASTE_TAG, CUT_TAG, COLLABORATION_TAG,
        SKIP_DOM_SELECTION_TAG, SKIP_SCROLL_INTO_VIEW_TAG,
      ]
      for tag in builtInTags {
        try addUpdateTag(tag)
        #expect(try hasUpdateTag(tag))
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > $addUpdateTag and $hasUpdateTag work correctly"
    ))
  func addAndHas() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let tag = "test-tag"
      #expect(try !hasUpdateTag(tag))
      try addUpdateTag(tag)
      #expect(try hasUpdateTag(tag))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > Multiple update tags can be added"
    ))
  func multiple() throws {
    let editor = try createTestEditor()
    try update(editor) {
      let tags = ["tag1", "tag2", "tag3"]
      for tag in tags {
        try addUpdateTag(tag)
      }
      for tag in tags {
        #expect(try hasUpdateTag(tag))
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > Update tags via editor.update() options work"
    ))
  func viaOptions() throws {
    let editor = try createTestEditor()
    let tag = "test-tag"
    var hasTag = false
    try editor.update(tag: tag) {
      hasTag = try hasUpdateTag(tag)
    }
    drainMicrotasks()
    #expect(hasTag)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > Update tags are cleared after update"
    ))
  func cleared() throws {
    let editor = try createTestEditor()
    let tag = HISTORIC_TAG
    try update(editor) {
      try addUpdateTag(tag)
      #expect(try hasUpdateTag(tag))
    }
    var hasTag = false
    try update(editor) {
      hasTag = try hasUpdateTag(tag)
    }
    #expect(!hasTag)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > Update tags affect editor behavior"
    ))
  func affectBehavior() throws {
    let editor = try createTestEditor()
    var calls: [[UpdateTag]] = []
    _ = editor.registerUpdateListener { payload in
      calls.append(payload.tags.array)
    }
    try editor.update(tag: SKIP_DOM_SELECTION_TAG) {
      try getRoot().append(createParagraphNode())
    }
    drainMicrotasks()
    #expect(calls.contains { $0.contains(SKIP_DOM_SELECTION_TAG) })
  }
}

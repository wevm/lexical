import Lexical
import LexicalTesting
import LexicalUtils
import Testing

// Port of packages/lexical-utils/src/__tests__/unit/iterators.test.tsx.

private func run(_ body: @escaping () throws -> Void) throws {
  let editor = try createHeadlessTestEditor()
  try editor.update(discrete: true, body)
}

@Suite struct IteratorsTests {
  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/iterators.test.tsx::$firstToLastIterator > Iterates from first to last"
    ))
  func firstToLast() throws {
    try run {
      let parent = try fiveTextNodes()
      #expect(try texts(parent) == ["0", "1", "2", "3", "4"])
      #expect(
        try firstToLastIterator(parent).map { try $0.getTextContent() } == [
          "0", "1", "2", "3", "4",
        ])
      #expect(try texts(parent) == ["0", "1", "2", "3", "4"])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/iterators.test.tsx::$firstToLastIterator > Can handle node removal"
    ))
  func firstToLastRemoval() throws {
    try run {
      let parent = try fiveTextNodes()
      let result = try firstToLastIterator(parent).map { node in
        let text = try node.getTextContent()
        try node.remove()
        return text
      }
      #expect(result == ["0", "1", "2", "3", "4"])
      #expect(try parent.getChildren().isEmpty)
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/iterators.test.tsx::$firstToLastIterator > Detects cycles when nodes move incorrectly"
    ))
  func firstToLastCycle() throws {
    try run {
      let parent = try fiveTextNodes()
      let error = #expect(throws: LexicalError.self) {
        _ = try firstToLastIterator(parent).map { node in
          try parent.append(node)
          return try node.getTextContent()
        }
      }
      #expect(error.map { "\($0)".contains("$childIterator: Cycle detected") } == true)
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/iterators.test.tsx::$firstToLastIterator > Can handle nodes moving in the other direction"
    ))
  func firstToLastMovingBack() throws {
    try run {
      let parent = try fiveTextNodes()
      let result = try firstToLastIterator(parent).map { node in
        let text = try node.getTextContent()
        if try node.getPreviousSibling() != nil { try parent.splice(0, 0, [node]) }
        return text
      }
      #expect(result == ["0", "1", "2", "3", "4"])
      #expect(try texts(parent) == ["4", "3", "2", "1", "0"])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/iterators.test.tsx::$lastToFirstIterator > Iterates from last to first"
    ))
  func lastToFirst() throws {
    try run {
      let parent = try fiveTextNodes()
      #expect(
        try lastToFirstIterator(parent).map { try $0.getTextContent() } == [
          "4", "3", "2", "1", "0",
        ])
      #expect(try texts(parent) == ["0", "1", "2", "3", "4"])
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/iterators.test.tsx::$lastToFirstIterator > Can handle node removal"
    ))
  func lastToFirstRemoval() throws {
    try run {
      let parent = try fiveTextNodes()
      let result = try lastToFirstIterator(parent).map { node in
        let text = try node.getTextContent()
        try node.remove()
        return text
      }
      #expect(result == ["4", "3", "2", "1", "0"])
      #expect(try parent.getChildren().isEmpty)
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/iterators.test.tsx::$lastToFirstIterator > Can handle nodes moving in the other direction"
    ))
  func lastToFirstMoving() throws {
    try run {
      let parent = try fiveTextNodes()
      let result = try lastToFirstIterator(parent).map { node in
        let text = try node.getTextContent()
        try parent.append(node)
        return text
      }
      #expect(result == ["4", "3", "2", "1", "0"])
      #expect(try texts(parent) == ["4", "3", "2", "1", "0"])
    }
  }
}

import Lexical
import LexicalLink
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts.

private func createInitializedEditor() throws -> Editor {
  let editor = try createTestEditor()
  try editor.update(discrete: true) {
    try getRoot().clear().append(
      try createParagraphNode().append(
        try createTextNode("01234"), try createTextNode("56789").setFormat("bold")))
  }
  return editor
}

private func createTextSelection(_ start: Int, _ end: Int) throws -> RangeSelection {
  var i = 0
  let selection = createRangeSelection()
  for node in try getRoot().getAllTextNodes() {
    let len = try node.getTextContentSize()
    let j = i + len
    if start >= i && start <= j {
      try selection.anchor.set(node.getKey(), start - i, .text)
    }
    if end >= i && end <= j {
      try selection.focus.set(node.getKey(), end - i, .text)
    }
    i = j
  }
  return selection
}

private func fullSize(_ nodes: [TextNode]) throws -> Int {
  try nodes.reduce(0) { try $0 + $1.getTextContentSize() }
}

struct SliceEmptyEndpointsCase: Sendable, CustomTestStringConvertible {
  let offset: Int
  let backward: Bool
  let outer: Bool
  let mutates: SliceMutation
  var testDescription: String {
    "empty equivalent endpoints (offset=\(offset), backward=\(backward), outer=\(outer), mutates=\(mutates == .clone ? "clone" : "self"))"
  }

  static let all: [SliceEmptyEndpointsCase] = [0, 2].flatMap { offset in
    [false, true].flatMap { backward in
      [false, true].flatMap { outer in
        [SliceMutation.clone, SliceMutation.`self`].map {
          SliceEmptyEndpointsCase(offset: offset, backward: backward, outer: outer, mutates: $0)
        }
      }
    }
  }
}

@Suite struct SliceSelectedTextNodeContentTests {
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=false, outer=false, mutates=clone)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=false, outer=false, mutates=self)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=false, outer=true, mutates=clone)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=false, outer=true, mutates=self)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=true, outer=false, mutates=clone)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=true, outer=false, mutates=self)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=true, outer=true, mutates=clone)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=0, backward=true, outer=true, mutates=self)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=false, outer=false, mutates=clone)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=false, outer=false, mutates=self)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=false, outer=true, mutates=clone)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=false, outer=true, mutates=self)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=true, outer=false, mutates=clone)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=true, outer=false, mutates=self)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=true, outer=true, mutates=clone)"
    ),
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > empty equivalent endpoints (offset=2, backward=true, outer=true, mutates=self)"
    ),
    arguments: SliceEmptyEndpointsCase.all)
  func emptyEquivalentEndpoints(_ c: SliceEmptyEndpointsCase) throws {
    let editor = try createTestEditor(nodes: [.klass(LinkNode.self)])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let text = try createTextNode("jh")
      let link = try createLinkNode("https://example.com").append(text)
      let paragraph = try createParagraphNode().append(link)
      try getRoot().append(paragraph)
      let selection = createRangeSelection()
      let (textPoint, elementPoint) =
        c.backward ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
      try textPoint.set(text.getKey(), c.offset, .text)
      try elementPoint.set(
        (c.outer ? paragraph as ElementNode : link).getKey(), c.offset == 0 ? 0 : 1, .element)
      let slice = try sliceSelectedTextNodeContent(selection, text, c.mutates)
      #expect(slice.__text == "")
      if c.mutates == .clone {
        #expect(slice !== text)
        #expect(text.__text == "jh")
        #expect(try getNodeByKey(text.getKey()) === text)
      } else {
        #expect(slice === text)
      }
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > clone > does not clone with full selection (both nodes)"
    ))
  func fullSelectionDoesNotClone() throws {
    let editor = try createInitializedEditor()
    defer { editor.dispose() }
    try editor.read { () throws in
      let textNodes = try getRoot().getAllTextNodes()
      let selection = try createTextSelection(0, fullSize(textNodes))
      for node in textNodes {
        #expect(try sliceSelectedTextNodeContent(selection, node, .clone) === node)
      }
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > clone > clones only with partial selection (last node)"
    ))
  func partialLastNode() throws {
    let editor = try createInitializedEditor()
    defer { editor.dispose() }
    try editor.read { () throws in
      let textNodes = try getRoot().getAllTextNodes()
      let selection = try createTextSelection(0, fullSize(textNodes) - 1)
      let lastNode = textNodes.last
      for node in textNodes {
        let slice = try sliceSelectedTextNodeContent(selection, node, .clone)
        if node === lastNode {
          #expect(slice !== node)
          #expect(try slice.getTextContent() == node.getTextContent().jsSlice(0, -1))
        } else {
          #expect(slice === node)
        }
      }
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > clone > clones only with partial selection (first node)"
    ))
  func partialFirstNode() throws {
    let editor = try createInitializedEditor()
    defer { editor.dispose() }
    try editor.read { () throws in
      let textNodes = try getRoot().getAllTextNodes()
      let selection = try createTextSelection(1, fullSize(textNodes))
      let firstNode = textNodes.first
      for node in textNodes {
        let slice = try sliceSelectedTextNodeContent(selection, node, .clone)
        if node === firstNode {
          #expect(slice !== node)
          #expect(try slice.getTextContent() == node.getTextContent().jsSlice(1))
        } else {
          #expect(slice === node)
        }
      }
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/$sliceSelectedTextNodeContent.test.ts::$sliceSelectedTextNodeContent > clone > can slice a node from both sides"
    ))
  func sliceBothSides() throws {
    let editor = try createInitializedEditor()
    defer { editor.dispose() }
    try editor.read { () throws in
      let node = try #require(try getRoot().getAllTextNodes().first)
      let originalText = try node.getTextContent()
      let selection = try createTextSelection(1, 3)
      let slice = try sliceSelectedTextNodeContent(selection, node, .clone)
      #expect(slice !== node)
      #expect(try slice.getTextContent() == node.getTextContent().jsSlice(1, 3))
      try slice.setTextContent("different")
      // no side-effects are observed
      #expect(try node.getTextContent() == originalText)
      #expect(try slice.getTextContent() == "different")
      #expect(try getNodeByKey(node.getKey()) === node)
    }
  }
}

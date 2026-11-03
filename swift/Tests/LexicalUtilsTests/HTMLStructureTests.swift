import Foundation
import Lexical
import LexicalHTML
import LexicalTesting
import LexicalUtils
import Testing

// Ports of packages/lexical-utils/src/__tests__/unit
// LexicalUtilsInsertNodeToNearestRoot.test.tsx and LexicalUtilsSplitNode.test.tsx.
// Each case imports its document from HTML, edits it, and compares the
// exported HTML in the same update, so adjacent text nodes stay unmerged.
// The list cases need @lexical/list and are skipped.

private func importDocument(_ editor: Editor, _ html: String) throws {
  let dom = DOMParser().parseFromString(html)
  try getRoot().clear().append(contentsOf: try generateNodesFromDOM(editor, dom))
}

/// `$generateHtmlFromNodes(editor)` without list `value` attributes.
private func exportedHtml(_ editor: Editor) throws -> String {
  try generateHtmlFromNodes(editor).replacingOccurrences(
    of: #"\svalue="\d{1,}""#, with: "", options: .regularExpression)
}

struct InsertCase: CustomTestStringConvertible, Sendable {
  let name: String
  let initialHtml: String
  let expectedHtml: String
  let selectionPath: [Int]
  let selectionOffset: Int
  var testDescription: String { name }
}

private let insertCases: [InsertCase] = [
  InsertCase(
    name: "insert into paragraph in between two text nodes",
    initialHtml: "<p><span>Helloworld</span></p>",
    expectedHtml:
      #"<p><span style="white-space: pre-wrap;">Hello</span></p><test-decorator></test-decorator><p><span style="white-space: pre-wrap;">world</span></p>"#,
    selectionPath: [0, 0], selectionOffset: 5),
  InsertCase(
    name: "insert into empty paragraph",
    initialHtml: "<p></p>",
    expectedHtml: "<p><br></p><test-decorator></test-decorator><p><br></p>",
    selectionPath: [0], selectionOffset: 0),
  InsertCase(
    name: "insert in the end of paragraph",
    initialHtml: "<p>Hello world</p>",
    expectedHtml:
      #"<p><span style="white-space: pre-wrap;">Hello world</span></p><test-decorator></test-decorator><p><br></p>"#,
    selectionPath: [0, 0], selectionOffset: 11),
  InsertCase(
    name: "insert in the end of paragraph that is followed by another block",
    initialHtml: "<p>Hello world</p><p>Second</p>",
    expectedHtml:
      #"<p><span style="white-space: pre-wrap;">Hello world</span></p><test-decorator></test-decorator><p><span style="white-space: pre-wrap;">Second</span></p>"#,
    selectionPath: [0, 0], selectionOffset: 11),
  InsertCase(
    name: "insert in the beginning of paragraph",
    initialHtml: "<p>Hello world</p>",
    expectedHtml:
      #"<p><br></p><test-decorator></test-decorator><p><span style="white-space: pre-wrap;">Hello world</span></p>"#,
    selectionPath: [0, 0], selectionOffset: 0),
  InsertCase(
    name: "insert with selection on root start",
    initialHtml:
      "<test-decorator></test-decorator><p><span>Before</span></p><p><span>After</span></p>",
    expectedHtml:
      #"<test-decorator></test-decorator><test-decorator></test-decorator><p><span style="white-space: pre-wrap;">Before</span></p><p><span style="white-space: pre-wrap;">After</span></p>"#,
    selectionPath: [], selectionOffset: 0),
  InsertCase(
    name: "insert with selection on root child",
    initialHtml: "<p>Before</p><p>After</p>",
    expectedHtml:
      #"<p><span style="white-space: pre-wrap;">Before</span></p><test-decorator></test-decorator><p><span style="white-space: pre-wrap;">After</span></p>"#,
    selectionPath: [], selectionOffset: 1),
  InsertCase(
    name: "insert with selection on root end",
    initialHtml: "<p>Before</p>",
    expectedHtml:
      #"<p><span style="white-space: pre-wrap;">Before</span></p><test-decorator></test-decorator>"#,
    selectionPath: [], selectionOffset: 1),
]

@Suite struct LexicalUtilsInsertNodeToNearestRootTests {
  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert into paragraph in between two text nodes"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert into empty paragraph"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert in the end of paragraph"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert in the end of paragraph that is followed by another block"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert in the beginning of paragraph"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert with selection on root start"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert with selection on root child"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRoot.test.tsx::LexicalUtils#insertNodeToNearestRoot > insert with selection on root end"
    ),
    arguments: insertCases)
  func insert(_ testCase: InsertCase) throws {
    let editor = try createTestEditor(headless: true)
    defer { editor.dispose() }
    try update(editor) {
      try importDocument(editor, testCase.initialHtml)
      var selectionNode: LexicalNode = try getRoot()
      for index in testCase.selectionPath {
        let element = try #require(selectionNode as? ElementNode)
        selectionNode = try #require(try element.getChildAtIndex(index))
      }
      // `select()` would normalize onto a text node; keep the point as given.
      let selection = createRangeSelection()
      let type: PointType = selectionNode is ElementNode ? .element : .text
      try selection.anchor.set(selectionNode.getKey(), testCase.selectionOffset, type)
      try selection.focus.set(selectionNode.getKey(), testCase.selectionOffset, type)
      try setSelection(selection)
      try insertNodeToNearestRoot(try createTestDecoratorNode().setIsInline(false))
      #expect(try exportedHtml(editor) == testCase.expectedHtml)
    }
  }
}

struct SplitCase: CustomTestStringConvertible, Sendable {
  let name: String
  let initialHtml: String
  let expectedHtml: String
  let splitPath: [Int]
  let splitOffset: Int
  var testDescription: String { name }
}

private let splitCases: [SplitCase] = [
  SplitCase(
    name: "split paragraph in between two text nodes",
    initialHtml: "<p><span>Hello</span><span>world</span></p>",
    expectedHtml:
      #"<p><span style="white-space: pre-wrap;">Hello</span></p><p><span style="white-space: pre-wrap;">world</span></p>"#,
    splitPath: [0], splitOffset: 1),
  SplitCase(
    name: "split paragraph before the first text node",
    initialHtml: "<p><span>Hello</span><span>world</span></p>",
    expectedHtml:
      #"<p><br></p><p><span style="white-space: pre-wrap;">Hello</span><span style="white-space: pre-wrap;">world</span></p>"#,
    splitPath: [0], splitOffset: 0),
  SplitCase(
    name: "split paragraph after the last text node",
    initialHtml: "<p><span>Hello</span><span>world</span></p>",
    expectedHtml:
      #"<p><span style="white-space: pre-wrap;">Hello</span><span style="white-space: pre-wrap;">world</span></p><p><br></p>"#,
    splitPath: [0], splitOffset: 2),
]

@Suite struct LexicalUtilsSplitNodeTests {
  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx::LexicalUtils#splitNode > split paragraph in between two text nodes"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx::LexicalUtils#splitNode > split paragraph before the first text node"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx::LexicalUtils#splitNode > split paragraph after the last text node"
    ),
    arguments: splitCases)
  func split(_ testCase: SplitCase) throws {
    let editor = try createTestEditor(headless: true)
    defer { editor.dispose() }
    try update(editor) {
      try importDocument(editor, testCase.initialHtml)
      var nodeToSplit: ElementNode = try getRoot()
      for index in testCase.splitPath {
        nodeToSplit = try #require(try nodeToSplit.getChildAtIndex(index) as? ElementNode)
      }
      _ = try splitNode(nodeToSplit, testCase.splitOffset)
      #expect(try exportedHtml(editor) == testCase.expectedHtml)
    }
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsSplitNode.test.tsx::LexicalUtils#splitNode > throws when splitting root"
    ))
  func throwsOnRoot() throws {
    let editor = try createTestEditor(headless: true)
    defer { editor.dispose() }
    try update(editor) {
      #expect(throws: (any Error).self) { try splitNode(try getRoot(), 0) }
    }
  }
}

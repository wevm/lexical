import Lexical
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/utils.test.ts.

private func bullets(_ count: Int) throws -> [ListNode] {
  try (0..<count).map { _ in try createListNode("bullet") }
}

private func items(_ count: Int) throws -> [ListItemNode] {
  try (0..<count).map { _ in try createListItemNode() }
}

/// Root > list > item > list > item > list > item: the lists and items.
private func threeLevels(_ root: ElementNode) throws -> ([ListNode], [ListItemNode]) {
  let lists = try bullets(3)
  let items = try items(3)
  try root.append(lists[0])
  try lists[0].append(items[0])
  try items[0].append(lists[1])
  try lists[1].append(items[1])
  try items[1].append(lists[2])
  try lists[2].append(items[2])
  return (lists, items)
}

/// Root > (parent >) list > item, item, list > item: the lists and items.
private func twoLevels(_ parent: ElementNode) throws -> ([ListNode], [ListItemNode]) {
  let lists = try bullets(2)
  let items = try items(3)
  try parent.append(lists[0])
  try lists[0].append(items[0])
  try lists[0].append(items[1])
  try lists[0].append(lists[1])
  try lists[1].append(items[2])
  return (lists, items)
}

@Suite struct ListUtilsTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getListDepth should return the 1-based depth of a list with one levels"
    ))
  func depthOne() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let topListNode = try createListNode("bullet")
      try getRoot().append(topListNode)
      #expect(try getListDepth(topListNode) == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getListDepth should return the 1-based depth of a list with two levels"
    ))
  func depthTwo() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let (lists, _) = try twoLevels(getRoot())
      #expect(try getListDepth(lists[1]) == 2)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getListDepth should return the 1-based depth of a list with five levels"
    ))
  func depthFive() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let lists = try bullets(5)
      let items = try items(4)
      try getRoot().append(lists[0])
      for i in 0..<4 {
        try lists[i].append(items[i])
        try items[i].append(lists[i + 1])
      }
      #expect(try getListDepth(lists[4]) == 5)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getTopListNode should return the top list node when the list is a direct child of the RootNode"
    ))
  func topListAtRoot() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let (lists, items) = try twoLevels(getRoot())
      #expect(try getTopListNode(items[2]).getKey() == lists[0].getKey())
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getTopListNode should return the top list node when the list is not a direct child of the RootNode"
    ))
  func topListInParagraph() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let paragraphNode = try createParagraphNode()
      try getRoot().append(paragraphNode)
      let (lists, items) = try twoLevels(paragraphNode)
      #expect(try getTopListNode(items[2]).getKey() == lists[0].getKey())
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > getTopListNode should return the top list node when the list item is deeply nested."
    ))
  func topListDeeplyNested() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let paragraphNode = try createParagraphNode()
      let lists = try bullets(3)
      let items = try items(4)
      try getRoot().append(paragraphNode)
      try paragraphNode.append(lists[0])
      try lists[0].append(items[0])
      try items[0].append(lists[1])
      try lists[1].append(items[1])
      try items[1].append(lists[2])
      try lists[2].append(items[2])
      try lists[0].append(items[3])
      #expect(try getTopListNode(items[3]).getKey() == lists[0].getKey())
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > isLastItemInList should return true if the listItem is the last in a nested list."
    ))
  func lastInNested() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let (_, items) = try threeLevels(getRoot())
      #expect(try isLastItemInList(items[2]))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > isLastItemInList should return true if the listItem is the last in a non-nested list."
    ))
  func lastInFlat() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let list = try createListNode("bullet")
      let items = try items(2)
      try getRoot().append(list)
      try list.append(contentsOf: items)
      #expect(try isLastItemInList(items[1]))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > isLastItemInList should return false if the listItem is not the last in a nested list."
    ))
  func notLastInNested() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let (_, items) = try threeLevels(getRoot())
      #expect(try !isLastItemInList(items[1]))
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/utils.test.ts::Lexical List Utils tests > isLastItemInList should return true if the listItem is not the last in a non-nested list."
    ))
  func notLastInFlat() throws {
    let editor = try createMountedTestEditor(nodes: listTestNodes)
    try update(editor) {
      let list = try createListNode("bullet")
      let items = try items(2)
      try getRoot().append(list)
      try list.append(contentsOf: items)
      // The upstream title says "true", but it expects false.
      #expect(try !isLastItemInList(items[0]))
    }
  }
}

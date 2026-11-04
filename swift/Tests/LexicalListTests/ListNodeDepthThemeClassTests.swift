import Lexical
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/ListNodeDepthThemeClass.test.ts.
// The DOM element the reconciler reuses is the one createDOM built for the
// list's first commit, updated with updateDOM after the list is nested.

@Suite struct ListNodeDepthThemeClassTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListNodeDepthThemeClass.test.ts::ListNode depth theme classes > the previous depth class is removed when a list changes depth"
    ))
  func depthClassReplaced() throws {
    let theme: EditorThemeClasses = [
      "list": ["ul": "my-ul-list-class", "ulDepth": ["depth-1", "depth-2", "depth-3"]]
    ]
    let editor = try createMountedTestEditor(nodes: listTestNodes, theme: theme)
    var listKey = ""
    try update(editor) {
      let list = try createListNode("bullet")
      try list.append(createListItemNode().append(createTextNode("a")))
      try getRoot().append(list)
      listKey = list.getKey()
    }
    let (prev, dom) = try editor.read(.latest) { () throws -> (LexicalNode, DOMElement) in
      let list = try getNodeByKeyOrThrow(listKey)
      return (list, try (list as! ListNode).createDOM(editor._config))
    }
    #expect(dom.className == "my-ul-list-class depth-1")
    try update(editor) {
      let list = try #require(try getRoot().getFirstChild() as? ListNode)
      let outer = try createListNode("bullet")
      let listItem = try createListItemNode()
      try outer.append(listItem)
      try getRoot().append(outer)
      try listItem.append(list)
    }
    try editor.read(.latest) { () throws -> Void in
      let list = try #require(try getNodeByKeyOrThrow(listKey) as? ListNode)
      #expect(try list.updateDOM(prev, dom, editor._config) == false)
    }
    #expect(dom.className == "my-ul-list-class depth-2")
  }
}

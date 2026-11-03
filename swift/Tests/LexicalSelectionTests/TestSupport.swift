import Lexical
import LexicalSelection

/// `TestShadowRootNode` from packages/lexical/src/__tests__/utils.
final class TestShadowRootNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(TestShadowRootNode.self, type: "test_shadow_root", extends: ElementNode.self)
  }
  override func isShadowRoot() -> Bool { true }
}

func createTestShadowRootNode() throws -> TestShadowRootNode { try TestShadowRootNode() }

/// A root element that reports a left-to-right computed style for each
/// node the reconciler has rendered, as `getComputedStyle` does for a
/// node's DOM in JS.
final class StyledRootElement: ComputedStyleProvider {
  weak var editor: Editor?
  var style = ComputedStyle()
  func computedStyle(forKey key: NodeKey) -> ComputedStyle? {
    editor?._keyToDOMMap.has(key) == true ? style : nil
  }
}

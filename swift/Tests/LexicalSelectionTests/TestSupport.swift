import Lexical
import LexicalSelection
import LexicalTesting

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

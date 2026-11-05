import Lexical

// Port of packages/lexical-text/src/findTextIntersectionFromCharacters.ts.

/// `$findTextIntersectionFromCharacters(root, targetCharacters)`: the text
/// node, in document order, that holds the character at `targetCharacters`
/// (in UTF-16 code units of the text nodes only), and the offset in it.
public func findTextIntersectionFromCharacters(
  _ root: RootNode, _ targetCharacters: Int
) throws -> TextNodeWithOffset? {
  var node = try root.getFirstChild()
  var currentCharacters = 0
  mainLoop: while let current = node {
    if let element = current as? ElementNode {
      if let child = try element.getFirstChild() {
        node = child
        continue
      }
    } else if let text = current as? TextNode {
      let characters = try text.getTextContentSize()
      if currentCharacters + characters > targetCharacters {
        return TextNodeWithOffset(node: text, offset: targetCharacters - currentCharacters)
      }
      currentCharacters += characters
    }
    if let sibling = try current.getNextSibling() {
      node = sibling
      continue
    }
    var parent = try current.getParent()
    while let p = parent {
      if let parentSibling = try p.getNextSibling() {
        node = parentSibling
        continue mainLoop
      }
      parent = try p.getParent()
    }
    break
  }
  return nil
}

import Lexical

/// The computed style properties `@lexical/selection` reads:
/// `CSSStyleDeclaration.direction` and `writingMode`.
public struct ComputedStyle: Equatable, Sendable {
  public var direction: String
  public var writingMode: String
  public init(direction: String = "ltr", writingMode: String = "horizontal-tb") {
    self.direction = direction
    self.writingMode = writingMode
  }
}

/// A root element that can report the computed style of a node's rendered
/// element, as `getComputedStyle(editor.getElementByKey(key))` does in JS.
/// The views conform; without one there is no computed style, as JS has
/// none for a node that is not rendered.
public protocol ComputedStyleProvider: AnyObject {
  func computedStyle(forKey key: NodeKey) -> ComputedStyle?
}

/// `$getComputedStyleForElement`.
public func getComputedStyleForElement(_ element: ElementNode) throws -> ComputedStyle? {
  guard let provider = try getEditor().getRootElement() as? ComputedStyleProvider else {
    return nil
  }
  return provider.computedStyle(forKey: element.getKey())
}

/// `$getComputedStyleForParent`: the parent's computed style; the root's
/// own for the root. A slot value or detached node has none.
public func getComputedStyleForParent(_ node: LexicalNode) throws -> ComputedStyle? {
  let parent: ElementNode? = try (node as? RootNode) ?? node.getParent()
  return try parent.flatMap(getComputedStyleForElement)
}

/// `$isParentRTL`.
public func isParentRTL(_ node: LexicalNode) throws -> Bool {
  try getComputedStyleForParent(node)?.direction == "rtl"
}

/// `getCSSFromStyleObject`.
public func getCSSFromStyleObject(_ styles: StyleObject) -> String {
  var css = ""
  for (style, value) in styles where !style.isEmpty {
    css += "\(style): \(value);"
  }
  return css
}

/// `getStyleObjectFromRawCSS`, an alias of `getStyleObjectFromCSS`.
public func getStyleObjectFromRawCSS(_ css: String) -> StyleObject { getStyleObjectFromCSS(css) }

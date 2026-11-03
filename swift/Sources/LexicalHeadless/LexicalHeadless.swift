import Lexical

// Port of packages/lexical-headless/src/index.ts and HeadlessExtension.ts.
// The Swift DOM needs no host window, so `@lexical/headless/dom`'s `withDOM`
// has no counterpart.

/// `createHeadlessEditor(editorConfig)`: an editor for use without a view,
/// such as on a server. It refuses a root element. JS also throws from the
/// other DOM-only methods; in Swift those can't throw, and on a headless
/// editor they behave as on one with no root (DESIGN.md, Headless).
public func createHeadlessEditor(
  _ editorConfig: CreateEditorArgs = CreateEditorArgs()
) throws
  -> Editor
{
  let editor = try createEditor(editorConfig)
  editor._headless = true
  editor._setRootElementGuard = { _ in
    throw LexicalError("setRootElement is not supported in headless mode")
  }
  return editor
}

/// `HeadlessExtension`: marks the editor headless and refuses a root
/// element, though setting it to `nil` is allowed.
public let HeadlessExtension = defineExtension(
  name: "@lexical/headless/Headless", config: (),
  build: { editor, _, _ in
    editor._headless = true
    editor._setRootElementGuard = { nextRootElement in
      try invariant(
        nextRootElement == nil,
        "HeadlessExtension: setRootElement is not supported in headless mode")
    }
  })

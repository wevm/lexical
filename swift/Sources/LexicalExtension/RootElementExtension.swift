import Lexical

// Port of packages/lexical-extension/src/RootElementExtension.ts.

/// `RootElementExtension`: its output is a signal of the editor's current
/// root element, mirroring `editor.getRootElement()` through a root
/// listener while something watches it.
public let RootElementExtension = defineExtension(
  name: "@lexical/extension/RootElement",
  build: { editor, _, _ -> Signal<AnyObject?> in
    watchedSignal(
      { editor.getRootElement() },
      { rootElementSignal in
        editor.registerRootListener { rootElement, _ in
          rootElementSignal.value = rootElement
          return nil
        }
      })
  })

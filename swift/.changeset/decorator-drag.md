---
'lexical-swift': patch
---

A selected decorator can be dragged to move it, as a selected image's `draggable` element is on the web. On macOS a drag of its view and on iOS a long press and drag dispatch `DRAGSTART_COMMAND` from the decorator, and the drop dispatches `DROP_COMMAND` where it lands, so the handlers an app registers (an image extension's) place it. `LexicalNSTextView` takes a drop of a Lexical drag payload from itself as a move, which `NSTextView` refused. `View.lexicalDecoratorDragDisabled(_:)` keeps a decorator's view from being dragged, for example while it resizes.

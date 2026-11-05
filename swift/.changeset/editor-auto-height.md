---
'lexical-swift': patch
---

The SwiftUI `LexicalEditor` can size itself to its content with `.sizedToContent(minHeight:maxHeight:)`: it grows with its text up to `maxHeight` and scrolls past it, as a chat composer does. Without the modifier it still fills the height it's offered. `LexicalNSTextView` and `LexicalUITextView` take the same range as `heightRange`, which sets their intrinsic height to their `contentHeight` clamped to that range. On iOS, an empty last line now has the last block's height and look, as on macOS, so an empty editor and its caret are as tall as a line of text.

---
'lexical-swift': patch
---

On iOS 26, `LexicalUITextView` shows the selection handles again when a text selection follows the selection of an inline decorator, such as a mention or emoji chip. UIKit sometimes left them hidden.

---
'lexical-swift': patch
---

On iOS, a selected decorator (a node selection, such as a tapped image or horizontal rule) shows only its own outline, as on the web: `LexicalUITextView` turns UIKit's selection display (caret, highlight and handles) off while the selection is a node selection, and on again for text.

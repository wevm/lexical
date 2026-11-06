---
'lexical-swift': patch
---

[fix] `LexicalNSTextView` and `LexicalUITextView` paint `TextKitInputController.highlights` (comment marks, find matches) over each line's text box, the font's content area, as the selection is drawn and as an inline background is on the web, instead of over the whole line box; and the selection beside an inline decorator taller than the line is on the text, which is on the decorator's baseline, instead of in the middle of the line.

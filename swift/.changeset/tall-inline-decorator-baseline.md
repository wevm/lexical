---
'lexical-swift': patch
---

[fix] A line with an inline decorator taller than the line height, such as an inline image, draws its text on the decorator's baseline, as CSS puts an inline image's line's text, in `LexicalNSTextView` and `LexicalUITextView`. Its text was drawn centered in the tall line instead, overlapping the next line's text, which looked like that line twice; a list item's marker follows the text.

Verify: an inline image in a paragraph or list item, with text before and after it and text that wraps below it.

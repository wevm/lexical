---
'lexical-swift': patch
---

Fixed `RichTextExtension` and `PlainTextExtension` to depend on `NormalizeTripleClickSelectionExtension`, as JS's do, so an editor built from either one trimmed a triple click's block selection back into the clicked block without listing that extension itself.

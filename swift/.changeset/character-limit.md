---
'lexical-swift': patch
---

Add `registerCharacterLimit(_:maxCharacters:strlen:remainingCharacters:)` to `LexicalOverflow`, a port of `@lexical/react`'s `useCharacterLimit`: it wraps the text past the limit in `OverflowNode`s and reports the characters left. The TextKit views lay out an element inline when its `createDOM` makes an inline-level HTML element, as a browser does, so the text past the limit stays on its line, with the theme's `characterLimit` background.

Verify: an element node that is a block in the model but makes a `<span>`, `<a>` or another inline-level element in `createDOM` now shows inline in the views.

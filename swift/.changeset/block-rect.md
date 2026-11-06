---
'lexical-swift': patch
---

`LexicalNSTextView` and `LexicalUITextView` gain `blockRect(for:)`: the rect of a rendered block (a code block's box, padding included), as JS reads the block element's `getBoundingClientRect()`, for floating UI anchored at a block.

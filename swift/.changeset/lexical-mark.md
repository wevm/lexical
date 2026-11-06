---
'lexical-swift': patch
---

Add `LexicalMark`, a port of `@lexical/mark` in its own library product: `MarkNode`, `wrapSelectionInMarkNode`, `unwrapMarkNode`, `getMarkIDs`, `MarkExtension` and the mark state extension. The TextKit views draw marks with the theme's `mark` and `markOverlap` backgrounds. `TextKitInputController.highlights` paints named model ranges behind the text without changing it, as a page's CSS custom highlights do, for things such as a selected comment's marks or search matches.

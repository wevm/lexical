---
'lexical-swift': patch
---

`LexicalMenu`'s `MenuState` gains `onCloseAsync`, upstream's `onClose` that returns a promise: a closed menu keeps its resolution and its keys until it returns or throws, then clears, unless a newer query reopened the menu meanwhile, which it keeps.

---
'lexical-swift': patch
---

Fixed the reconciler of a mounted editor not calling `updateDOM`, so a node whose `updateDOM` returns true (a heading whose tag changed, a list whose type changed) was not created again as in JS. Now its children are reported `created` to mutation listeners, its decorators decorate again, and it keeps its `textFormat` and `textStyle` instead of taking them from its first text. A node class that leaves `createDOM` or `updateDOM` to the base method keeps its rendering, as before.

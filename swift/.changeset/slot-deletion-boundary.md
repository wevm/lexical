---
'lexical-swift': patch
---

Fixed word, line and character deletion across a slot boundary: when an editor's `SelectionMovement` landed in another slot frame, `deleteWord`, `deleteLine` and `deleteCharacter` threw (`$comparePointCaretNext: … do not have a common ancestor`). They now stop at the boundary in the caret's frame, as JS's `'deletion'` mode of `$setPointAtSlotBoundary` does: beside a host, the deletion stops before the host, and the focus descends toward the caret.

---
'lexical-ui': patch
---

Fixed word and line deletion crossing a slot boundary: when an editor's `SelectionMovement` lands in another slot frame, `delete_word`, `delete_line` and `delete_character` now stop at the boundary in the caret's frame, as JS's `'deletion'` mode of `$setPointAtSlotBoundary` does, instead of deleting into the other frame.

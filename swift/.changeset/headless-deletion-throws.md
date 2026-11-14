---
'lexical-swift': minor
---

On an editor with no root element and no `selectionMovement` (a headless editor), `RangeSelection.deleteCharacter`, `deleteWord`, `deleteLine` and `modify` throw `LexicalError("window object not found")` when the move needs text layout, as headless JS does, instead of doing nothing. Moves that the model resolves (across a decorator or a block boundary, or deleting an expanded selection) work as before. To delete within text headless, set `editor.selectionMovement`, or extend the selection in the model and call `removeText()`.

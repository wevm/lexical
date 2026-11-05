---
'lexical-swift': patch
---

On macOS, the selection reaches the editor at each step of a click or a drag, as a browser's `selectionchange` does, rather than once the mouse button is released: update listeners (and a toolbar that shows the block type) and the caret's size follow the selection from the press on. Selection-only updates still add no history.

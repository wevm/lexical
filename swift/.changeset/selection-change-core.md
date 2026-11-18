---
'lexical-swift': patch
---

Fixed the text views' selection changes to run JS `onSelectionChange`: `SELECTION_CHANGE_COMMAND` was dispatched for every selection the view reported, even one that had not changed, and is now dispatched only for a change, and a caret placed at an element point now keeps the previous selection's format instead of clearing it.

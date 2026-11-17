---
'lexical-ui': patch
---

Added `on_selection_change`, the model half of JS's `onSelectionChange`: an app passes the points its view selected, and the editor selects them, gives a caret the format and style of the text it is in (and a range the format all its text shares), and dispatches `SELECTION_CHANGE_COMMAND`.

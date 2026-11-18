---
'lexical-ui': patch
---

Added `watch_editable_extension()`, the port of `WatchEditableExtension`: its output is a signal of `Editor::is_editable` that follows `Editor::set_editable` while something watches it.

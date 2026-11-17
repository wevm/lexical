---
'lexical-ui': patch
---

Added `code::register_code_gutter`, the port of `registerCodeGutter`: it gives the app each code block's line numbers (what JS writes to `data-gutter`) when the block is rendered and whenever its line count changes.

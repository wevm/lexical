---
'lexical-swift': patch
---

Code blocks look like the web playground's: the default theme's `code` style draws a background, padding and a line-number gutter with a right border, in light and dark mode, and sets a tab size of 2. `TextKitStyle` gains `paddingTop`, `paddingBottom`, `gutter` (`TextKitGutter`) and `tabSize` for themes that want the same for their own blocks.

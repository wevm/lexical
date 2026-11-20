---
'lexical-swift': patch
---

Added `TextKitStyle.border` (`TextKitBorder`: a width, a color and a corner radius), which `LexicalNSTextView` and `LexicalUITextView` draw around a block's box with its fill and gutter clipped to the rounded corners, and `TextKitStyle.checkbox` (`TextKitCheckbox`: size, fill, border and check mark color), which they read from the theme's `listitemChecked` and `listitemUnchecked` styles to draw check list boxes. `TextKitStyle(css:)` reads `border`, `border-width`, `border-style`, `border-color` and `border-radius`. Themes without them draw as before, except that the default unchecked box's 1-point border now has the 2-point outer radius the playground's CSS gives it.

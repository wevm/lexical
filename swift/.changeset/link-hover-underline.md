---
'lexical-swift': patch
---

On macOS, a link is underlined while the pointer is over it, as the web playground's `__link:hover { text-decoration: underline }` does, and only that link. It is view state: no editor update or history entry. `TextKitStyle` gains `hoverUnderline`, which the default theme's `link` style sets.

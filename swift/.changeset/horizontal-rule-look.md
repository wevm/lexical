---
'lexical-swift': patch
---

Horizontal rules show on macOS and iOS: they rendered nothing, because `HorizontalRuleNode` has no view of its own. The views now draw one from the theme's new `horizontalRule` (`TextKitRule`), which the default theme sets to the web playground's 2-point #ccc line with 1em margins, outlined in blue while the rule is selected, without the selection highlight or iOS's handles.

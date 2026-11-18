---
'lexical-swift': patch
---

Added `TextKitStyle.tracking`, the points added after each character (SwiftUI's `tracking(_:)`), which text and the placeholder inherit like CSS `letter-spacing` and which the views draw as the `.tracking` attribute. `TextKitStyle(css:)` reads `letter-spacing` in `px` or `pt`, or `normal`.

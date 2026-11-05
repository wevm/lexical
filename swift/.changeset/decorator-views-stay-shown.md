---
'lexical-swift': patch
---

Decorator views in the TextKit views (images, horizontal rules, inline decorators such as the playground's date pill) no longer stop drawing after an edit lays their line out again. TextKit makes a new view provider for the line and releases the old one, which took the decorator's view out of the text after the new provider had shown it; each provider now shows the view in its own container. Fixed on macOS and iOS.

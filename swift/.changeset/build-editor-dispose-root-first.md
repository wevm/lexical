---
'lexical-swift': patch
---

Fixed `buildEditor`'s `dispose()` to remove the root element before it unregistered the extensions, as JS's does, so the extensions' root and update listeners heard the root element go.

---
'lexical-swift': patch
---

A key pressed with Command reports the `key` Safari does: Option applies and Shift doesn't (⇧⌘7 is `7`, ⌥⌘1 is `¡`, ⇧⌘. is `.`), on macOS and with a hardware keyboard on iOS. Shortcuts bound to Shift+Command and a digit or symbol, such as ⇧⌘7 for a numbered list, now match. With Control alone, Shift still applies, as before.

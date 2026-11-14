---
'lexical-swift': minor
---

`lexical-swift` requires Swift 6.4 (Xcode 27) again, as it did before 0.1.9. Xcode 27 runs on macOS Tahoe 26.6 or later, so an app that builds with Xcode 26.5 or 26.6 moves to Xcode 27, or stays on `lexical-swift` 0.1.10 with `.upToNextMinor(from: "0.1.10")`. It still accepts swift-syntax 603 as well as 604.

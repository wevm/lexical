---
'lexical-swift': patch
---

Lowered the minimum toolchain from Swift 6.4 (Xcode 27) to Swift 6.3 (Xcode 26.5), so apps that still build with Xcode 26.5 can now depend on `lexical-swift`. The package now accepts swift-syntax 603 as well as 604, so it no longer forces an app onto swift-syntax 604.

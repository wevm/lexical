---
'lexical-swift': patch
---

`lexical-swift` builds with Swift 6.3 (Xcode 26.5) and later, not only Swift 6.4 (Xcode 27), so apps that still build with Xcode 26.5 can depend on it. It accepts swift-syntax 603 as well as 604, so it no longer forces an app onto swift-syntax 604.

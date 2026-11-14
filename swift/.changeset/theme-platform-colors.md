---
'lexical-swift': minor
---

Themes take an app's own colors and change on a live view. `TextKitColor(uiColor:)` and `TextKitColor(nsColor:)` make a theme color from a `UIColor` or `NSColor`, keeping a dynamic color's light and dark variants (a high-contrast variant draws as its regular one), so `TextKitColor(uiColor: .label)` or a SwiftUI `Color` through `UIColor(_:)` follows the appearance. `LexicalUITextView` and `LexicalNSTextView` have a `theme` property that draws the text again when set to a different theme, as `LexicalEditor.theme(_:)` already did, which now sets it. To keep a theme's sizes fixed on iOS instead of scaling with Dynamic Type, pin the size category with `.dynamicTypeSize(.large)` on `LexicalEditor` or `traitOverrides.preferredContentSizeCategory = .large` on the view.

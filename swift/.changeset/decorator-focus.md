---
'lexical-swift': patch
---

A text field in a decorator's SwiftUI view keeps editing when TextKit lays the decorator's line out again, in the AppKit and UIKit views. The view moved through a container outside the window, which ended the editing; on iOS the keyboard change that followed resized the text view during its own layout, and TextKit 2 raised (`textViewportElementsInRect: cannot be called during layout`).

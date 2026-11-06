---
'lexical-swift': patch
---

[fix] `LexicalUITextView` scales the theme's lengths with Dynamic Type as it scales its fonts: indents, paragraph spacing, list markers and check boxes, quote bars, code blocks' padding and gutter, horizontal rules and baseline shifts grow by body text's factor for the content size category, so a large-text document keeps its proportions. At the accessibility sizes a numbered list's markers are no longer cut off at the view's edge and a check box is as tall as its text.

A block decorator's view is measured at the line's width on iOS, so text in it wraps to the line instead of being cut off where it is taller than it would be on one line.

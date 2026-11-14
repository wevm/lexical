---
'lexical-swift': patch
---

Fixed the order of a `StyleObject`'s properties, and so of `getStyleObjectFromCSS`'s, which now follows a JS object's key order: properties named by an array index (such as `2`) come first, in ascending order, then the others in the order they were first set. `getCSSFromStyleObject` and `patchStyleText` write CSS in that order, as Lexical JS does; before, Swift kept every property in the order it was first set.

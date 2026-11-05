---
'lexical-swift': patch
---

`LexicalMenu` ports the platform-neutral half of `@lexical/react`'s typeahead and node menus: `basicTypeaheadTriggerMatch` (`useBasicTypeaheadTriggerMatch`), `MenuOption`, `registerTypeaheadMenu` and `NodeMenu`, which find the query before the caret, keep the options and the highlighted one, and handle the arrow keys, Enter, Tab and Escape at the menu's priority while it is open. The app draws the menu. `LexicalNSTextView` and `LexicalUITextView` gain `rect(from:to:)` and `screenRect(from:to:)`, the rect of a model range, to position floating UI at. On iOS the software keyboard's Return is a `keydown` of Enter, as in Safari, so an open menu takes it.

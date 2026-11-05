---
'lexical-swift': patch
---

`LexicalMenu`: `MenuState.drawsEmptyMenu` tells the menu that the app's view draws it even with no options, as a JS `menuRenderFn` can. Such a menu keeps the arrow, Enter, Tab and Escape keys instead of letting them through to the editor, and an arrow key leaves nothing highlighted, as upstream does.

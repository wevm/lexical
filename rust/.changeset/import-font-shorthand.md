---
'lexical-ui': patch
---

Fixed HTML import ignoring the `font` shorthand: the inline style declaration now expands `font` into its longhands as jsdom's does, so `<span style="font: bold 12px serif">` and a `<style>` rule's `font: italic 10px a` import as bold and italic. Added pseudo-class selectors to `DomNode::query_selector_all` and `DomNode::matches` (`:first-child` and the other `:nth-` positions, `:not()`, `:is()`, `:where()`, `:has()`, `:link`, `:lang()`, `:empty`, `:root`, `:checked`, `:disabled`, `:enabled`), so `<style>` rules that use them, such as Word's `a:link` lists, now apply on paste; user-action pseudo-classes such as `:hover` and pseudo-elements match nothing.

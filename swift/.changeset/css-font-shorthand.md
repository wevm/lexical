---
'lexical-swift': patch
---

Fixed HTML import of formatting that comes through CSS, as in JS: an inline or style sheet `font` shorthand (`font: bold 12px serif`) now expands into its longhands, so it imports as bold or italic, and `<style>` rules whose selectors use pseudo-classes (`a:link`, `:first-child`, `:nth-child()`, `:not()`, `:is()`, `:has()`...) now apply. `CSSStyleDeclaration` keeps an earlier `!important` declaration over a later plain one, and `calc()` of a sum or difference in one unit is evaluated.

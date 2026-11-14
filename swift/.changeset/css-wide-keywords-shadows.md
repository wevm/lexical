---
'lexical-swift': patch
---

Fixed the inline style declaration keeping the case of a CSS-wide keyword (`INHERIT`, `Unset`) for font names and shadows, and keeping a `box-shadow` or `text-shadow` of nothing but identifiers (`Foo`, `red, blue`) as written. It now lower-cases those keywords for every property and rejects such shadows, as jsdom does.

---
'lexical-swift': patch
---

The HTML serializer (`generateHtmlFromNodes`, `outerHTML` and `innerHTML` of the port's DOM) writes `<` and `>` in an attribute value as `&lt;` and `&gt;`, as Chromium, WebKit and the current HTML standard do, so exported HTML matches the browser's byte for byte where an attribute holds them. Text and the other escapes are unchanged.

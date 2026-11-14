---
'lexical-swift': patch
---

[fix] `autoLinkUrlMatcher`, `autoLinkEmailMatcher` and `registerAutoLink` match JS for white space, line terminators and characters outside the BMP. U+FEFF counts as white space and U+0085 does not, in the email pattern, the default `separatorRegex` (`/[.,;\s]/`) and the check for where a link's text nodes end, so a URL after U+FEFF is linked and an email around U+0085 is linked whole. A quoted local part may hold U+000B, U+000C and U+0085, as in JS. `autoLinkEmailMatcher` counts its length limits in UTF-16 code units, as JS does for a regular expression without the `u` flag, so a local part longer than 64 code units, such as 33 emoji, is no longer linked whole.

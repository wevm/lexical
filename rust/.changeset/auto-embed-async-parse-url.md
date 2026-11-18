---
'lexical-ui': patch
---

Added `EmbedConfig::parse_url_async` and `ParseUrlDone`, so an embed config can answer `parseUrl` later, as a JS `parseUrl` that returns a promise does: the auto-embed menu opens when the config resolves, unless a newer paste was checked meanwhile or the link was removed or given another URL, pending edits included. `EmbedConfig::parse_url` now has a default (`None`), so a config implements one of the two; existing configs are unchanged.

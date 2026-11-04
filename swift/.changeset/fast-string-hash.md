---
'lexical-swift': patch
---

`JSMap`, `JSSet` and `JSONObject` hash string keys with a fast hash of their UTF-8 bytes instead of SipHash, which made node-map lookups a large share of every update and load. String keys now compare by code units, as a JS `Map` or object does: two keys that are canonically equivalent but encoded differently (`"\u{E9}"` and `"e\u{301}"`) are distinct, where they used to collapse into one.

//! Ports of `packages/lexical/src/__tests__/unit/LexicalBulkSplice.test.ts`.
//!
//! Its one test observes the tree from `ObservedTextNode.afterCloneFrom`
//! during a bulk splice; a Rust node has no `afterCloneFrom` (its `Clone`
//! runs while the node map is borrowed), so it is skipped in
//! `rust/parity/skips.json`.

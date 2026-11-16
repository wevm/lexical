//! Ports of `packages/lexical/src/__tests__/unit/FastPathCrossParent.test.ts`.
//!
//! Its one test compares the reconciler's children fast path with its
//! general walk; Rust's reconciler has only the general walk
//! (`reconciler.rs`), so it is skipped in `rust/parity/skips.json`.

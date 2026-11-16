//! Ports of `packages/lexical-history`'s unit tests.

#[cfg(feature = "extension")]
mod history_snapshot_cut_paste;
#[cfg(feature = "extension")]
// Named after LexicalHistory.test.tsx, as every module here is after its
// upstream file.
#[allow(clippy::module_inception)]
mod lexical_history;
#[cfg(feature = "extension")]
mod lexical_history_push_tag;
#[cfg(feature = "extension")]
mod shared_history_max_depth;

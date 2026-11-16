//! Ports of `packages/lexical-a11y`'s unit tests. The focus trap, roving
//! tabindex and focus manager tests are skipped as DOM-only, as Swift's are.

mod aria_live_region_extension;
mod editor_mode_announce_extension;
mod history_announce_extension;
pub(super) mod support;

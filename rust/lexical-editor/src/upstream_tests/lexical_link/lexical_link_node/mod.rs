//! Ports of `packages/lexical-link/src/__tests__/unit/LexicalLinkNode.test.ts`,
//! one module per top-level `describe`, less the three `updateDOM` tests
//! that check the DOM writes of `updateDOM` (rust/parity/skips.json).

mod extract_with_child;
#[cfg(feature = "extension")]
mod link_boundary;
mod node;
#[cfg(all(feature = "extension", feature = "rich-text"))]
mod node_selection;
#[cfg(all(feature = "extension", feature = "rich-text"))]
mod transform;

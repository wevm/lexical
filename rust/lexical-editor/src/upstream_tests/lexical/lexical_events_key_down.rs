//! Ports of `packages/lexical/src/__tests__/unit/LexicalEventsKeyDown.test.ts`
//! (`runKeyDownDispatchParityTests(false)` of `keyDownDispatchParity.ts`,
//! which lives in [`super::lexical_events_key_down_apple`]).

#[cfg(feature = "extension")]
use super::lexical_events_key_down_apple::run_key_down_dispatch_parity_test;

// upstream: lexical/src/__tests__/unit/LexicalEventsKeyDown.test.ts::$handleKeyDown dispatch parity (IS_APPLE=false) > dispatches the same commands as the legacy predicate chain
#[cfg(feature = "extension")]
#[test]
fn dispatches_the_same_commands_as_the_legacy_predicate_chain() {
    run_key_down_dispatch_parity_test(false);
}

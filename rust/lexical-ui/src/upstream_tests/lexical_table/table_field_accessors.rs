//! Ports of `packages/lexical-table/src/__tests__/unit/TableFieldAccessors.test.ts`.
//!
//! None: each test subclasses `TableNode` or `TableCellNode` and relies on
//! `resolveSchemaField` finding the subclass's setter, which Rust has no
//! counterpart for (`parity/skips.json`).

//! `extension-core/safeCast.ts`.

/// `safeCast(value)`: `value`. JS uses it to give a config its type where
/// inference wouldn't; Rust infers it from the annotation, so this only
/// keeps ported code close to JS.
pub fn safe_cast<T>(value: T) -> T {
    value
}

//! `environment.ts`: what Lexical knows of the platform it runs on. Only
//! `IS_APPLE` is here, for the keyboard conventions; the browser flags
//! (`IS_SAFARI`, `IS_IOS`...) have no counterpart without a browser.

use std::cell::Cell;

thread_local! {
    static IS_APPLE_OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// `IS_APPLE`: whether the keyboard conventions are Apple's (⌘ for the
/// primary shortcuts, ⌥ for words). JS reads the browser's platform; here
/// it is `true` on Apple platforms. [`with_is_apple`] overrides it on this
/// thread, as Swift's `withIsApple` does, since the vectors are recorded
/// for both.
pub fn is_apple() -> bool {
    IS_APPLE_OVERRIDE
        .with(Cell::get)
        .unwrap_or(cfg!(target_vendor = "apple"))
}

/// Runs `f` with [`is_apple`] returning `is_apple` on this thread.
pub fn with_is_apple<T>(is_apple: bool, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<bool>);
    impl Drop for Restore {
        fn drop(&mut self) {
            IS_APPLE_OVERRIDE.with(|c| c.set(self.0));
        }
    }
    let _restore = Restore(IS_APPLE_OVERRIDE.with(|c| c.replace(Some(is_apple))));
    f()
}

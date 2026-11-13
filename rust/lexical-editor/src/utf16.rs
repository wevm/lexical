//! Text offsets in UTF-16 code units, as JS strings count them, over Rust's
//! UTF-8 strings. A slice that splits a surrogate pair gives U+FFFD for the
//! half it keeps, the Tier 1 exception for unpaired surrogates
//! (rust/PARITY.md); it is still one unit long, so lengths and offsets agree
//! with JS.

/// `text.length`.
pub fn len(text: &str) -> usize {
    if text.is_ascii() {
        text.len()
    } else {
        text.chars().map(char::len_utf16).sum()
    }
}

/// `text.slice(start, end)` for `0 <= start <= end`, clamped to the length.
pub fn slice(text: &str, start: usize, end: usize) -> String {
    if text.is_ascii() {
        let end = end.min(text.len());
        let start = start.min(end);
        return text[start..end].to_owned();
    }
    let units: Vec<u16> = text.encode_utf16().collect();
    let end = end.min(units.len());
    let start = start.min(end);
    String::from_utf16_lossy(&units[start..end])
}

/// `text.slice(start)`.
pub fn slice_from(text: &str, start: usize) -> String {
    slice(text, start, usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_slices_like_js() {
        // Each `.length` and `.slice(...)` in Node.
        let text = "aé😀b";
        assert_eq!(len(text), 5);
        assert_eq!(slice(text, 1, 4), "é😀");
        assert_eq!(slice_from(text, 4), "b");
        assert_eq!(slice(text, 9, 12), "");
        // Splitting the pair keeps one unit each side, as U+FFFD.
        assert_eq!(slice(text, 0, 3), "aé\u{FFFD}");
        assert_eq!(len(&slice(text, 0, 3)), 3);
    }
}

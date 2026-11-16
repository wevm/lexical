//! `LexicalConstants.ts`: the flags and the tables between their serialized
//! names and stored values. The DOM and platform constants come with the
//! packages that use them.

// Text node modes.
pub const IS_NORMAL: u8 = 0;
pub const IS_TOKEN: u8 = 1;
pub const IS_SEGMENTED: u8 = 2;

// Text node formatting.
pub const IS_BOLD: i32 = 1;
pub const IS_ITALIC: i32 = 1 << 1;
pub const IS_STRIKETHROUGH: i32 = 1 << 2;
pub const IS_UNDERLINE: i32 = 1 << 3;
pub const IS_CODE: i32 = 1 << 4;
pub const IS_SUBSCRIPT: i32 = 1 << 5;
pub const IS_SUPERSCRIPT: i32 = 1 << 6;
pub const IS_HIGHLIGHT: i32 = 1 << 7;
pub const IS_LOWERCASE: i32 = 1 << 8;
pub const IS_UPPERCASE: i32 = 1 << 9;
pub const IS_CAPITALIZE: i32 = 1 << 10;

pub const IS_ALL_FORMATTING: i32 = IS_BOLD
    | IS_ITALIC
    | IS_STRIKETHROUGH
    | IS_UNDERLINE
    | IS_CODE
    | IS_SUBSCRIPT
    | IS_SUPERSCRIPT
    | IS_HIGHLIGHT
    | IS_LOWERCASE
    | IS_UPPERCASE
    | IS_CAPITALIZE;

// Text node details.
pub const IS_DIRECTIONLESS: i32 = 1;
pub const IS_UNMERGEABLE: i32 = 1 << 1;

// Element node formatting.
pub const IS_ALIGN_LEFT: u8 = 1;
pub const IS_ALIGN_CENTER: u8 = 2;
pub const IS_ALIGN_RIGHT: u8 = 3;
pub const IS_ALIGN_JUSTIFY: u8 = 4;
pub const IS_ALIGN_START: u8 = 5;
pub const IS_ALIGN_END: u8 = 6;

pub const DOUBLE_LINE_BREAK: &str = "\n\n";

/// `TEXT_TYPE_TO_FORMAT`.
pub const TEXT_TYPE_TO_FORMAT: &[(&str, i32)] = &[
    ("bold", IS_BOLD),
    ("capitalize", IS_CAPITALIZE),
    ("code", IS_CODE),
    ("highlight", IS_HIGHLIGHT),
    ("italic", IS_ITALIC),
    ("lowercase", IS_LOWERCASE),
    ("strikethrough", IS_STRIKETHROUGH),
    ("subscript", IS_SUBSCRIPT),
    ("superscript", IS_SUPERSCRIPT),
    ("underline", IS_UNDERLINE),
    ("uppercase", IS_UPPERCASE),
];

/// `DETAIL_TYPE_TO_DETAIL`.
pub const DETAIL_TYPE_TO_DETAIL: &[(&str, i32)] = &[
    ("directionless", IS_DIRECTIONLESS),
    ("unmergeable", IS_UNMERGEABLE),
];

/// `ELEMENT_TYPE_TO_FORMAT`, with the empty format ElementNode's schema adds.
pub const ELEMENT_TYPE_TO_FORMAT: &[(&str, u8)] = &[
    ("", 0),
    ("center", IS_ALIGN_CENTER),
    ("end", IS_ALIGN_END),
    ("justify", IS_ALIGN_JUSTIFY),
    ("left", IS_ALIGN_LEFT),
    ("right", IS_ALIGN_RIGHT),
    ("start", IS_ALIGN_START),
];

/// `TEXT_MODE_TO_TYPE`.
pub const TEXT_MODE_TO_TYPE: &[(&str, u8)] = &[
    ("normal", IS_NORMAL),
    ("segmented", IS_SEGMENTED),
    ("token", IS_TOKEN),
];

/// The key a node's flat NodeState is serialized under.
pub const NODE_STATE_KEY: &str = "$";

/// The value a table maps `name` to: `name in TABLE ? TABLE[name] : ...`.
pub fn lookup<T: Copy>(table: &[(&str, T)], name: &str) -> Option<T> {
    table.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

/// The name a table maps to `value`: the inverted table.
pub fn name_of<T: Copy + PartialEq>(table: &[(&'static str, T)], value: T) -> Option<&'static str> {
    table.iter().find(|(_, v)| *v == value).map(|(n, _)| *n)
}

/// `RTL`: the UTF-16 code unit ranges `RTL_REGEX` and `LTR_REGEX` take as
/// strongly right-to-left.
pub const RTL: &[(u16, u16)] = &[(0x0591, 0x07FF), (0xFB1D, 0xFDFD), (0xFE70, 0xFEFC)];

/// `LTR`: the UTF-16 code unit ranges they take as strongly left-to-right.
/// Surrogates (U+D800–U+DFFF) fall in U+2C00–U+FB1C: the regular
/// expressions have no `u` flag, so they read code units.
pub const LTR: &[(u16, u16)] = &[
    (0x41, 0x5A),
    (0x61, 0x7A),
    (0x00C0, 0x00D6),
    (0x00D8, 0x00F6),
    (0x00F8, 0x02B8),
    (0x0300, 0x0590),
    (0x0800, 0x1FFF),
    (0x200E, 0x200E),
    (0x2C00, 0xFB1C),
    (0xFE00, 0xFE6F),
    (0xFEFD, 0xFFFF),
];

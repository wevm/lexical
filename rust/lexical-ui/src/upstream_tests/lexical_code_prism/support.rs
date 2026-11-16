//! Helpers for the ports of `packages/lexical-code-prism`'s unit tests.

use crate::code::{CodeLanguageState, CodeToken, Tokenizer};

/// Prism's place: Prism's grammars aren't ported (rust/PARITY.md,
/// `@lexical/code-core`), so the ports highlight with a small tokenizer
/// for `javascript` (upstream's default language) that splits code as
/// Prism's JavaScript grammar does for what the tests type: `let`, `const`
/// and `var` are keywords, runs of digits numbers, `=` an operator and
/// `;` punctuation (Swift's `TestCodeTokenizer`). Any other language is
/// unknown, as an unknown language is to Prism.
pub(super) struct TestTokenizer {
    /// `defaultLanguage`.
    pub(super) default_language: Option<&'static str>,
}

impl Default for TestTokenizer {
    fn default() -> Self {
        Self {
            default_language: Some(crate::code::DEFAULT_CODE_LANGUAGE),
        }
    }
}

impl Tokenizer for TestTokenizer {
    fn default_language(&self) -> Option<&str> {
        self.default_language
    }
    fn language_state(&self, language: &str) -> CodeLanguageState {
        if language == crate::code::DEFAULT_CODE_LANGUAGE {
            CodeLanguageState::Loaded
        } else {
            CodeLanguageState::Unsupported
        }
    }
    fn tokenize(&self, code: &str, _language: &str) -> Vec<CodeToken> {
        let mut tokens = Vec::new();
        let mut plain = String::new();
        let mut rest = code;
        while let Some(first) = rest.chars().next() {
            let word_len = rest
                .find(|c: char| !c.is_ascii_alphabetic())
                .unwrap_or(rest.len());
            let digits_len = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            let (token, len) = if matches!(&rest[..word_len], "let" | "const" | "var") {
                (Some("keyword"), word_len)
            } else if digits_len > 0 {
                (Some("number"), digits_len)
            } else if first == '=' {
                (Some("operator"), 1)
            } else if first == ';' {
                (Some("punctuation"), 1)
            } else {
                (None, word_len.max(first.len_utf8()))
            };
            match token {
                Some(token_type) => {
                    if !plain.is_empty() {
                        tokens.push(CodeToken::Text(std::mem::take(&mut plain)));
                    }
                    tokens.push(CodeToken::token(token_type, &rest[..len]));
                }
                None => plain.push_str(&rest[..len]),
            }
            rest = &rest[len..];
        }
        if !plain.is_empty() {
            tokens.push(CodeToken::Text(plain));
        }
        tokens
    }
}

/// `CodePrismExtension`: `code_highlighter_extension` with a
/// [`TestTokenizer`].
#[cfg(feature = "extension")]
pub(super) fn code_prism_extension(
    tokenizer: TestTokenizer,
) -> crate::extension_core::AnyLexicalExtensionArgument {
    use crate::code::{CodeHighlighterConfigOverrides, TokenizerRef, code_highlighter_extension};
    crate::config_extension(
        &code_highlighter_extension(),
        [CodeHighlighterConfigOverrides {
            tokenizer: Some(TokenizerRef(std::rc::Rc::new(tokenizer))),
            ..Default::default()
        }],
    )
}

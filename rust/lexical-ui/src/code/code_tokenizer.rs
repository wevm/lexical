//! The tokenizer interface of `@lexical/code-prism` and
//! `@lexical/code-shiki` (`Tokenizer`, with Prism's token shape), and
//! `$mapTokensToLexicalStructure` (`FacadePrism.ts`), without a grammar
//! engine: [`register_code_highlighting`](super::register_code_highlighting)
//! runs the highlighting loop over any [`Tokenizer`] an app supplies, the
//! way upstream lets Shiki stand in for Prism.
//!
//! Upstream's two engines differ in how a language becomes available. Prism
//! has its grammars loaded or not (`isCodeLanguageLoaded`); Shiki downloads
//! one on demand (`loadCodeLanguage`) and highlights the block again once it
//! has loaded. [`CodeLanguageState`] covers both, as Swift's does.

use super::code_highlight_node::create_code_highlight_node;
use super::flat_structure_utils::plainify_code_content;
use crate::node::NodeRef;
use crate::nodes::line_break::create_line_break_node;
use crate::nodes::tab::create_tab_node;
use crate::selection_insert_nodes::{RawTextVisitor, tokenize_raw_text};

/// `string | Token`: an item of what a tokenizer returns (Prism's
/// `TokenStream`), text without a type or a typed [`Token`].
#[derive(Clone, Debug, PartialEq)]
pub enum CodeToken {
    /// A string.
    Text(String),
    /// A `Token`.
    Token(Token),
}

impl CodeToken {
    /// A token of `token_type` whose content is the string `text`.
    pub fn token(token_type: &str, text: &str) -> CodeToken {
        CodeToken::Token(Token {
            token_type: token_type.to_owned(),
            alias: None,
            content: TokenContent::Text(text.to_owned()),
        })
    }
}

/// `Token`: a token of `type` (a Prism token name such as `keyword`,
/// `string`, `comment`, `function`, `number`, `operator` or
/// `punctuation`).
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    /// `type`.
    pub token_type: String,
    /// `alias` when it is a string: the type a `prefix` token with string
    /// content gives its text (a diff's `inserted`, say). JS's alias can
    /// also be an array, which the highlighter ignores, as it does `None`.
    pub alias: Option<String>,
    /// `content`.
    pub content: TokenContent,
}

/// `TokenContent`: what a [`Token`] holds.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenContent {
    /// A string, typed by the token (by its alias for a `prefix`).
    Text(String),
    /// A token on its own, which `$mapTokensToLexicalStructure` leaves out,
    /// as JS does.
    Token(Box<Token>),
    /// Tokens, typed by the token unless it is `unchanged` (a diff's
    /// unchanged line, whose text has no type).
    Tokens(Vec<CodeToken>),
}

/// Whether a tokenizer can highlight a language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeLanguageState {
    /// It has the language's grammar (`isCodeLanguageLoaded`).
    Loaded,
    /// It has the grammar, but not yet: [`Tokenizer::load_language`] gets
    /// it, as Shiki's `loadCodeLanguage` imports a grammar.
    Loading,
    /// It doesn't know the language: the block is plain text.
    Unsupported,
}

/// `Tokenizer`: what the highlighter asks a highlighting engine.
pub trait Tokenizer {
    /// `defaultLanguage`: the language of a code block without one, which
    /// the highlighter sets on the block. `None` (JS's `null`) opts out:
    /// such a block keeps no language and is split into plain tokens, so a
    /// Markdown round trip keeps a fence without an info string.
    fn default_language(&self) -> Option<&str>;

    /// Whether the tokenizer can highlight `language` now. JS's loop asks
    /// the engine's `isCodeLanguageLoaded`.
    fn language_state(&self, language: &str) -> CodeLanguageState;

    /// `tokenize(code, language)`: `code` as tokens, for a loaded
    /// `language`.
    fn tokenize(&self, code: &str, language: &str) -> Vec<CodeToken>;

    /// Loads `language`, whose state is [`CodeLanguageState::Loading`], and
    /// calls `on_ready` once it is loaded, on the editor's thread
    /// (Shiki's `loadCodeLanguage`); the highlighter then highlights the
    /// blocks waiting for it. The default does nothing, for a tokenizer
    /// whose languages are all loaded or unsupported.
    fn load_language(&self, language: &str, on_ready: Box<dyn FnOnce()>) {
        let _ = (language, on_ready);
    }

    /// `$tokenize(codeNode, language)`: the children that highlight
    /// `code_node`. The default is `PrismTokenizer.$tokenize`'s:
    /// `language || defaultLanguage`, and with neither the plain tokens of
    /// the block's text (`$plainifyCodeContent`), else
    /// [`map_tokens_to_lexical_structure`] of
    /// [`tokenize`](Self::tokenize) of it (`$getHighlightNodes`, without
    /// Prism's `diff-<language>` grammars).
    fn tokenize_code_node(&self, code_node: &NodeRef, language: Option<&str>) -> Vec<NodeRef> {
        let code = code_node.get_text_content();
        match language
            .filter(|l| !l.is_empty())
            .or(self.default_language())
        {
            None => plainify_code_content(&code),
            Some(language) => {
                map_tokens_to_lexical_structure(&self.tokenize(&code, language), None)
            }
        }
    }
}

struct MapTokens<'a> {
    nodes: &'a mut Vec<NodeRef>,
    highlight_type: Option<&'a str>,
}

impl RawTextVisitor for MapTokens<'_> {
    fn linebreak(&mut self) {
        self.nodes.push(create_line_break_node());
    }
    fn tab(&mut self) {
        self.nodes.push(create_tab_node());
    }
    fn text(&mut self, text: &str) {
        self.nodes
            .push(create_code_highlight_node(text, self.highlight_type));
    }
}

/// `$mapTokensToLexicalStructure(tokens, type)` (`FacadePrism.ts`): the
/// CodeHighlightNodes, TabNodes and LineBreakNodes of `tokens`, each string
/// split at tabs and line breaks (`tokenizeRawText`) and typed by its
/// innermost token: a `prefix` with string content by its alias, the
/// tokens of an `unchanged` one by none.
pub fn map_tokens_to_lexical_structure(
    tokens: &[CodeToken],
    highlight_type: Option<&str>,
) -> Vec<NodeRef> {
    let mut nodes = Vec::new();
    add_tokens(tokens, highlight_type, &mut nodes);
    nodes
}

fn add_tokens(tokens: &[CodeToken], highlight_type: Option<&str>, nodes: &mut Vec<NodeRef>) {
    for token in tokens {
        match token {
            CodeToken::Text(text) => tokenize_raw_text(
                text,
                &mut MapTokens {
                    nodes,
                    highlight_type,
                },
            ),
            CodeToken::Token(token) => match &token.content {
                TokenContent::Text(text) => {
                    let highlight_type = match (&token.alias, token.token_type.as_str()) {
                        (Some(alias), "prefix") => alias.as_str(),
                        _ => token.token_type.as_str(),
                    };
                    tokenize_raw_text(
                        text,
                        &mut MapTokens {
                            nodes,
                            highlight_type: Some(highlight_type),
                        },
                    );
                }
                TokenContent::Tokens(content) => add_tokens(
                    content,
                    (token.token_type != "unchanged").then_some(token.token_type.as_str()),
                    nodes,
                ),
                TokenContent::Token(_) => {}
            },
        }
    }
}

/// A tokenizer that knows no language: every code block is split into
/// plain tokens, so the indent and line-move handlers can work on it. The
/// default of `CodeHighlighterConfig`, as
/// Swift's `PlainCodeTokenizer` is; JS's default is `PrismTokenizer`, whose
/// grammars aren't ported.
#[derive(Clone, Debug, Default)]
pub struct PlainTokenizer {
    /// `defaultLanguage`.
    pub default_language: Option<String>,
}

impl Tokenizer for PlainTokenizer {
    fn default_language(&self) -> Option<&str> {
        self.default_language.as_deref()
    }
    fn language_state(&self, _language: &str) -> CodeLanguageState {
        CodeLanguageState::Unsupported
    }
    fn tokenize(&self, code: &str, _language: &str) -> Vec<CodeToken> {
        vec![CodeToken::Text(code.to_owned())]
    }
}

//! The tokenizers of a script's `codeHighlight` (`ops.ts`'s
//! `scriptTokenizer`), and the `codeBlocks` entry logged after each of its
//! steps (`codeBlocksEntry`).

use lexical::code::{CodeLanguageState, CodeToken, Token, TokenContent, Tokenizer, is_code_node};
use lexical::json::Value;
use lexical::{Editor, get_root};
use std::rc::Rc;

/// `ops.ts`'s tokenizer for `spec`: CONFORMANCE_TOKENIZE (`conformance`),
/// or the tokens it recorded from Prism (`recorded`). Either has only the
/// spec's `languages`.
pub(super) fn tokenizer(spec: &Value) -> Result<Rc<dyn Tokenizer>, String> {
    let strings = |key: &str| -> Vec<String> {
        spec.get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect()
    };
    let default_language = match spec.get("defaultLanguage") {
        None => Some(lexical::code::DEFAULT_CODE_LANGUAGE.to_owned()),
        Some(Value::Null) => None,
        Some(v) => Some(v.as_str().ok_or("defaultLanguage")?.to_owned()),
    };
    let recorded = match spec.get("tokenizer").and_then(Value::as_str) {
        Some("conformance") => None,
        Some("recorded") => Some(
            spec.get("tokens")
                .and_then(Value::as_array)
                .ok_or("recorded tokens")?
                .iter()
                .map(|entry| {
                    let entry = entry
                        .as_array()
                        .filter(|e| e.len() == 3)
                        .ok_or("a token entry")?;
                    let text = |v: &Value| v.as_str().map(str::to_owned).ok_or("a string");
                    Ok((text(&entry[0])?, text(&entry[1])?, tokens(&entry[2])?))
                })
                .collect::<Result<Vec<_>, String>>()?,
        ),
        other => return Err(format!("unknown tokenizer {other:?}")),
    };
    Ok(Rc::new(ScriptTokenizer {
        languages: strings("languages"),
        default_language,
        recorded,
    }))
}

/// `TokenJson[]` as tokens.
fn tokens(json: &Value) -> Result<Vec<CodeToken>, String> {
    json.as_array()
        .ok_or("tokens")?
        .iter()
        .map(|token| match token {
            Value::String(text) => Ok(CodeToken::Text(text.clone())),
            Value::Object(object) => Ok(CodeToken::Token(Token {
                token_type: object
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or("a token's type")?
                    .to_owned(),
                alias: object
                    .get("alias")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                content: match object.get("content") {
                    Some(Value::String(text)) => TokenContent::Text(text.clone()),
                    Some(content) => TokenContent::Tokens(tokens(content)?),
                    None => return Err("a token's content".to_owned()),
                },
            })),
            _ => Err("a token".to_owned()),
        })
        .collect()
}

struct ScriptTokenizer {
    languages: Vec<String>,
    default_language: Option<String>,
    /// `[language, code, tokens]` of `recorded`.
    recorded: Option<Vec<(String, String, Vec<CodeToken>)>>,
}

impl Tokenizer for ScriptTokenizer {
    fn default_language(&self) -> Option<&str> {
        self.default_language.as_deref()
    }

    fn language_state(&self, language: &str) -> CodeLanguageState {
        if self.languages.iter().any(|l| l == language) {
            CodeLanguageState::Loaded
        } else {
            CodeLanguageState::Unsupported
        }
    }

    fn tokenize(&self, code: &str, language: &str) -> Vec<CodeToken> {
        assert!(
            self.languages.iter().any(|l| l == language),
            "the tokenizer has no {language}"
        );
        match &self.recorded {
            None => conformance_tokenize(code, language),
            Some(recorded) => recorded
                .iter()
                .find(|(l, c, _)| l == language && c == code)
                .unwrap_or_else(|| panic!("no tokens recorded for {language}: {code}"))
                .2
                .clone(),
        }
    }
}

const CONFORMANCE_KEYWORDS: [&str; 4] = ["fn", "if", "let", "return"];

fn is_conformance_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// An ASCII letter, digit or `_`, or any code point past ASCII.
fn is_conformance_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || !c.is_ascii()
}

fn token(token_type: &str, content: TokenContent, alias: Option<&str>) -> CodeToken {
    CodeToken::Token(Token {
        token_type: token_type.to_owned(),
        alias: alias.map(str::to_owned),
        content,
    })
}

/// `conformanceWords(code)`, over code points.
fn conformance_words(code: &str) -> Vec<CodeToken> {
    let chars: Vec<char> = code.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let run_of = |i: &mut usize, test: &dyn Fn(char) -> bool| -> String {
        let start = *i;
        while *i < chars.len() && test(chars[*i]) {
            *i += 1;
        }
        chars[start..*i].iter().collect()
    };
    while i < chars.len() {
        let c = chars[i];
        if is_conformance_space(c) {
            out.push(CodeToken::Text(run_of(&mut i, &is_conformance_space)));
        } else if is_conformance_word(c) {
            let run = run_of(&mut i, &is_conformance_word);
            out.push(if CONFORMANCE_KEYWORDS.contains(&run.as_str()) {
                token("keyword", TokenContent::Text(run), None)
            } else if run.chars().all(|c| c.is_ascii_digit()) {
                token("number", TokenContent::Text(run), None)
            } else {
                token(
                    "word",
                    TokenContent::Tokens(vec![CodeToken::Text(run)]),
                    None,
                )
            });
        } else if c == '"' {
            let mut content = vec![CodeToken::Text("\"".to_owned())];
            i += 1;
            let mut text = String::new();
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    if !text.is_empty() {
                        content.push(CodeToken::Text(std::mem::take(&mut text)));
                    }
                    let escape = ['\\', chars[i + 1]].iter().collect();
                    content.push(token("escape", TokenContent::Text(escape), None));
                    i += 2;
                } else {
                    text.push(chars[i]);
                    i += 1;
                }
            }
            if !text.is_empty() {
                content.push(CodeToken::Text(text));
            }
            if i < chars.len() {
                content.push(CodeToken::Text("\"".to_owned()));
                i += 1;
            }
            out.push(token("string", TokenContent::Tokens(content), None));
        } else if c == '#' {
            let comment = run_of(&mut i, &|d| d != '\n');
            out.push(token("comment", TokenContent::Text(comment), None));
        } else {
            out.push(token("punctuation", TokenContent::Text(c.into()), None));
            i += 1;
        }
    }
    out
}

/// `CONFORMANCE_TOKENIZE(code, language)`.
fn conformance_tokenize(code: &str, language: &str) -> Vec<CodeToken> {
    if language != "diff" {
        return conformance_words(code);
    }
    let mut out = Vec::new();
    for (i, line) in code.split('\n').enumerate() {
        if i > 0 {
            out.push(CodeToken::Text("\n".to_owned()));
        }
        let mut chars = line.chars();
        let first = chars.next();
        let line_type = match first {
            Some(' ') => "unchanged",
            Some('!') => "changed",
            Some('+') => "inserted",
            Some('-') => "deleted",
            _ => {
                out.extend(conformance_words(line));
                continue;
            }
        };
        let first = first.expect("a prefix").to_string();
        let alias = (line_type != "changed").then_some(line_type);
        let mut content = vec![token("prefix", TokenContent::Text(first), alias)];
        content.extend(conformance_words(chars.as_str()));
        out.push(token(line_type, TokenContent::Tokens(content), None));
    }
    out
}

/// `codeBlocksEntry(editor)`: `['codeBlocks', [[key, language,
/// isSyntaxHighlightSupported, childKeys]...]]` of the root's code blocks.
pub(super) fn code_blocks_entry(editor: &Editor) -> Result<Value, String> {
    editor
        .read(|| {
            let blocks = get_root()
                .get_children()
                .into_iter()
                .filter(is_code_node)
                .map(|block| {
                    Value::Array(vec![
                        block.get_key().into(),
                        block.get_language().map_or(Value::Null, Value::from),
                        block.get_is_syntax_highlight_supported().into(),
                        Value::Array(
                            block
                                .get_children()
                                .iter()
                                .map(|c| c.get_key().into())
                                .collect(),
                        ),
                    ])
                })
                .collect();
            Value::Array(vec!["codeBlocks".into(), Value::Array(blocks)])
        })
        .map_err(|e| format!("committing: {e}"))
}

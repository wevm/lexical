//! Ports of `packages/lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts`,
//! one table-driven test whose table and markers were generated from
//! upstream's `test.for` tables.

use crate::link::auto_link_url_matcher as url_matcher;

/// What a case checks: the match's `text`, its `url`, or no match.
enum Expect {
    Text(&'static str),
    Url(&'static str),
    None,
}

/// `(input, expected)` of each case, in upstream's order.
const CASES: [(&str, Expect); 29] = [
    ("https://google.com", Expect::Text("https://google.com")),
    ("www.example.com", Expect::Text("www.example.com")),
    (
        "https://example.com/path?q=1#hash",
        Expect::Text("https://example.com/path?q=1#hash"),
    ),
    (
        "https://example.com:8080/api",
        Expect::Text("https://example.com:8080/api"),
    ),
    (
        "https://qabilah.com/posts/عربي",
        Expect::Text("https://qabilah.com/posts/عربي"),
    ),
    ("https://예시.한국", Expect::Text("https://예시.한국")),
    ("https://пример.рф", Expect::Text("https://пример.рф")),
    (
        "https://例子.中国/测试",
        Expect::Text("https://例子.中国/测试"),
    ),
    (
        "http://예시.한국/경로?키=값#부분",
        Expect::Text("http://예시.한국/경로?키=값#부분"),
    ),
    (
        "http://مثال.موقع/مسار?مفتاح=قيمة#قسم",
        Expect::Text("http://مثال.موقع/مسار?مفتاح=قيمة#قسم"),
    ),
    (
        "hello https://google.com world",
        Expect::Text("https://google.com"),
    ),
    (
        "مرحبا https://qabilah.com/posts/عربي end",
        Expect::Text("https://qabilah.com/posts/عربي"),
    ),
    (
        "텍스트 https://예시.한국 끝",
        Expect::Text("https://예시.한국"),
    ),
    ("go www.예시.한국 end", Expect::Text("www.예시.한국")),
    (
        "see https://google.com, ok",
        Expect::Text("https://google.com"),
    ),
    (
        "see https://google.com. ok",
        Expect::Text("https://google.com"),
    ),
    (
        "see https://예시.한국, ok",
        Expect::Text("https://예시.한국"),
    ),
    (
        "see https://예시.한국. ok",
        Expect::Text("https://예시.한국"),
    ),
    ("(https://google.com)", Expect::Text("https://google.com")),
    ("(https://예시.한국)", Expect::Text("https://예시.한국")),
    (
        "https://en.wikipedia.org/wiki/Fish_(disambiguation)",
        Expect::Text("https://en.wikipedia.org/wiki/Fish_(disambiguation)"),
    ),
    (
        "https://example.com/a_(b_(c))",
        Expect::Text("https://example.com/a_(b_(c))"),
    ),
    ("www.example.com", Expect::Url("https://www.example.com")),
    ("www.예시.한국", Expect::Url("https://www.예시.한국")),
    ("https://google.com", Expect::Url("https://google.com")),
    ("http://пример.рф", Expect::Url("http://пример.рф")),
    ("just text no url", Expect::None),
    ("http:// alone", Expect::None),
    ("", Expect::None),
];

// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches ASCII URL: https
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches ASCII URL: www prefix
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches ASCII URL: path+query+hash
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches ASCII URL: port
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: Arabic path
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: Korean IDN
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: Cyrillic IDN
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: CJK IDN+path
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: Korean full URL
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: Arabic full URL
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > extracts URL from surrounding text: ASCII surrounded
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > extracts URL from surrounding text: Arabic surrounded
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > extracts URL from surrounding text: Korean surrounded
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > extracts URL from surrounding text: www Korean IDN
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > excludes trailing punctuation: trailing comma
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > excludes trailing punctuation: trailing period
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > excludes trailing punctuation: Korean trailing comma
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > excludes trailing punctuation: Korean trailing period
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > handles parentheses correctly: wrapped in parens
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > handles parentheses correctly: Korean wrapped in parens
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > handles parentheses correctly: Wikipedia balanced
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > handles parentheses correctly: nested balanced
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > sets correct url for: www gets https
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > sets correct url for: Korean www gets https
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > sets correct url for: https stays
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > sets correct url for: http stays
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > returns null for non-URL: plain text
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > returns null for non-URL: bare protocol
// upstream: lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > returns null for non-URL: empty string
#[test]
fn auto_link_url_matcher() {
    for (input, expected) in CASES {
        let result = url_matcher(input);
        match expected {
            Expect::Text(text) => assert_eq!(result.expect(input).text, text),
            Expect::Url(url) => assert_eq!(result.expect(input).url, url),
            Expect::None => assert!(result.is_none(), "{input}"),
        }
    }
}

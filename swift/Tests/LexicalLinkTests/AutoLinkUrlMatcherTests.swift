import LexicalLink
import Testing

// Port of packages/lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts.

@Suite struct AutoLinkUrlMatcherTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches ASCII URL: https"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches ASCII URL: www prefix"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches ASCII URL: path+query+hash"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches ASCII URL: port"
    ),
    arguments: [
      ("https://google.com", "https://google.com"),
      ("www.example.com", "www.example.com"),
      ("https://example.com/path?q=1#hash", "https://example.com/path?q=1#hash"),
      ("https://example.com:8080/api", "https://example.com:8080/api"),
    ])
  func matchesASCIIURL(input: String, expected: String) throws {
    let result = try #require(autoLinkUrlMatcher(input))
    #expect(result.text == expected)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: Arabic path"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: Korean IDN"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: Cyrillic IDN"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: CJK IDN+path"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: Korean full URL"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > matches Unicode URL: Arabic full URL"
    ),
    arguments: [
      ("https://qabilah.com/posts/عربي", "https://qabilah.com/posts/عربي"),
      ("https://예시.한국", "https://예시.한국"),
      ("https://пример.рф", "https://пример.рф"),
      ("https://例子.中国/测试", "https://例子.中国/测试"),
      ("http://예시.한국/경로?키=값#부분", "http://예시.한국/경로?키=값#부분"),
      ("http://مثال.موقع/مسار?مفتاح=قيمة#قسم", "http://مثال.موقع/مسار?مفتاح=قيمة#قسم"),
    ])
  func matchesUnicodeURL(input: String, expected: String) throws {
    let result = try #require(autoLinkUrlMatcher(input))
    #expect(result.text == expected)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > extracts URL from surrounding text: ASCII surrounded"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > extracts URL from surrounding text: Arabic surrounded"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > extracts URL from surrounding text: Korean surrounded"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > extracts URL from surrounding text: www Korean IDN"
    ),
    arguments: [
      ("hello https://google.com world", "https://google.com"),
      ("مرحبا https://qabilah.com/posts/عربي end", "https://qabilah.com/posts/عربي"),
      ("텍스트 https://예시.한국 끝", "https://예시.한국"),
      ("go www.예시.한국 end", "www.예시.한국"),
    ])
  func extractsURLFromSurroundingText(input: String, expected: String) throws {
    let result = try #require(autoLinkUrlMatcher(input))
    #expect(result.text == expected)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > excludes trailing punctuation: trailing comma"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > excludes trailing punctuation: trailing period"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > excludes trailing punctuation: Korean trailing comma"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > excludes trailing punctuation: Korean trailing period"
    ),
    arguments: [
      ("see https://google.com, ok", "https://google.com"),
      ("see https://google.com. ok", "https://google.com"),
      ("see https://예시.한국, ok", "https://예시.한국"),
      ("see https://예시.한국. ok", "https://예시.한국"),
    ])
  func excludesTrailingPunctuation(input: String, expected: String) throws {
    let result = try #require(autoLinkUrlMatcher(input))
    #expect(result.text == expected)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > handles parentheses correctly: wrapped in parens"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > handles parentheses correctly: Korean wrapped in parens"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > handles parentheses correctly: Wikipedia balanced"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > handles parentheses correctly: nested balanced"
    ),
    arguments: [
      ("(https://google.com)", "https://google.com"),
      ("(https://예시.한국)", "https://예시.한국"),
      (
        "https://en.wikipedia.org/wiki/Fish_(disambiguation)",
        "https://en.wikipedia.org/wiki/Fish_(disambiguation)"
      ),
      ("https://example.com/a_(b_(c))", "https://example.com/a_(b_(c))"),
    ])
  func handlesParentheses(input: String, expected: String) throws {
    let result = try #require(autoLinkUrlMatcher(input))
    #expect(result.text == expected)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > sets correct url for: www gets https"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > sets correct url for: Korean www gets https"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > sets correct url for: https stays"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > sets correct url for: http stays"
    ),
    arguments: [
      ("www.example.com", "https://www.example.com"),
      ("www.예시.한국", "https://www.예시.한국"),
      ("https://google.com", "https://google.com"),
      ("http://пример.рф", "http://пример.рф"),
    ])
  func setsCorrectURL(input: String, expected: String) throws {
    let result = try #require(autoLinkUrlMatcher(input))
    #expect(result.url == expected)
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > returns null for non-URL: plain text"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > returns null for non-URL: bare protocol"
    ),
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkUrlMatcher.test.ts::autoLinkUrlMatcher > returns null for non-URL: empty string"
    ),
    arguments: ["just text no url", "http:// alone", ""])
  func returnsNilForNonURL(input: String) {
    #expect(autoLinkUrlMatcher(input) == nil)
  }
}

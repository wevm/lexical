import Lexical
import LexicalTesting
import Testing

// Port of the getStyleObjectFromCSS cases of
// packages/lexical/src/__tests__/unit/setDOMStyle.test.ts.

@Suite struct GetStyleObjectFromCSSTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/setDOMStyle.test.ts::setDOMStyle > parses CSS text into a style object"
    ))
  func parsesCSSText() {
    #expect(
      getStyleObjectFromCSS("color: red; margin: 0 !important; --custom: example;")
        == ["--custom": "example", "color": "red", "margin": "0 !important"])
    #expect(
      getStyleObjectFromCSS("color: red; margin: 0 !important; --custom: example;").keys == [
        "color", "margin", "--custom",
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/setDOMStyle.test.ts::setDOMStyle > parses values with comments, semicolons and colons inside quotes and parentheses"
    ))
  func parsesQuotedValues() {
    #expect(
      getStyleObjectFromCSS(
        "background-image: url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'></svg>\"); /* ignored */ content: \"semi;colon:value\"; color: red;"
      ) == [
        "background-image":
          "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg'></svg>\")",
        "color": "red",
        "content": "\"semi;colon:value\"",
      ])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/setDOMStyle.test.ts::setDOMStyle > returns a fresh style object for each parse"
    ))
  func returnsFreshObject() {
    var styleObject = getStyleObjectFromCSS("color: red;")
    styleObject["color"] = "blue"
    #expect(getStyleObjectFromCSS("color: red;") == ["color": "red"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/setDOMStyle.test.ts::setDOMStyle > drops comments and respects quotes and parentheses"
    ))
  func dropsComments() {
    #expect(
      getStyleObjectFromCSS("color: re/* x */d; margin: 0") == ["color": "red", "margin": "0"])
    #expect(getStyleObjectFromCSS("a/* c1 *//* c2 */b: c") == ["ab": "c"])
    #expect(getStyleObjectFromCSS("content: \"a\\\"b;c:d\"") == ["content": "\"a\\\"b;c:d\""])
    #expect(
      getStyleObjectFromCSS("background: url(http://x/a;b);") == ["background": "url(http://x/a;b)"]
    )
  }
}

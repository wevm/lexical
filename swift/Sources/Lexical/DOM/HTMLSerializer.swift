// The HTML fragment serialization algorithm (`innerHTML` and `outerHTML`),
// escaping as jsdom does: attribute values escape `&`, `"` and U+00A0;
// text escapes `&`, `<`, `>` and U+00A0, except inside raw-text elements.

private let voidElements: Set<String> = [
  "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
  "keygen", "link", "meta", "param", "source", "track", "wbr",
]

private let rawTextElements: Set<String> = [
  "style", "script", "xmp", "iframe", "noembed", "noframes", "plaintext",
]

/// Whether an element is void: it has no end tag and no children in HTML.
func isVoidHTMLElement(_ localName: String) -> Bool { voidElements.contains(localName) }

func serializeHTMLChildren(of node: DOMNode) -> String {
  var out = ""
  for child in node.childNodes { serializeHTML(child, into: &out) }
  return out
}

func serializeHTML(_ node: DOMNode, into out: inout String) {
  switch node {
  case let element as DOMElement:
    out += "<" + element.localName
    for (name, value) in element.attributes {
      out += " " + name + "=\""
      escapeHTML(value, attribute: true, into: &out)
      out += "\""
    }
    out += ">"
    if isVoidHTMLElement(element.localName) { return }
    for child in element.childNodes { serializeHTML(child, into: &out) }
    out += "</" + element.localName + ">"
  case let text as DOMText:
    if let parent = text.parentElement, rawTextElements.contains(parent.localName) {
      out += text.data
    } else {
      escapeHTML(text.data, attribute: false, into: &out)
    }
  case let comment as DOMComment:
    out += "<!--" + comment.data + "-->"
  default:
    for child in node.childNodes { serializeHTML(child, into: &out) }
  }
}

private func escapeHTML(_ string: String, attribute: Bool, into out: inout String) {
  for scalar in string.unicodeScalars {
    switch scalar {
    case "&": out += "&amp;"
    case "\u{A0}": out += "&nbsp;"
    case "\"" where attribute: out += "&quot;"
    case "<" where !attribute: out += "&lt;"
    case ">" where !attribute: out += "&gt;"
    default: out.unicodeScalars.append(scalar)
    }
  }
}

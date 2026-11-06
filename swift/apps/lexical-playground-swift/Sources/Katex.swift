import Foundation
import JavaScriptCore
import Lexical

// KaTeX for `EquationNode.exportDOM`, which on the web appends what
// `katex.render` builds to the exported element. KaTeX (the version the web
// playground resolves, in `katex/`) runs in JavaScriptCore against a small
// DOM that keeps what `render` does to it as a browser would serialize it:
// attributes in the order they were set, and properties set on `style` as
// `name: value;` (a hex color as `rgb(…)`), while a `style` set with
// `setAttribute` stays as written. The tree is then rebuilt in the editor's
// DOM.

/// The `katex/` folder in the app's bundle: KaTeX's script, style sheet and
/// fonts, and the page the equation views draw in.
let katexDirectory = Bundle.main.url(forResource: "katex", withExtension: nil)!

/// A node of what `katex.render` built.
enum KatexNode: Equatable {
  case element(tag: String, attributes: [(String, String)], children: [KatexNode])
  case text(String)

  static func == (lhs: KatexNode, rhs: KatexNode) -> Bool {
    switch (lhs, rhs) {
    case (.text(let a), .text(let b)): a == b
    case (.element(let t1, let a1, let c1), .element(let t2, let a2, let c2)):
      t1 == t2 && a1.elementsEqual(a2, by: ==) && c1 == c2
    default: false
    }
  }

  /// The node in `document`.
  func dom(_ document: DOMDocument) throws -> DOMNode {
    switch self {
    case .text(let text): return document.createTextNode(text)
    case .element(let tag, let attributes, let children):
      let element = document.createElement(tag)
      for (name, value) in attributes { try element.setAttribute(name, value) }
      for child in children { _ = try element.appendChild(child.dom(document)) }
      return element
    }
  }
}

/// `katex.render(equation, element, options)` as `KatexRenderer` and
/// `EquationNode.exportDOM` call it, as the nodes it appends to `element`.
func renderKatex(_ equation: String, displayMode: Bool) -> [KatexNode] {
  katexLock.lock()
  defer { katexLock.unlock() }
  guard let render = katexRender,
    let json = render.call(withArguments: [equation, displayMode])?.toString(),
    let data = json.data(using: .utf8),
    let nodes = try? JSONSerialization.jsonObject(with: data) as? [Any]
  else { return [] }
  return nodes.compactMap(katexNode)
}

private func katexNode(_ value: Any) -> KatexNode? {
  if let text = value as? String { return .text(text) }
  guard let object = value as? [String: Any], let tag = object["tag"] as? String else { return nil }
  let attributes = (object["attrs"] as? [[String]] ?? []).compactMap { pair in
    pair.count == 2 ? (pair[0], pair[1]) : nil
  }
  let children = (object["children"] as? [Any] ?? []).compactMap(katexNode)
  return .element(tag: tag, attributes: attributes, children: children)
}

private let katexLock = NSLock()

/// The context's `render(equation, displayMode)`, which returns the nodes as
/// JSON. Built once, on first use.
nonisolated(unsafe) private let katexRender: JSValue? = {
  guard let context = JSContext(),
    let script = try? String(
      contentsOf: katexDirectory.appendingPathComponent("katex.min.js"), encoding: .utf8)
  else { return nil }
  context.evaluateScript(katexDOMShim)
  context.evaluateScript(script)
  context.evaluateScript(
    """
    var render = function (equation, displayMode) {
      var base = document.createElement('span');
      katex.render(equation, base, {
        displayMode: displayMode, errorColor: '#cc0000', output: 'html',
        strict: 'warn', throwOnError: false, trust: false,
      });
      return JSON.stringify(base.childNodes.map(serialize));
    };
    """)
  return context.objectForKeyedSubscript("render")
}()

/// The DOM `katex.render` uses: `createElement`, `createElementNS`,
/// `createTextNode` and `createDocumentFragment`, and on elements
/// `className`, `style[name]`, `setAttribute`, `appendChild` and
/// `textContent`.
private let katexDOMShim = """
  var console = { log: function () {}, warn: function () {}, error: function () {} };
  function hyphenate(name) {
    return name.replace(/[A-Z]/g, function (c) { return '-' + c.toLowerCase(); });
  }
  // CSSOM serializes a hex color as rgb() or rgba().
  function cssValue(name, value) {
    var hex = /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.exec(value);
    if (!hex || name.indexOf('color') < 0) return value;
    var digits = hex[1];
    if (digits.length <= 4) digits = digits.replace(/./g, function (c) { return c + c; });
    var channel = function (i) { return parseInt(digits.substr(i * 2, 2), 16); };
    if (digits.length === 8) {
      var alpha = Math.round(channel(3) / 255 * 1000) / 1000;
      return 'rgba(' + channel(0) + ', ' + channel(1) + ', ' + channel(2) + ', ' + alpha + ')';
    }
    return 'rgb(' + channel(0) + ', ' + channel(1) + ', ' + channel(2) + ')';
  }
  function Parent() {}
  Parent.prototype.appendChild = function (child) {
    if (child.isFragment) {
      for (var i = 0; i < child.childNodes.length; i++) this.childNodes.push(child.childNodes[i]);
      child.childNodes = [];
    } else {
      this.childNodes.push(child);
    }
    return child;
  };
  function Fragment() { this.isFragment = true; this.childNodes = []; }
  Fragment.prototype = Object.create(Parent.prototype);
  function Element(tag) {
    var self = this;
    this.tag = tag;
    this.attrs = [];
    this.childNodes = [];
    this.declarations = null;
    this.style = new Proxy({}, {
      set: function (_, name, value) { self.setStyle(hyphenate(String(name)), String(value)); return true; },
      get: function (_, name) { return self.getStyle(hyphenate(String(name))); },
    });
  }
  Element.prototype = Object.create(Parent.prototype);
  Element.prototype.indexOf = function (name) {
    for (var i = 0; i < this.attrs.length; i++) if (this.attrs[i][0] === name) return i;
    return -1;
  };
  Element.prototype.setAttribute = function (name, value) {
    name = String(name); value = String(value);
    var i = this.indexOf(name);
    if (i >= 0) this.attrs[i][1] = value; else this.attrs.push([name, value]);
    if (name === 'style') this.declarations = null;
  };
  Element.prototype.getAttribute = function (name) {
    var i = this.indexOf(name);
    return i >= 0 ? this.attrs[i][1] : null;
  };
  Element.prototype.getStyle = function (name) {
    var list = this.declarations || [];
    for (var i = 0; i < list.length; i++) if (list[i][0] === name) return list[i][1];
    return '';
  };
  Element.prototype.setStyle = function (name, value) {
    var list = this.declarations || [];
    var i = -1;
    for (var j = 0; j < list.length; j++) if (list[j][0] === name) i = j;
    if (value === '') { if (i >= 0) list.splice(i, 1); }
    else if (i >= 0) list[i][1] = cssValue(name, value);
    else list.push([name, cssValue(name, value)]);
    this.declarations = list;
    var text = list.map(function (d) { return d[0] + ': ' + d[1] + ';'; }).join(' ');
    var at = this.indexOf('style');
    if (text === '') { if (at >= 0) this.attrs.splice(at, 1); }
    else if (at >= 0) this.attrs[at][1] = text;
    else this.attrs.push(['style', text]);
  };
  Object.defineProperty(Element.prototype, 'className', {
    get: function () { return this.getAttribute('class') || ''; },
    set: function (value) { this.setAttribute('class', value); },
  });
  ['src', 'alt'].forEach(function (name) {
    Object.defineProperty(Element.prototype, name, {
      get: function () { return this.getAttribute(name) || ''; },
      set: function (value) { this.setAttribute(name, value); },
    });
  });
  Object.defineProperty(Element.prototype, 'textContent', {
    set: function (value) {
      this.childNodes = [];
      if (String(value) !== '') this.childNodes.push(String(value));
    },
  });
  var document = {
    compatMode: 'CSS1Compat',
    createElement: function (tag) { return new Element(tag); },
    createElementNS: function (_, tag) { return new Element(tag); },
    createTextNode: function (text) { return String(text); },
    createDocumentFragment: function () { return new Fragment(); },
  };
  function serialize(node) {
    if (typeof node === 'string') return node;
    return { tag: node.tag, attrs: node.attrs, children: node.childNodes.map(serialize) };
  }
  """

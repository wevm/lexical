/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {JSDOM} from 'jsdom';

// jsdom 30 serializes `innerHTML` and `outerHTML` with parse5, which still
// leaves `<` and `>` as they are in attribute values. The HTML standard
// escapes them since 2025, and Chromium and WebKit do,
// so the vectors are what a browser writes: the windows here serialize with
// the standard's HTML fragment serialization algorithm.

const VOID_ELEMENTS = new Set([
  'area',
  'base',
  'basefont',
  'bgsound',
  'br',
  'col',
  'embed',
  'frame',
  'hr',
  'img',
  'input',
  'keygen',
  'link',
  'meta',
  'param',
  'source',
  'track',
  'wbr',
]);

// `noscript` too only where scripting is enabled, which it is not in jsdom.
const RAW_TEXT_ELEMENTS = new Set([
  'style',
  'script',
  'xmp',
  'iframe',
  'noembed',
  'noframes',
  'plaintext',
]);

const HTML_NS = 'http://www.w3.org/1999/xhtml';
const SVG_NS = 'http://www.w3.org/2000/svg';
const MATHML_NS = 'http://www.w3.org/1998/Math/MathML';
const XML_NS = 'http://www.w3.org/XML/1998/namespace';
const XMLNS_NS = 'http://www.w3.org/2000/xmlns/';
const XLINK_NS = 'http://www.w3.org/1999/xlink';

/** "Escaping a string", in attribute mode or not. */
function escapeString(value: string, attributeMode: boolean): string {
  return value.replace(attributeMode ? /[&\u00A0"<>]/g : /[&\u00A0<>]/g, c =>
    c === '&'
      ? '&amp;'
      : c === '\u00A0'
        ? '&nbsp;'
        : c === '"'
          ? '&quot;'
          : c === '<'
            ? '&lt;'
            : '&gt;',
  );
}

function serializedAttributeName(attr: Attr): string {
  switch (attr.namespaceURI) {
    case null:
      return attr.localName;
    case XML_NS:
      return 'xml:' + attr.localName;
    case XMLNS_NS:
      return attr.localName === 'xmlns' ? 'xmlns' : 'xmlns:' + attr.localName;
    case XLINK_NS:
      return 'xlink:' + attr.localName;
    default:
      return attr.name;
  }
}

/** The children of `node`, as the HTML fragment serialization algorithm. */
function serializeChildren(node: Node): string {
  const parent =
    node.nodeType === 1 && (node as Element).localName === 'template'
      ? (node as HTMLTemplateElement).content
      : node;
  let out = '';
  for (let child = parent.firstChild; child; child = child.nextSibling) {
    out += serializeNode(child, parent);
  }
  return out;
}

function serializeNode(node: Node, parent: Node | null): string {
  switch (node.nodeType) {
    case 1: {
      const element = node as Element;
      const ns = element.namespaceURI;
      const tagName =
        ns === HTML_NS || ns === SVG_NS || ns === MATHML_NS
          ? element.localName
          : element.tagName;
      let out = '<' + tagName;
      for (const attr of Array.from(element.attributes)) {
        out +=
          ' ' +
          serializedAttributeName(attr) +
          '="' +
          escapeString(attr.value, true) +
          '"';
      }
      out += '>';
      if (ns === HTML_NS && VOID_ELEMENTS.has(element.localName)) {
        return out;
      }
      return out + serializeChildren(element) + '</' + tagName + '>';
    }
    case 3: {
      const data = (node as Text).data;
      const parentElement =
        parent && parent.nodeType === 1 ? (parent as Element) : null;
      return parentElement &&
        parentElement.namespaceURI === HTML_NS &&
        RAW_TEXT_ELEMENTS.has(parentElement.localName)
        ? data
        : escapeString(data, false);
    }
    case 4:
      return (node as CDATASection).data;
    case 7: {
      const pi = node as ProcessingInstruction;
      return '<?' + pi.target + ' ' + pi.data + '>';
    }
    case 8:
      return '<!--' + (node as Comment).data + '-->';
    case 10:
      return '<!DOCTYPE ' + (node as DocumentType).name + '>';
    default:
      return serializeChildren(node);
  }
}

/** A new jsdom window whose `innerHTML` and `outerHTML` serialize as a
 * browser's do. */
export function browserWindow(): JSDOM['window'] {
  const win = new JSDOM('').window;
  for (const proto of [win.Element.prototype, win.ShadowRoot.prototype]) {
    const inner = Object.getOwnPropertyDescriptor(proto, 'innerHTML')!;
    Object.defineProperty(proto, 'innerHTML', {
      ...inner,
      get(this: Node) {
        return serializeChildren(this);
      },
    });
  }
  const outer = Object.getOwnPropertyDescriptor(
    win.Element.prototype,
    'outerHTML',
  )!;
  Object.defineProperty(win.Element.prototype, 'outerHTML', {
    ...outer,
    get(this: Element) {
      return serializeNode(this, this.parentNode);
    },
  });
  return win;
}

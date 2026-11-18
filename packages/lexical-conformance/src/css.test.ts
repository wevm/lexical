/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {JSDOM} from 'jsdom';
import {test} from 'vitest';

import {emitVectors} from './vectors';

// The inline style declaration Lexical writes through (`setDOMStyleFromCSS`,
// `element.style.*`) is jsdom's CSSStyleDeclaration in the upstream tests,
// and its normalization shows up byte for byte in exported HTML. These
// vectors pin what it accepts and how it serializes, for the Swift DOM.

const doc = new JSDOM('').window.document;

test('css/properties', () => {
  // The property names an inline style accepts, as `setProperty` takes them.
  const names = new Set<string>();
  for (
    let proto = Object.getPrototypeOf(doc.createElement('div').style);
    proto !== null;
    proto = Object.getPrototypeOf(proto)
  ) {
    for (const name of Object.getOwnPropertyNames(proto)) {
      const descriptor = Object.getOwnPropertyDescriptor(proto, name);
      if (
        descriptor !== undefined &&
        descriptor.get !== undefined &&
        name === name.toLowerCase() &&
        /^-?[a-z][a-z-]*$/.test(name)
      ) {
        names.add(name);
      }
    }
  }
  const probe = (name: string, value: string): string => {
    const element = doc.createElement('div');
    element.style.setProperty(name, value);
    return element.style.getPropertyValue(name);
  };
  // Each accepted property, with how it normalizes the values Lexical
  // writes: hex colors as rgb(), a bare 0 as 0px, and keyword case.
  const cases: PropertyCase[] = [...names]
    .filter(name => probe(name, 'initial') !== '')
    .sort()
    .map(name => ({
      color:
        probe(name, '#f00') === 'rgb(255, 0, 0)' ||
        probe(name, '1px solid #f00') === '1px solid rgb(255, 0, 0)',
      name,
      preserveCase: probe(name, 'Foo') === 'Foo',
      verbatim: probe(name, '1PX 1px RED') === '1PX 1px RED',
      zeroLength: probe(name, '0') === '0px',
    }));
  emitVectors(
    'css/properties',
    'css-properties',
    'The CSS properties an inline style declaration accepts, sorted, and how each normalizes hex colors and a bare 0, whether it keeps the case of an identifier (font names) rather than lowercasing keywords, and whether it keeps a value as written (verbatim) rather than parsing it.',
    cases,
  );
});

type PropertyCase = {
  name: string;
  color: boolean;
  preserveCase: boolean;
  verbatim: boolean;
  zeroLength: boolean;
};

test('css/values', () => {
  const corpus = [
    'color: red',
    'color: RED',
    'color: #FF0000',
    'color: #f00',
    'color: #FFFFFF00',
    'color: rgb(1,2,3)',
    'color: rgba(0,0,0,1)',
    'color: rgba(0,0,0,.5)',
    'color: rgb(255 0 0)',
    'color: rgb(255 0 0 / 50%)',
    'color: hsl(0, 100%, 50%)',
    'color: transparent',
    'color: currentColor',
    'background-color: Red',
    'background-color: #00000080',
    'background-color: rgb(255, 255, 0)',
    'font-size: 15px',
    'font-size: 0',
    'font-size: 1.50em',
    'font-size: 12.0px',
    'font-size: +5px',
    'font-size: 1e1px',
    'font-size: 10PX',
    'font-family: Arial',
    'font-family: Arial,Times',
    'font-family: Arial ,  "Times New Roman" , serif',
    "font-family: 'Courier New'",
    'font-weight: bold',
    'font-weight: BOLD',
    'font-weight: 700',
    'font-style: italic',
    'white-space: pre-wrap',
    'white-space: PRE-WRAP',
    'text-transform: uppercase',
    'text-align: center',
    'text-decoration: UNDERLINE',
    'text-decoration: underline line-through',
    'vertical-align: SUB',
    'vertical-align: super',
    'line-height: 1.5',
    'line-height: 0',
    'opacity: 0',
    'z-index: 0',
    'margin: 0',
    'margin: 0 !important',
    'margin: 0 !IMPORTANT',
    'margin: -0.50px .5em',
    'padding-left: 0',
    'padding-inline-start: 40px',
    'padding-inline-start: calc(2 * 40px)',
    'text-indent: 0',
    'text-indent: 20px',
    'letter-spacing: 0',
    'width: 50%',
    'border: 1px solid #f00',
    'border:1px   solid red',
    'text-shadow: 1px 1px 2px #000',
    'unknown-prop: 1',
    'COLOR: red',
    '--custom: example',
    '--Custom-Case:  Value ',
    '-webkit-text-stroke: 1px',
    'color: red; font-weight: bold',
    'color:red;;font-weight :bold',
    'color: red; color: blue',
    'font-size: 15px; color: rgb(0, 0, 0); background-color: #fff',
    'background-image: url("data:image/svg+xml;utf8,<svg></svg>")',
    'content: "semi;colon:value"',
    '',
  ];
  const cases: Case[] = corpus.map(css => {
    const element = doc.createElement('span');
    element.style.cssText = css;
    return {css, cssText: element.style.cssText};
  });
  emitVectors(
    'css/values',
    'css-values',
    'Inline CSS text and the `cssText` the declaration serializes it to.',
    cases,
  );
});

type Case = {
  css: string;
  cssText: string;
};

test('css/declarations', () => {
  // How the declaration expands the `font` shorthand into its longhands and
  // serializes them back: each case sets `css` as `cssText`, then calls
  // `setProperty` with each of `set`, and records the property list (what
  // `$inlineStylesFromStyleSheets` copies and the import rules read) and
  // `cssText`. jsdom's own rules, quirks included (a numeric weight is also
  // read as the line height; `small-caps` is refused).
  const corpus: [string, ...[string, string, string?][]][] = [
    ['font: bold 12px serif'],
    ['font: italic bold 12px/30px Georgia, serif'],
    ['font: 12px Arial'],
    ['font: normal 12px Arial'],
    ['font: Bold Italic 12px A'],
    ['font: BOLD 12PX SERIF'],
    ['font: 700 1.2em "Times New Roman", Times'],
    ['font: 400 normal 12px a'],
    ['font: 1000 12px a'],
    ['font: 1001 12px a'],
    ['font: oblique 10px sans-serif'],
    ['font: oblique 10deg 12px a'],
    ['font: bolder 10px a'],
    ['font: condensed bold 12px serif'],
    ['font: ultra-condensed 12px a'],
    ['font: italic small-caps bold 16px/2 cursive'],
    ['font: small-caps 12px a'],
    ['font: italic italic 12px a'],
    ['font: bold bold 12px a'],
    ['font: foo 12px a'],
    ['font: normal normal normal normal 12px a'],
    ['font: normal normal normal normal normal 12px a'],
    ['font: inherit'],
    ['font: INHERIT'],
    ['font: caption'],
    ['font: bold'],
    ['font: 12px'],
    ['font: italic 12px/30px'],
    ['font: 0 a'],
    ['font: -1px a'],
    ['font: 1e1px a'],
    ['font: italic 0.5em a'],
    ['font: 150% a'],
    ['font: xx-small a'],
    ['font: SMALLER a'],
    ['font: calc(2 * 6px) a'],
    ['font: calc(10px + 2px) a'],
    ['font: 12px/normal a'],
    ['font: 12px/1.50 a'],
    ['font: 12px/-1 a'],
    ['font: 12px/ 30px a'],
    ['font: 12px /30px a'],
    ['font: bold 2em/1.5 a'],
    ['font: 12px/2 "a", b'],
    ['font: 12px/calc(1 + 1) a'],
    ['font: italic 1px a,b,c'],
    ['font: 12px a, "b c" , d'],
    ["font: 12px 'a b'"],
    ['font: 12px a b'],
    ['font: 12px Times New Roman'],
    ['font: 12px bold'],
    ['font: 12px inherit'],
    ['font: 12px default'],
    ['font: 12px serif sans-serif'],
    ["font: 12px 'it''s'"],
    ['font: 12px a,'],
    ['font: 12px , a'],
    ['font: var(--x)'],
    ['font: bold 12px serif; font-weight: normal'],
    ['font-weight: normal; font: bold 12px serif'],
    ['font: bold 12px serif !important; font-weight: normal'],
    ['font-weight: bold !important; font: italic 12px serif'],
    ['font: 12px a; font-weight: bold !important'],
    ['font: 12px a !important; font: 13px b'],
    ['font: bold 12px serif; font-family: Arial'],
    ['font: 1px a; font: 2px b'],
    ['font: 12px a; FONT-WEIGHT: bold'],
    ['color: red; font: bold 12px serif; color: blue'],
    ['color: red !important; color: blue'],
    ['font: bold 12px serif', ['font-weight', 'normal']],
    ['font: bold 12px serif', ['font-weight', '']],
    ['font: bold 12px serif', ['font', 'italic 10px a']],
    ['font: 12px a', ['font', '']],
    ['font: 12px a', ['font', '12px b', 'important']],
    ['font: 12px a', ['font-weight', 'bold', 'important']],
    ['font: 12px a !important', ['font-weight', 'bold']],
    ['font-weight: bold', ['font', 'var(--x)']],
    ['font: var(--x)', ['font-weight', 'bold']],
    [
      '',
      ['font-style', 'italic'],
      ['font-weight', 'bold'],
      ['font-size', '12px'],
      ['font-family', 'serif'],
      ['font', 'bold 12px serif'],
    ],
  ];
  const cases: DeclarationCase[] = corpus.map(([css, ...set]) => {
    const element = doc.createElement('span');
    element.style.cssText = css;
    for (const [name, value, priority = ''] of set) {
      element.style.setProperty(name, value, priority);
    }
    const {style} = element;
    return {
      css,
      cssText: style.cssText,
      properties: Array.from(style, name => [
        name,
        style.getPropertyValue(name),
        style.getPropertyPriority(name),
      ]),
      set: set.map(([name, value, priority = '']) => [name, value, priority]),
    };
  });
  emitVectors(
    'css/declarations',
    'css-declarations',
    'Inline CSS text set as `cssText`, then each `[name, value, priority]` of `set` passed to `setProperty`, and the declaration that results in jsdom: its properties in order as `[name, value, priority]` (the `font` shorthand with the longhands it expands into) and its `cssText`.',
    cases,
  );
});

type DeclarationCase = {
  css: string;
  set: string[][];
  cssText: string;
  properties: string[][];
};

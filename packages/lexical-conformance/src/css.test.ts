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

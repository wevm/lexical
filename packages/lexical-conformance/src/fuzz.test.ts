/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {
  $getRoot,
  $isElementNode,
  $isParagraphNode,
  $isTextNode,
  type LexicalNode,
  type SerializedLexicalNode,
} from 'lexical';
import {test} from 'vitest';

import {type Action, type ScriptResult, startSession, type Step} from './ops';
import {emitVectors} from './vectors';

// Seeded random editor sessions. Each step is chosen from the live JS state
// (the keys and sizes of the nodes it has), then run and recorded like a
// script vector, so the Swift runner replays them with Ops.swift. Seeds are
// fixed, so the vectors only change when JS behavior does.

/** mulberry32: a small, well-known 32-bit PRNG. */
function prng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

type Random = {
  int(n: number): number;
  pick<T>(items: readonly T[]): T;
  chance(p: number): boolean;
};

function random(seed: number): Random {
  const next = prng(seed);
  return {
    chance: p => next() < p,
    int: n => Math.floor(next() * n),
    pick: items => items[Math.floor(next() * items.length)],
  };
}

const PIECES = ['a', 'b', 'xy', ' ', '#', 'é', '😀', 'ab c', 'Z'];
const FORMATS = ['bold', 'italic', 'underline', 'code', 'strikethrough'];
const ALIGN = ['', 'left', 'center', 'right', 'justify'];

function word(r: Random): string {
  let s = '';
  const n = 1 + r.int(3);
  for (let i = 0; i < n; i++) {
    s += r.pick(PIECES);
  }
  return s;
}

function textNode(r: Random): SerializedLexicalNode {
  return {
    detail: 0,
    format: r.chance(0.3) ? 1 << r.int(4) : 0,
    mode: 'normal',
    style: '',
    text: word(r),
    type: 'text',
    version: 1,
  } as SerializedLexicalNode;
}

function inline(r: Random): SerializedLexicalNode {
  const roll = r.int(10);
  if (roll === 0) {
    return {type: 'linebreak', version: 1};
  }
  if (roll === 1) {
    return {...textNode(r), text: '\t', type: 'tab'} as SerializedLexicalNode;
  }
  return textNode(r);
}

function paragraph(r: Random): SerializedLexicalNode {
  const children = [];
  const n = r.int(4);
  for (let i = 0; i < n; i++) {
    children.push(inline(r));
  }
  return {
    children,
    direction: null,
    format: '',
    indent: 0,
    textFormat: 0,
    textStyle: '',
    type: 'paragraph',
    version: 1,
  } as SerializedLexicalNode;
}

type Doc = {
  texts: {key: string; size: number; text: string}[];
  elements: {key: string; size: number}[];
  paragraphs: string[];
};

function $doc(): Doc {
  const doc: Doc = {elements: [], paragraphs: [], texts: []};
  const visit = (node: LexicalNode) => {
    if ($isTextNode(node)) {
      doc.texts.push({
        key: node.getKey(),
        size: node.getTextContentSize(),
        text: node.getTextContent(),
      });
    } else if ($isElementNode(node)) {
      doc.elements.push({key: node.getKey(), size: node.getChildrenSize()});
      if ($isParagraphNode(node)) {
        doc.paragraphs.push(node.getKey());
      }
      node.getChildren().forEach(visit);
    }
  };
  visit($getRoot());
  return doc;
}

/**
 * A random offset in `text` that doesn't split a surrogate pair: a Swift
 * String can't hold the lone surrogate JS would leave (see swift/DESIGN.md,
 * unpaired surrogates).
 */
function offset(r: Random, text: string): number {
  const i = r.int(text.length + 1);
  const code = text.charCodeAt(i - 1);
  return i > 0 && i < text.length && code >= 0xd800 && code < 0xdc00
    ? i - 1
    : i;
}

function point(r: Random, doc: Doc): [string, number, 'text' | 'element'] {
  if (doc.texts.length > 0 && r.chance(0.8)) {
    const t = r.pick(doc.texts);
    return [t.key, offset(r, t.text), 'text'];
  }
  const e = r.pick(doc.elements);
  return [e.key, r.int(e.size + 1), 'element'];
}

function select(r: Random, doc: Doc): Action {
  const anchor = point(r, doc);
  return {
    anchor,
    do: 'select',
    focus: r.chance(0.4) ? anchor : point(r, doc),
  };
}

function call(method: string, ...args: unknown[]): Action {
  return {args: args as never, do: 'selection', method};
}

/** One random action that fits `doc`. */
function action(r: Random, doc: Doc): Action[] {
  const roll = r.int(20);
  if (roll < 3) {
    return [select(r, doc), call('insertText', word(r))];
  }
  if (roll < 4) {
    return [select(r, doc), call('insertRawText', `${word(r)}\n${word(r)}\t`)];
  }
  if (roll < 6) {
    return [select(r, doc), call('removeText')];
  }
  if (roll < 7) {
    return [select(r, doc), call('insertParagraph')];
  }
  if (roll < 8) {
    return [select(r, doc), call('insertLineBreak', r.chance(0.5))];
  }
  if (roll < 10) {
    return [select(r, doc), call('formatText', r.pick(FORMATS))];
  }
  if (roll < 11) {
    return [select(r, doc), call('insertNodes', [paragraph(r), paragraph(r)])];
  }
  if (roll < 12) {
    return [select(r, doc), call('insertNodes', [textNode(r), inline(r)])];
  }
  if (roll < 13) {
    return [{do: 'selectAll'}, call('insertText', word(r))];
  }
  if (roll < 14) {
    return [{do: 'append', key: 'root', nodes: [paragraph(r)]}];
  }
  if (doc.texts.length > 0 && roll < 18) {
    const t = r.pick(doc.texts);
    const sub = r.int(5);
    const args: unknown[] =
      sub === 0
        ? ['setTextContent', word(r)]
        : sub === 1
          ? ['toggleFormat', r.pick(FORMATS)]
          : sub === 2
            ? ['splitText', offset(r, t.text)]
            : sub === 3
              ? ['remove']
              : ['insertAfter', textNode(r)];
    return [
      {
        args: args.slice(1) as never,
        do: 'node',
        key: t.key,
        method: args[0] as string,
      },
    ];
  }
  if (doc.paragraphs.length > 0) {
    const key = r.pick(doc.paragraphs);
    return r.chance(0.5)
      ? [{args: [r.int(3)], do: 'node', key, method: 'setIndent'}]
      : r.chance(0.5)
        ? [{args: [r.pick(ALIGN)], do: 'node', key, method: 'setFormat'}]
        : [{args: [], do: 'node', key, method: 'remove'}];
  }
  return [{do: 'append', key: 'root', nodes: [paragraph(r)]}];
}

function fuzzScript(seed: number, steps: number): ScriptResult {
  const r = random(seed);
  const header = {name: `seed ${seed}`};
  const session = startSession(header);
  const doc: Step = {
    update: [
      {
        do: 'append',
        key: 'root',
        nodes: [paragraph(r), paragraph(r), paragraph(r)],
      },
    ],
  };
  const script = {...header, steps: [doc] as Step[]};
  const results = [session.run(doc, true)];
  while (script.steps.length < steps) {
    const current = session.editor.read(() => $doc());
    const step: Step = {update: action(r, current)};
    script.steps.push(step);
    results.push(session.run(step, true));
  }
  return {...script, results};
}

test('fuzz/core', () => {
  const cases: ScriptResult[] = [];
  for (let seed = 1; seed <= Number(process.env.FUZZ_SEEDS ?? 40); seed++) {
    cases.push(fuzzScript(seed, Number(process.env.FUZZ_STEPS ?? 10)));
  }
  emitVectors(
    'fuzz/core',
    'script',
    'Seeded random sessions of selection editing and node mutation over the core nodes, recorded like scripts.',
    cases,
  );
});

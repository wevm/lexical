/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import * as fc from 'fast-check';
import {test} from 'vitest';

import {
  type AbstractSequence,
  documentArbitrary,
  type DocumentCase,
  runDocument,
  runSequence,
  withJSDOM,
} from './property';
import {sequenceArbitrary} from './property';
import {
  DOCUMENT_REGRESSIONS,
  SEQUENCE_REGRESSIONS,
} from './propertyRegressions';
import {emitVectors} from './vectors';

// Property-based vectors (swift/DESIGN.md, Conformance tooling): inputs
// drawn by @lexical/fast-check from the nodes' `$config` schemas with a fixed
// seed, then the minimized counterexamples the nightly run has found
// (propertyRegressions.ts), each with what upstream JS makes of it.

const SEED = 20261105;

export function documentCases(seed: number, runs: number): DocumentCase[] {
  const inputs = [
    ...fc.sample(documentArbitrary(false), {numRuns: runs, seed}),
    ...DOCUMENT_REGRESSIONS,
  ];
  return withJSDOM(() =>
    inputs.map((document, i) => {
      const input = JSON.stringify(document);
      const name = i < runs ? `seed ${seed} #${i}` : `regression ${i - runs}`;
      return {input, name, ...runDocument(input)};
    }),
  );
}

export function sequenceCases(seed: number, runs: number) {
  const inputs: AbstractSequence[] = [
    ...fc.sample(sequenceArbitrary(), {numRuns: runs, seed}),
    ...SEQUENCE_REGRESSIONS,
  ];
  return inputs.map((sequence, i) =>
    runSequence(
      i < runs ? `seed ${seed} #${i}` : `regression ${i - runs}`,
      sequence,
    ),
  );
}

test('property/documents', () => {
  emitVectors(
    'property/documents',
    'property-document',
    'Random editor states drawn by @lexical/fast-check from the $config schemas of the in-scope nodes, and what JS makes of each: both JSON forms, text content, markdown and HTML export.',
    documentCases(SEED, Number(process.env.PROPERTY_RUNS ?? 120)),
  );
});

test('property/sequences', () => {
  emitVectors(
    'property/sequences',
    'property-sequence',
    'Random command sequences (text, deletion, Enter, formats, indent, lists, links, block types, undo and redo) on a rich-text editor, each from a selection in a random document, with the state after every step.',
    sequenceCases(SEED, Number(process.env.PROPERTY_RUNS ?? 80)),
  );
});

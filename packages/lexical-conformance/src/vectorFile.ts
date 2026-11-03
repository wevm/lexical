/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

// The vector file format, kept free of test-runner imports so that the
// WebKit recorder's config (run outside a test) can write vector files too.

import {fileURLToPath} from 'node:url';

/**
 * Bumped when the shape of every vector file changes. Each file also carries
 * its own `kind`, so readers can reject vectors they don't understand.
 */
export const VECTOR_FORMAT_VERSION = 1;

export const vectorsDir = fileURLToPath(
  new URL('../vectors/', import.meta.url),
);

export const updateVectors = process.env.UPDATE_VECTORS === '1';

export type VectorFile<T> = {
  formatVersion: number;
  kind: string;
  description: string;
  cases: T[];
};

/** The text of a vector file, with one case per line. */
export function vectorFileText<T>(
  kind: string,
  description: string,
  cases: T[],
): string {
  const file: VectorFile<T> = {
    cases,
    description,
    formatVersion: VECTOR_FORMAT_VERSION,
    kind,
  };
  // One case per line keeps diffs of large files readable.
  const {cases: _, ...head} = file;
  return (
    `${JSON.stringify(head).slice(0, -1)},"cases":[\n` +
    cases.map(c => JSON.stringify(c)).join(',\n') +
    '\n]}\n'
  );
}

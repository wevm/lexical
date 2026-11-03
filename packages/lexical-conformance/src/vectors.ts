/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {readFileSync, writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {expect} from 'vitest';

/**
 * Bumped when the shape of every vector file changes. Each file also carries
 * its own `kind`, so readers can reject vectors they don't understand.
 */
export const VECTOR_FORMAT_VERSION = 1;

const vectorsDir = fileURLToPath(new URL('../vectors/', import.meta.url));

export const updateVectors = process.env.UPDATE_VECTORS === '1';

export type VectorFile<T> = {
  formatVersion: number;
  kind: string;
  description: string;
  cases: T[];
};

/**
 * Writes `vectors/<name>.json` when `UPDATE_VECTORS=1`, otherwise checks that
 * the checked-in file matches what the JS implementation produces now. A
 * mismatch means the JS behavior changed and the Swift port must follow
 * (regenerate with `pnpm run generate`).
 */
export function emitVectors<T>(
  name: string,
  kind: string,
  description: string,
  cases: T[],
): void {
  const file: VectorFile<T> = {
    cases,
    description,
    formatVersion: VECTOR_FORMAT_VERSION,
    kind,
  };
  // One case per line keeps diffs of large files readable.
  const {cases: _, ...head} = file;
  const text =
    `${JSON.stringify(head).slice(0, -1)},"cases":[\n` +
    cases.map(c => JSON.stringify(c)).join(',\n') +
    '\n]}\n';
  const path = vectorsDir + name + '.json';
  if (updateVectors) {
    writeFileSync(path, text);
    return;
  }
  let existing: string;
  try {
    existing = readFileSync(path, 'utf8');
  } catch {
    throw new Error(
      `Missing vectors/${name}.json. Run \`pnpm run generate\` in packages/lexical-conformance.`,
    );
  }
  expect(
    existing,
    `vectors/${name}.json is stale. Run \`pnpm run generate\` in packages/lexical-conformance.`,
  ).toBe(text);
}

/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {readFileSync, writeFileSync} from 'node:fs';
import {expect} from 'vitest';

import {updateVectors, vectorFileText, vectorsDir} from './vectorFile';

export {
  updateVectors,
  VECTOR_FORMAT_VERSION,
  type VectorFile,
} from './vectorFile';

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
  const text = vectorFileText(kind, description, cases);
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

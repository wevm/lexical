/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {test} from 'vitest';

import {emitVectors} from './vectors';

// The upstream release the vectors were generated from. The Swift port checks
// it against swift/UPSTREAM_VERSION, so a sync that regenerates the vectors
// can't leave the declared conformance target behind.
test('meta', () => {
  const {version} = JSON.parse(
    readFileSync(
      fileURLToPath(new URL('../../lexical/package.json', import.meta.url)),
      'utf8',
    ),
  );
  emitVectors(
    'meta',
    'meta',
    'The upstream Lexical release these vectors come from.',
    [{lexicalVersion: version}],
  );
});

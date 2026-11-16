/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {Plugin} from 'vite';

import {defineConfig} from 'vitest/config';

/**
 * Module-private functions of the pinned upstream sources that the vectors
 * record, by the end of the module's path. Each is re-exported unchanged as
 * `INTERNAL_<name>` (see `src/ops.ts`'s `ReactInternals`), so the vectors run
 * the real function bodies; nothing else in those modules changes.
 */
const INTERNAL_EXPORTS: [string, string][] = [
  [
    '/lexical-react/src/shared/LexicalMenu.tsx',
    'export {getFullMatchOffset as INTERNAL_getFullMatchOffset, $splitNodeContainingQuery as INTERNAL_$splitNodeContainingQuery};',
  ],
  [
    '/lexical-react/src/LexicalTypeaheadMenuPlugin.tsx',
    "export {getTextUpToAnchor as INTERNAL_getTextUpToAnchor, isSelectionOnEntityBoundary as INTERNAL_isSelectionOnEntityBoundary}; export {INTERNAL_getFullMatchOffset, INTERNAL_$splitNodeContainingQuery} from './shared/LexicalMenu';",
  ],
];

function internalExports(): Plugin {
  return {
    enforce: 'pre',
    name: 'lexical-conformance-internal-exports',
    transform(code, id) {
      const entry = INTERNAL_EXPORTS.find(([end]) =>
        id.split('?')[0].endsWith(end),
      );
      return entry === undefined ? null : `${code}\n${entry[1]}\n`;
    },
  };
}

// Vectors are JS's answers, computed from the TypeScript sources of the
// pinned upstream packages (their `source` export condition), in Node.
export default defineConfig({
  plugins: [internalExports()],
  resolve: {conditions: ['source']},
  ssr: {resolve: {conditions: ['source']}},
  test: {
    environment: 'node',
    // Recorded in WebKit by vitest.webkit.config.mts.
    exclude: ['src/__tests__/webkit/**'],
    include: ['src/**/*.test.ts'],
    name: 'lexical-conformance',
  },
});

/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {Plugin} from 'vite';

import {fileURLToPath} from 'node:url';
import {defineConfig} from 'vitest/config';

/**
 * Module-private functions of the pinned upstream sources that the vectors
 * record, by the end of the module's path. Each is re-exported unchanged as
 * `INTERNAL_<name>` (see `src/ops.ts`'s `ReactInternals` and
 * `CharacterLimitInternals`), so the vectors run the real function bodies;
 * nothing else in those modules changes.
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
  [
    '/lexical-react/src/LexicalCharacterLimitPlugin.tsx',
    "export {useCharacterLimit as INTERNAL_useCharacterLimit, findOffset as INTERNAL_findOffset, $wrapOverflowedNodes as INTERNAL_$wrapOverflowedNodes, $mergePrevious as INTERNAL_$mergePrevious} from './shared/useCharacterLimit';",
  ],
  ['/lexical-react/src/shared/useCharacterLimit.ts', 'export {findOffset};'],
];

/**
 * Upstream React hooks whose effects the vectors run without React: in these
 * modules `react` and `@lexical/react/LexicalComposerContext` resolve to
 * `src/reactHost.ts`, which runs each effect at once (see `mountHooks`).
 */
const HOOK_MODULES = [
  '/lexical-react/src/shared/useCharacterLimit.ts',
  '/lexical-react/src/LexicalTableOfContentsPlugin.tsx',
];
const REACT_HOST = fileURLToPath(
  new URL('./src/reactHost.ts', import.meta.url),
);

function endsWithAny(id: string, ends: string[]): string | undefined {
  const path = id.split('?')[0];
  return ends.find(end => path.endsWith(end));
}

function internalExports(): Plugin {
  return {
    enforce: 'pre',
    name: 'lexical-conformance-internal-exports',
    resolveId(source, importer) {
      if (
        importer !== undefined &&
        endsWithAny(importer, HOOK_MODULES) !== undefined &&
        (source === 'react' ||
          source === '@lexical/react/LexicalComposerContext')
      ) {
        return REACT_HOST;
      }
      return null;
    },
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

/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {BrowserCommandContext} from 'vitest/node';

import {playwright} from '@vitest/browser-playwright';
import {readFileSync, writeFileSync} from 'node:fs';
import {defineConfig} from 'vitest/config';

import {updateVectors, vectorFileText, vectorsDir} from './src/vectorFile.ts';

// Tier 2 scenarios (src/scenarios.ts) are recorded by
// src/__tests__/webkit/scenarios.test.ts in WebKit, whose
// navigator.platform is a Mac's, from the TypeScript sources of the pinned
// upstream packages. Run with `pnpm run scenarios` (`generate-scenarios` to
// write the vectors).
export default defineConfig({
  test: {
    projects: [
      {
        resolve: {
          conditions: ['source', 'browser', 'import', 'module', 'default'],
        },
        test: {
          browser: {
            commands: {
              /**
               * Writes `vectors/<name>.json` with UPDATE_VECTORS=1 and returns
               * `null`; otherwise returns the checked-in text and the text the
               * recording would write, for the test to compare.
               */
              emitScenarioVectors(
                _context: BrowserCommandContext,
                name: string,
                description: string,
                cases: unknown[],
              ): {expected: string; actual: string} | null {
                const text = vectorFileText('scenarios', description, cases);
                const path = `${vectorsDir}${name}.json`;
                if (updateVectors) {
                  writeFileSync(path, text);
                  return null;
                }
                let existing = '';
                try {
                  existing = readFileSync(path, 'utf8');
                } catch {
                  // Not recorded yet: compare with nothing.
                }
                return {actual: text, expected: existing};
              },

              /** `page.keyboard.press(chord)`. */
              async keyboardPress(
                {page}: BrowserCommandContext,
                chord: string,
              ) {
                await page.keyboard.press(chord);
              },

              /** `page.keyboard.type(text)`. */
              async keyboardType({page}: BrowserCommandContext, text: string) {
                await page.keyboard.type(text);
              },
            },
            enabled: true,
            headless: true,
            instances: [{browser: 'webkit'}],
            provider: playwright(),
          },
          include: ['src/__tests__/webkit/**/*.test.ts'],
          name: 'lexical-conformance-webkit',
          setupFiles: ['./src/__tests__/webkit/setup.ts'],
        },
      },
    ],
  },
});

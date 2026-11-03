/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {defineConfig} from 'vitest/config';

// Vectors are JS's answers, computed from the TypeScript sources of the
// pinned upstream packages (their `source` export condition), in Node.
export default defineConfig({
  resolve: {conditions: ['source']},
  ssr: {resolve: {conditions: ['source']}},
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
    name: 'lexical-conformance',
  },
});

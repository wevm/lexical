/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

import {readFileSync} from 'node:fs';
import {describe, expect, test} from 'vitest';

import {type Script, startSession, type StepResult} from './ops';

// Replays a session another port recorded, such as one exported from a
// desktop playground in rust/apps, and checks that JS Lexical produces the
// same result at every step:
//
//   cd packages/lexical-conformance
//   LEXICAL_REPLAY=/path/to/session.json pnpm exec vitest run src/replay.test.ts
//
// The file is a `scripts/*` vector file (`{kind: 'script', cases}`) or one
// case. Without LEXICAL_REPLAY the test is skipped.

type Case = Script & {results: StepResult[]};

const path = process.env.LEXICAL_REPLAY;

function cases(): Case[] {
  const file = JSON.parse(readFileSync(path as string, 'utf8')) as
    | Case
    | {cases: Case[]};
  return 'cases' in file ? file.cases : [file];
}

describe.skipIf(path === undefined)('replay', () => {
  for (const c of path === undefined ? [] : cases()) {
    test(c.name, () => {
      const session = startSession(c);
      c.steps.forEach((step, i) => {
        const expected = c.results[i];
        const actual = session.run(step, expected.state !== undefined);
        expect(
          {...actual, step: i},
          `step ${i}: ${JSON.stringify(step)}`,
        ).toEqual({...expected, step: i});
      });
    });
  }
});

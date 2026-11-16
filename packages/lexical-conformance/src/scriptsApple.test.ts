/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {IS_APPLE} from 'lexical';
import {expect, test, vi} from 'vitest';

import {eventScripts} from './eventScripts';
import {runScript} from './ops';
import {emitVectors} from './vectors';

// `vi.mock` is hoisted above the imports, so `lexical` compiles its keyboard
// conventions (`CONTROL_OR_META`, `CONTROL_OR_ALT`, the keydown table) for
// Apple platforms, as upstream's LexicalEventsKeyDownApple.test.ts does.
vi.mock('../../lexical/src/environment.ts', async importOriginal => ({
  ...(await importOriginal<object>()),
  IS_APPLE: true,
}));

test('scripts/events-apple', () => {
  expect(IS_APPLE).toBe(true);
  emitVectors(
    'scripts/events-apple',
    'script',
    'The core handler of KEY_DOWN_COMMAND on Apple platforms: which command each key dispatches, with which payload, and what rich text then does.',
    eventScripts(true).map(runScript),
  );
});

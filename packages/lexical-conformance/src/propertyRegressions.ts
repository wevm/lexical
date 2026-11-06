/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

import type {AbstractSequence} from './property';

// Minimized counterexamples from the nightly property run
// (src/propertyNightly.test.ts), kept so the checked-in vectors replay them
// on every change. Paste the input the run prints at the end of the matching
// list, then `pnpm run generate`. JS computes the expected values as for any
// other case.

/** Serialized editor states for vectors/property/documents.json. */
export const DOCUMENT_REGRESSIONS: {[key: string]: unknown}[] = [
  // A text style ending in a backslash: jsdom's CSS preprocessing makes it
  // U+FFFD in the item's marker custom properties.
  {
    root: {
      children: [
        {
          children: [
            {children: [], textStyle: '):\\', type: 'listitem', version: 1},
          ],
          type: 'list',
          version: 1,
        },
      ],
      type: 'root',
      version: 1,
    },
  },
];

/** Command sequences for vectors/property/sequences.json. */
export const SEQUENCE_REGRESSIONS: AbstractSequence[] = [
  // Enter in an empty code-highlight node trapped in Swift.
  {
    document: {
      root: {
        children: [
          {
            children: [{type: 'code-highlight', version: 1}],
            type: 'code',
            version: 1,
          },
        ],
        type: 'root',
        version: 1,
      },
    },
    steps: [
      {
        anchor: {element: false, i: 0, o: 0},
        command: 'INSERT_PARAGRAPH_COMMAND',
        focus: null,
      },
    ],
  },
];

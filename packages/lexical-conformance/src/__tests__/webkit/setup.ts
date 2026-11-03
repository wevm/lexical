/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

// Scenarios are recorded with Apple platform flags. Playwright's WebKit on
// Linux reports a Mac user agent but the host's navigator.platform, from
// which Lexical computes IS_APPLE when it loads, after this file.
Object.defineProperty(Navigator.prototype, 'platform', {
  configurable: true,
  get: () => 'MacIntel',
});

/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {LexicalEditor} from 'lexical';

// What `react` and `@lexical/react/LexicalComposerContext` are to the
// upstream hooks in vitest.config.mts's HOOK_MODULES
// (`useCharacterLimit`, `TableOfContentsPlugin`): a host that runs a hook's
// body once, as a first render does, with each effect run at once and its
// clean-up kept, so the vectors record the real effect bodies without React's
// scheduling. The Swift and Rust ports have the same logic as registrations
// (`registerCharacterLimit`, `registerTableOfContents`).

type Host = {
  editor: LexicalEditor;
  cleanups: (() => void)[];
  /** What a `useState` setter is called with, in order. */
  onSetState: (value: unknown) => void;
};

let host: Host | null = null;

function currentHost(): Host {
  if (host === null) {
    throw new Error('a hook ran outside mountHooks');
  }
  return host;
}

/** `useEffect(effect, deps)`: runs `effect` now and keeps its clean-up. */
export function useEffect(effect: () => void | (() => void)): void {
  const h = currentHost();
  const cleanup = effect();
  if (typeof cleanup === 'function') {
    h.cleanups.push(cleanup);
  }
}

/** `useState(initial)`: the initial value, and a setter that reports. */
export function useState<T>(initial: T): [T, (value: T) => void] {
  const h = currentHost();
  return [initial, value => h.onSetState(value)];
}

/** `useLexicalComposerContext()`: the host's editor. */
export function useLexicalComposerContext(): [LexicalEditor, unknown] {
  return [currentHost().editor, null];
}

/**
 * Runs `hooks` (a hook or a component's body) as a first render, with its
 * effects, and returns the clean-up that unmounting runs: each effect's, in
 * order.
 */
export function mountHooks(
  editor: LexicalEditor,
  onSetState: (value: unknown) => void,
  hooks: () => unknown,
): () => void {
  const prev = host;
  const h: Host = {cleanups: [], editor, onSetState};
  host = h;
  try {
    hooks();
  } finally {
    host = prev;
  }
  return () => {
    for (const cleanup of h.cleanups) {
      cleanup();
    }
  };
}

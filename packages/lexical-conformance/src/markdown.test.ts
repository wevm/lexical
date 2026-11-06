/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {JSDOM} from 'jsdom';

import {$createCodeNode, CodeNode} from '@lexical/code-core';
import {createHeadlessEditor} from '@lexical/headless';
import {$generateHtmlFromNodes, $generateNodesFromDOM} from '@lexical/html';
import {$createLinkNode, LinkNode} from '@lexical/link';
import {ListItemNode, ListNode} from '@lexical/list';
import {
  $convertFromMarkdownString,
  $convertToMarkdownString,
  CHECK_LIST,
  CODE,
  LINK,
  type MultilineElementTransformer,
  type TextMatchTransformer,
  type Transformer,
  TRANSFORMERS,
} from '@lexical/markdown';
import {HeadingNode, QuoteNode} from '@lexical/rich-text';
import {$createTextNode, $getRoot, $insertNodes} from 'lexical';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {expect, test} from 'vitest';

import {browserWindow} from './browserWindow';
import {emitVectors} from './vectors';

const UPSTREAM_FILE =
  'lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts';
const packagesDir = fileURLToPath(new URL('../../', import.meta.url));

// The custom transformers below are copied from
// packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts, which
// does not export them. Vectors name them; the Swift side ports each one.

const HIGHLIGHT_TEXT_MATCH_IMPORT: TextMatchTransformer = {
  ...LINK,
  importRegExp: /\$([^$]+?)\$/,
  replace: (textNode, match) => {
    textNode.toggleFormat('highlight');
  },
};

const SIMPLE_INLINE_JSX_MATCHER: TextMatchTransformer = {
  dependencies: [LinkNode],
  getEndIndex(node, match) {
    // Find the closing tag. Count the number of opening and closing tags to find the correct closing tag.
    // For simplicity, this will only count the opening and closing tags without checking for "MyTag" specifically.
    let openedSubStartMatches = 0;
    const start = (match.index ?? 0) + match[0].length;
    let endIndex = start;
    const line = node.getTextContent();

    for (let i = start; i < line.length; i++) {
      const char = line[i];
      if (char === '<') {
        const nextChar = line[i + 1];
        if (nextChar === '/') {
          if (openedSubStartMatches === 0) {
            endIndex = i + '</MyTag>'.length;
            break;
          }
          openedSubStartMatches--;
        } else {
          openedSubStartMatches++;
        }
      }
    }
    return endIndex;
  },
  importRegExp: /<(MyTag)\s*>/,
  regExp: /__ignore__/,
  replace: (textNode, match) => {
    const linkNode = $createLinkNode('simple-jsx');

    const textStart = match[0].length + (match.index ?? 0);
    const textEnd =
      (match.index ?? 0) + textNode.getTextContent().length - '</MyTag>'.length;
    // eslint-disable-next-line @lexical/internal/no-optional-chaining -- as upstream
    const text = match.input?.slice(textStart, textEnd);

    const linkTextNode = $createTextNode(text);
    linkTextNode.setFormat(textNode.getFormat());
    linkNode.append(linkTextNode);
    textNode.replace(linkNode);
  },
  type: 'text-match',
};

// Matches html within a mdx file
const MDX_HTML_TRANSFORMER: MultilineElementTransformer = {
  dependencies: [CodeNode],
  export: node => {
    if (node.getTextContent().startsWith('From HTML:')) {
      return `<MyComponent>${node
        .getTextContent()
        .replace('From HTML: ', '')}</MyComponent>`;
    }
    return null; // Run next transformer
  },
  regExpEnd: /<\/(\w+)\s*>/,
  regExpStart: /<(\w+)[^>]*>/,
  replace: (rootNode, children, startMatch, endMatch, linesInBetween) => {
    if (!linesInBetween) {
      return false; // Run next transformer. We don't need to support markdown shortcuts for this test
    }
    if (startMatch[1] === 'MyComponent') {
      const codeBlockNode = $createCodeNode(startMatch[1]);
      const textNode = $createTextNode(
        'From HTML: ' + linesInBetween.join('\n'),
      );
      codeBlockNode.append(textNode);
      rootNode.append(codeBlockNode);
      return;
    }
    return false; // Run next transformer
  },
  type: 'multiline-element',
};

const CODE_TAG_COUNTER_EXAMPLE: MultilineElementTransformer = {
  dependencies: CODE.dependencies,
  export: CODE.export,
  handleImportAfterStartMatch({lines, rootNode, startLineIndex, startMatch}) {
    const regexpEndRegex: RegExp | undefined = /[ \t]*```$/;

    const isEndOptional = false;

    let endLineIndex = startLineIndex;
    const linesLength = lines.length;

    let openedSubStartMatches = 0;

    // check every single line for the closing match. It could also be on the same line as the opening match.
    while (endLineIndex < linesLength) {
      const potentialSubStartMatch =
        lines[endLineIndex].match(/^[ \t]*```(\w+)?/);

      const endMatch = regexpEndRegex
        ? lines[endLineIndex].match(regexpEndRegex)
        : null;

      if (potentialSubStartMatch) {
        if (endMatch) {
          if ((potentialSubStartMatch.index ?? 0) < (endMatch.index ?? 0)) {
            openedSubStartMatches++;
          }
        } else {
          openedSubStartMatches++;
        }
      }

      if (endMatch) {
        openedSubStartMatches--;
      }

      if (!endMatch || openedSubStartMatches > 0) {
        if (
          !isEndOptional ||
          (isEndOptional && endLineIndex < linesLength - 1) // Optional end, but didn't reach the end of the document yet => continue searching for potential closing match
        ) {
          endLineIndex++;
          continue; // Search next line for closing match
        }
      }

      // Now, check if the closing match matched is the same as the opening match.
      // If it is, we need to continue searching for the actual closing match.
      if (
        endMatch &&
        startLineIndex === endLineIndex &&
        endMatch.index === startMatch.index
      ) {
        endLineIndex++;
        continue; // Search next line for closing match
      }

      // At this point, we have found the closing match. Next: calculate the lines in between open and closing match
      // This should not include the matches themselves, and be split up by lines
      const linesInBetween: string[] = [];

      if (endMatch && startLineIndex === endLineIndex) {
        linesInBetween.push(
          lines[startLineIndex].slice(
            startMatch[0].length,
            -endMatch[0].length,
          ),
        );
      } else {
        for (let i = startLineIndex; i <= endLineIndex; i++) {
          if (i === startLineIndex) {
            const text = lines[i].slice(startMatch[0].length);
            linesInBetween.push(text); // Also include empty text
          } else if (i === endLineIndex && endMatch) {
            const text = lines[i].slice(0, -endMatch[0].length);
            linesInBetween.push(text); // Also include empty text
          } else {
            linesInBetween.push(lines[i]);
          }
        }
      }

      if (
        CODE.replace(
          rootNode,
          null,
          startMatch,
          endMatch,
          linesInBetween,
          true,
        ) !== false
      ) {
        // Return here. This $importMultiline function is run line by line and should only process a single multiline element at a time.
        return [true, endLineIndex];
      }

      // The replace function returned false, despite finding the matching open and close tags => this transformer does not want to handle it.
      // Thus, we continue letting the remaining transformers handle the passed lines of text from the beginning
      break;
    }

    // No multiline transformer handled this line successfully
    return [false, startLineIndex];
  },
  regExpStart: CODE.regExpStart,
  replace: CODE.replace,
  type: 'multiline-element',
};

/** The identifiers the upstream table may reference, by name. */
const SCOPE: Record<string, unknown> = {
  CHECK_LIST,
  CODE_TAG_COUNTER_EXAMPLE,
  HIGHLIGHT_TEXT_MATCH_IMPORT,
  MDX_HTML_TRANSFORMER,
  SIMPLE_INLINE_JSX_MATCHER,
  URL: 'https://lexical.dev',
};

type Entry = {
  html: string;
  md: string;
  skipExport?: true;
  skipImport?: true;
  shouldPreserveNewLines?: true;
  shouldMergeAdjacentLines?: boolean;
  customTransformers?: Transformer[];
  mdAfterExport?: string;
};

/**
 * Reads the `IMPORT_AND_EXPORT` table out of the upstream test file and
 * evaluates it, so an upstream change makes the vectors stale.
 */
function readUpstreamTable(): Entry[] {
  const text = readFileSync(packagesDir + UPSTREAM_FILE, 'utf8');
  const head = '\n  const IMPORT_AND_EXPORT: Input = [\n';
  const start = text.indexOf(head);
  const end = text.indexOf('\n  ];\n', start);
  if (start < 0 || end < 0 || text.indexOf(head, start + 1) >= 0) {
    throw new Error(`Cannot find IMPORT_AND_EXPORT in ${UPSTREAM_FILE}`);
  }
  const literal = text.slice(start + head.length - 2, end + 4);
  // Every entry opens on its own line at the table's indentation.
  const expected = (literal.match(/^ {4}\{$/gm) || []).length;
  // eslint-disable-next-line no-new-func
  const table = new Function(...Object.keys(SCOPE), `return ${literal};`)(
    ...Object.values(SCOPE),
  ) as Entry[];
  if (!Array.isArray(table) || table.length !== expected || expected === 0) {
    throw new Error(
      `IMPORT_AND_EXPORT in ${UPSTREAM_FILE} has ${expected} entries, evaluated ${table.length}`,
    );
  }
  return table;
}

type Case = {
  /** Upstream's test title: `md` with newlines shown as `\n`. */
  name: string;
  md: string;
  /** The table's HTML: what import must render, and the export's input. */
  html: string;
  skipImport?: true;
  skipExport?: true;
  shouldPreserveNewLines?: true;
  shouldMergeAdjacentLines?: boolean;
  mdAfterExport?: string;
  /** Names of the custom transformers, placed before `TRANSFORMERS`. */
  customTransformers?: string[];
  /** `JSON.stringify(editorState.toJSON())` after `$convertFromMarkdownString`. */
  importState?: string;
  /** `$generateHtmlFromNodes(editor)` after the import. */
  importHtml?: string;
  /** `JSON.stringify(editorState.toJSON())` after inserting the parsed `html`. */
  exportState?: string;
  /** `$convertToMarkdownString` of that state. */
  exportMd?: string;
  importId?: string;
  exportId?: string;
  noSelectionId?: string;
};

/** Runs `f` with a jsdom window as the global `window` and `document`. */
function withJSDOM<T>(f: (dom: JSDOM['window']) => T): T {
  const dom = browserWindow();
  const globals = globalThis as {window?: unknown; document?: unknown};
  const prev = [globals.window, globals.document];
  globals.window = dom;
  globals.document = dom.document;
  try {
    return f(dom);
  } finally {
    [globals.window, globals.document] = prev;
    dom.close();
  }
}

function createTestEditor() {
  return createHeadlessEditor({
    nodes: [HeadingNode, ListNode, ListItemNode, QuoteNode, CodeNode, LinkNode],
    onError(error) {
      throw error;
    },
  });
}

/** The ledger's id of each upstream test, numbering repeated titles. */
function ledgerIds(): (title: string) => string {
  const upstream = JSON.parse(
    readFileSync(packagesDir + '../swift/parity/upstream-tests.json', 'utf8'),
  ) as {packages: Record<string, string[]>};
  const known = new Set(upstream.packages['lexical-markdown'] ?? []);
  const seen = new Map<string, number>();
  return title => {
    const count = (seen.get(title) ?? 0) + 1;
    seen.set(title, count);
    const id = `${UPSTREAM_FILE}::Markdown > ${title}${count > 1 ? ` #${count}` : ''}`;
    expect.soft(known.has(id), `ledger id ${id}`).toBe(true);
    return id;
  };
}

// The upstream `IMPORT_AND_EXPORT` table and what JS produces for each entry
// in its three loops: import, export, and "should not select when importing".
test('markdown/import-export', () => {
  const names = new Map(Object.entries(SCOPE).map(([k, v]) => [v, k]));
  const id = ledgerIds();
  const entries = readUpstreamTable();
  const cases: Case[] = entries.map(entry => {
    const {html, md, customTransformers} = entry;
    const name = md.replace(/\n/g, '\\n');
    const c: Case = {html, md, name};
    for (const flag of [
      'skipImport',
      'skipExport',
      'shouldPreserveNewLines',
      'shouldMergeAdjacentLines',
      'mdAfterExport',
    ] as const) {
      if (flag in entry) {
        (c as Record<string, unknown>)[flag] = entry[flag];
      }
    }
    if (customTransformers) {
      c.customTransformers = customTransformers.map(t => {
        const n = names.get(t);
        if (n === undefined) {
          throw new Error(`Unknown custom transformer in "${name}"`);
        }
        return n;
      });
    }
    return c;
  });
  // Ids in the order the upstream loops declare them.
  cases.forEach(c => {
    if (!c.skipImport) {
      c.importId = id(`can import "${c.name}"`);
    }
  });
  cases.forEach(c => {
    if (!c.skipExport) {
      c.exportId = id(`can export "${c.name}"`);
    }
  });
  cases.forEach(c => {
    if (!c.skipImport) {
      c.noSelectionId = id(`should not select when importing "${c.name}"`);
    }
  });
  withJSDOM(dom => {
    entries.forEach((entry, i) => {
      const c = cases[i];
      const custom = entry.customTransformers || [];
      if (!entry.skipImport) {
        const editor = createTestEditor();
        editor.update(
          () =>
            $convertFromMarkdownString(
              entry.md,
              [...custom, ...TRANSFORMERS, HIGHLIGHT_TEXT_MATCH_IMPORT],
              undefined,
              entry.shouldPreserveNewLines,
              entry.shouldMergeAdjacentLines,
            ),
          {discrete: true},
        );
        c.importState = JSON.stringify(editor.getEditorState().toJSON());
        c.importHtml = editor.read('latest', () =>
          $generateHtmlFromNodes(editor),
        );
        expect.soft(c.importHtml, `import "${c.name}"`).toBe(entry.html);
      }
      if (!entry.skipExport) {
        const editor = createTestEditor();
        editor.update(
          () => {
            const parser = new dom.DOMParser();
            const doc = parser.parseFromString(entry.html, 'text/html');
            const nodes = $generateNodesFromDOM(editor, doc);
            $getRoot().select();
            $insertNodes(nodes);
          },
          {discrete: true},
        );
        c.exportState = JSON.stringify(editor.getEditorState().toJSON());
        c.exportMd = editor.read('latest', () =>
          $convertToMarkdownString(
            [...custom, ...TRANSFORMERS],
            undefined,
            entry.shouldPreserveNewLines,
          ),
        );
        expect
          .soft(c.exportMd, `export "${c.name}"`)
          .toBe(entry.mdAfterExport ?? entry.md);
      }
    });
  });
  emitVectors(
    'markdown/import-export',
    'markdown',
    "Upstream LexicalMarkdown.test.ts's IMPORT_AND_EXPORT table, with the editor state and HTML JS imports from each md and the state and md JS exports from each html.",
    cases,
  );
});

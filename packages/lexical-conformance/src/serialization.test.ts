/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {
  $createCodeHighlightNode,
  $createCodeNode,
  CodeHighlightNode,
  CodeNode,
} from '@lexical/code-core';
import {createHeadlessEditor} from '@lexical/headless';
import {
  $createAutoLinkNode,
  $createLinkNode,
  AutoLinkNode,
  LinkNode,
} from '@lexical/link';
import {
  $createListItemNode,
  $createListNode,
  ListItemNode,
  ListNode,
} from '@lexical/list';
import {$addMarkID, $createMarkNode, MarkNode} from '@lexical/mark';
import {$createOverflowNode, OverflowNode} from '@lexical/overflow';
import {
  $createLineBreakNode,
  $createParagraphNode,
  $createRangeSelection,
  $createTabNode,
  $createTextNode,
  $getRoot,
  $setState,
  createState,
} from 'lexical';
import {test} from 'vitest';

import {emitVectors} from './vectors';

type Case = {
  name: string;
  /** `JSON.stringify(editorState.toJSON())`: the legacy form, the default. */
  legacy: string;
  /** `JSON.stringify(editorState.toJSON(true))`: the compact form. */
  compact: string;
};

// Documents built from the core nodes alone, and the exact bytes JS writes for
// them in both forms. The Swift port must import each form and write the same
// bytes back.
test('serialization/core', () => {
  const documents: Record<string, () => void> = {
    'empty paragraph': () => {
      $getRoot().append($createParagraphNode());
    },
    'formatted text, line break, tab': () => {
      $getRoot().append(
        $createParagraphNode().append(
          $createTextNode('Hello ').toggleFormat('bold'),
          $createTextNode('world')
            .toggleFormat('italic')
            .setStyle('color: red'),
          $createLineBreakNode(),
          $createTabNode(),
          $createTextNode('UTF-16: 😀 é'),
        ),
      );
    },
    'indented, aligned paragraphs': () => {
      $getRoot().append(
        $createParagraphNode()
          .setIndent(2)
          .setFormat('center')
          .append($createTextNode('a')),
        $createParagraphNode()
          .setDirection('rtl')
          .append($createTextNode('שלום')),
      );
    },
    // NodeState under `$`, which an editor that has none of these configs
    // writes back as it came. `count` 0 is the default, which is left out.
    'node state': () => {
      const label = createState('label', {
        parse: v => (typeof v === 'string' ? v : ''),
      });
      const count = createState('count', {
        parse: v => (typeof v === 'number' ? v : 0),
      });
      $getRoot().append(
        $setState($createParagraphNode(), label, 'p').append(
          $setState($setState($createTextNode('x'), label, 'one'), count, 2),
          $setState($createTextNode('y'), count, 0),
        ),
      );
    },
  };
  const cases: Case[] = Object.entries(documents).map(([name, build]) => {
    const editor = createHeadlessEditor({
      onError: e => {
        throw e;
      },
    });
    editor.update(build, {discrete: true});
    const state = editor.getEditorState();
    return {
      compact: JSON.stringify(state.toJSON(true)),
      legacy: JSON.stringify(state.toJSON()),
      name,
    };
  });
  emitVectors(
    'serialization/core',
    'serialization',
    'Core-node documents and the exact JSON text JS writes for them, legacy and compact.',
    cases,
  );
});

// Documents with @lexical/list nodes: list types, start, checked items and
// nesting (a list item's indent is derived from it).
test('serialization/list', () => {
  const $item = (text: string, checked?: boolean) =>
    $createListItemNode(checked).append($createTextNode(text));
  const documents: Record<string, () => void> = {
    'aligned item, empty item': () => {
      $getRoot().append(
        $createListNode('bullet').append(
          $item('centered').setFormat('center'),
          $createListItemNode(),
        ),
        $createParagraphNode(),
      );
    },
    'bullet list': () => {
      $getRoot().append(
        $createListNode('bullet').append($item('a'), $item('b')),
      );
    },
    checklist: () => {
      $getRoot().append(
        $createListNode('check').append($item('done', true), $item('todo')),
      );
    },
    'nested list': () => {
      const nested = $createListItemNode().append(
        $createListNode('bullet').append($item('inner')),
      );
      $getRoot().append(
        $createListNode('number').append(
          $item('outer'),
          nested,
          $item('after'),
        ),
      );
    },
    'numbered list from 3': () => {
      $getRoot().append(
        $createListNode('number', 3).append($item('one'), $item('two')),
      );
    },
  };
  const cases: Case[] = Object.entries(documents).map(([name, build]) => {
    const editor = createHeadlessEditor({
      nodes: [ListNode, ListItemNode],
      onError: e => {
        throw e;
      },
    });
    editor.update(build, {discrete: true});
    const state = editor.getEditorState();
    return {
      compact: JSON.stringify(state.toJSON(true)),
      legacy: JSON.stringify(state.toJSON()),
      name,
    };
  });
  emitVectors(
    'serialization/list',
    'serialization',
    'Documents with @lexical/list nodes and the exact JSON text JS writes for them.',
    cases,
  );
});

// Documents with @lexical/link nodes: attributes set, empty and absent, and
// unlinked autolinks.
test('serialization/link', () => {
  const documents: Record<string, () => void> = {
    'autolink, unlinked autolink': () => {
      $getRoot().append(
        $createParagraphNode().append(
          $createAutoLinkNode('https://a.example').append(
            $createTextNode('a.example'),
          ),
          $createTextNode(' and '),
          $createAutoLinkNode('https://b.example', {isUnlinked: true}).append(
            $createTextNode('b.example'),
          ),
        ),
      );
    },
    'link with every attribute': () => {
      $getRoot().append(
        $createParagraphNode().append(
          $createLinkNode('https://example.com/', {
            rel: 'noopener noreferrer',
            target: '_blank',
            title: 'Example',
          }).append($createTextNode('example')),
        ),
      );
    },
    'link without attributes, formatted text': () => {
      $getRoot().append(
        $createParagraphNode().append(
          $createTextNode('see '),
          $createLinkNode('mailto:a@example.com').append(
            $createTextNode('mail').toggleFormat('bold'),
          ),
        ),
      );
    },
  };
  const cases: Case[] = Object.entries(documents).map(([name, build]) => {
    const editor = createHeadlessEditor({
      nodes: [LinkNode, AutoLinkNode],
      onError: e => {
        throw e;
      },
    });
    editor.update(build, {discrete: true});
    const state = editor.getEditorState();
    return {
      compact: JSON.stringify(state.toJSON(true)),
      legacy: JSON.stringify(state.toJSON()),
      name,
    };
  });
  emitVectors(
    'serialization/link',
    'serialization',
    'Documents with @lexical/link nodes and the exact JSON text JS writes for them.',
    cases,
  );
});

// Documents with @lexical/code-core nodes: languages, themes, highlight
// types, tabs and line breaks.
test('serialization/code', () => {
  const documents: Record<string, () => void> = {
    'code with language and highlights': () => {
      $getRoot().append(
        $createCodeNode('javascript').append(
          $createCodeHighlightNode('const', 'keyword'),
          $createCodeHighlightNode(' x = '),
          $createCodeHighlightNode('1', 'number'),
          $createLineBreakNode(),
          $createTabNode(),
          $createCodeHighlightNode('x'),
        ),
      );
    },
    'code with theme, no language': () => {
      $getRoot().append(
        $createCodeNode(null, 'dark').append($createCodeHighlightNode('a')),
        $createParagraphNode(),
      );
    },
    'empty code': () => {
      $getRoot().append($createCodeNode());
    },
  };
  const cases: Case[] = Object.entries(documents).map(([name, build]) => {
    const editor = createHeadlessEditor({
      nodes: [CodeNode, CodeHighlightNode],
      onError: e => {
        throw e;
      },
    });
    editor.update(build, {discrete: true});
    const state = editor.getEditorState();
    return {
      compact: JSON.stringify(state.toJSON(true)),
      legacy: JSON.stringify(state.toJSON()),
      name,
    };
  });
  emitVectors(
    'serialization/code',
    'serialization',
    'Documents with @lexical/code-core nodes and the exact JSON text JS writes for them.',
    cases,
  );
});

// Documents with @lexical/overflow's OverflowNode: the text past a character
// limit, alone and after formatted text.
test('serialization/overflow', () => {
  const documents: Record<string, () => void> = {
    'overflow after text': () => {
      $getRoot().append(
        $createParagraphNode().append(
          $createTextNode('within '),
          $createOverflowNode().append(
            $createTextNode('past').toggleFormat('italic'),
            $createTextNode(' the limit'),
          ),
        ),
      );
    },
    'overflow alone': () => {
      $getRoot().append(
        $createParagraphNode().append(
          $createOverflowNode().append($createTextNode('over')),
        ),
      );
    },
  };
  const cases: Case[] = Object.entries(documents).map(([name, build]) => {
    const editor = createHeadlessEditor({
      nodes: [OverflowNode],
      onError: e => {
        throw e;
      },
    });
    editor.update(build, {discrete: true});
    const state = editor.getEditorState();
    return {
      compact: JSON.stringify(state.toJSON(true)),
      legacy: JSON.stringify(state.toJSON()),
      name,
    };
  });
  emitVectors(
    'serialization/overflow',
    'serialization',
    'Documents with @lexical/overflow nodes and the exact JSON text JS writes for them.',
    cases,
  );
});

// Documents with @lexical/mark's MarkNode, and with the `markIds` NodeState
// that MarkStateExtension's $addMarkID stores on the leaves instead.
test('serialization/mark', () => {
  const documents: Record<string, () => void> = {
    'a mark with element properties': () => {
      $getRoot().append(
        $createParagraphNode().append(
          $createMarkNode(['x'])
            .setFormat('center')
            .setIndent(1)
            .setDirection('rtl')
            .setTextFormat(1)
            .setTextStyle('color: red')
            .append($createTextNode('b')),
        ),
      );
    },
    'a mark with no ids': () => {
      $getRoot().append(
        $createParagraphNode().append(
          $createMarkNode().append($createTextNode('none')),
        ),
      );
    },
    'mark ids as NodeState': () => {
      const text = $createTextNode('one two');
      $getRoot().append(
        $createParagraphNode().append(text, $createLineBreakNode()),
      );
      const selection = $createRangeSelection();
      selection.anchor.set(text.getKey(), 0, 'text');
      selection.focus.set(text.getKey(), 3, 'text');
      $addMarkID('c2', selection);
      $addMarkID('c1', selection);
    },
    'nested and overlapping marks': () => {
      $getRoot().append(
        $createParagraphNode().append(
          $createTextNode('a'),
          $createMarkNode(['x', 'y']).append(
            $createTextNode('b').toggleFormat('bold'),
            $createMarkNode(['z']).append($createTextNode('c')),
          ),
          $createTextNode('d'),
        ),
      );
    },
  };
  const cases: Case[] = Object.entries(documents).map(([name, build]) => {
    const editor = createHeadlessEditor({
      nodes: [MarkNode],
      onError: e => {
        throw e;
      },
    });
    editor.update(build, {discrete: true});
    const state = editor.getEditorState();
    return {
      compact: JSON.stringify(state.toJSON(true)),
      legacy: JSON.stringify(state.toJSON()),
      name,
    };
  });
  emitVectors(
    'serialization/mark',
    'serialization',
    'Documents with @lexical/mark nodes, and mark ids as NodeState, and the exact JSON text JS writes for them.',
    cases,
  );
});

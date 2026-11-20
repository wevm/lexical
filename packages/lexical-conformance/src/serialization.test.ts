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
  $createTableCellNode,
  $createTableNode,
  $createTableNodeWithDimensions,
  $createTableRowNode,
  TableCellHeaderStates,
  TableCellNode,
  TableNode,
  TableRowNode,
} from '@lexical/table';
import {
  $createLineBreakNode,
  $createParagraphNode,
  $createRangeSelection,
  $createTabNode,
  $createTextNode,
  $getRoot,
  $setSlot,
  $setState,
  aliasedValue,
  arraySetValue,
  arrayValue,
  booleanValue,
  createState,
  enumValue,
  nullable,
  numberValue,
  objectValue,
  optional,
  rawValue,
  type SerializationSchema,
  stringValue,
  transformValue,
  unionValue,
} from 'lexical';
import {expect, test} from 'vitest';

import {TestDecoratorBlockNode, TestSchemaNode} from './ops';
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
    // Named slots under `$slots`, after the children, in canonical order
    // (here, code unit order: no class declares them), with a slot of a slot
    // value; their values are any non-inline blocks.
    'named slots': () => {
      const host = $createParagraphNode().append($createTextNode('body'));
      $getRoot().append(host);
      const title = $createParagraphNode().append($createTextNode('title'));
      $setSlot(host, 'title', title);
      $setSlot(
        title,
        'inner',
        $createParagraphNode().append($createTextNode('inner')),
      );
      $setSlot(host, 'aside', $createParagraphNode());
      $setSlot(host, '2', $createParagraphNode().setIndent(1));
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

// Documents with @lexical/table nodes: header states, spans, widths, row
// heights, background colors, vertical alignment, striping and frozen rows
// and columns.
test('serialization/table', () => {
  const $cell = (text: string, headerState = TableCellHeaderStates.NO_STATUS) =>
    $createTableCellNode(headerState).append(
      $createParagraphNode().append($createTextNode(text)),
    );
  const documents: Record<string, () => void> = {
    'merged cells': () => {
      const merged = $cell('wide').setColSpan(2).setRowSpan(2);
      $getRoot().append(
        $createTableNode().append(
          $createTableRowNode().append(merged, $cell('c')),
          $createTableRowNode().append($cell('f')),
        ),
        $createParagraphNode(),
      );
    },
    'styled table': () => {
      const table = $createTableNode()
        .setColWidths([120, 80.5])
        .setRowStriping(true)
        .setFrozenColumns(1)
        .setFrozenRows(1);
      table.append(
        $createTableRowNode(40).append(
          $cell('a', TableCellHeaderStates.BOTH).setBackgroundColor('#eee'),
          $cell('b', TableCellHeaderStates.ROW)
            .setVerticalAlign('middle')
            .setWidth(80.5),
        ),
        $createTableRowNode().append(
          $cell('c', TableCellHeaderStates.COLUMN),
          $createTableCellNode(),
        ),
      );
      $getRoot().append(table, $createParagraphNode());
    },
    'table with dimensions': () => {
      $getRoot().append(
        $createTableNodeWithDimensions(2, 3),
        $createParagraphNode(),
      );
    },
    'table without headers': () => {
      $getRoot().append(
        $createTableNodeWithDimensions(2, 2, false),
        $createParagraphNode(),
      );
    },
  };
  const cases: Case[] = Object.entries(documents).map(([name, build]) => {
    const editor = createHeadlessEditor({
      nodes: [TableNode, TableRowNode, TableCellNode],
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
    'serialization/table',
    'serialization',
    'Documents with @lexical/table nodes and the exact JSON text JS writes for them.',
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

// Documents with @lexical/react's DecoratorBlockNode, through
// TestDecoratorBlockNode (ops.ts), which extends it with nothing: its
// `format` is written in the legacy form always and in the compact form
// unless it is ''.
test('serialization/decorator-block', () => {
  const block = (
    format?: Parameters<TestDecoratorBlockNode['setFormat']>[0],
  ) => {
    const node = new TestDecoratorBlockNode();
    return format === undefined ? node : node.setFormat(format);
  };
  const documents: Record<string, () => void> = {
    'a block without a format': () => {
      $getRoot().append(block());
    },
    'blocks with each format, between paragraphs': () => {
      $getRoot().append(
        $createParagraphNode().append($createTextNode('before')),
        block('left'),
        block('start'),
        block('center'),
        block('right'),
        block('end'),
        block('justify'),
        block(''),
        $createParagraphNode().append($createTextNode('after')),
      );
    },
  };
  const cases: Case[] = Object.entries(documents).map(([name, build]) => {
    const editor = createHeadlessEditor({
      nodes: [TestDecoratorBlockNode],
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
    'serialization/decorator-block',
    'serialization',
    "Documents with @lexical/react's DecoratorBlockNode (TestDecoratorBlockNode) and the exact JSON text JS writes for them.",
    cases,
  );
});

type AnySchema = SerializationSchema<unknown, unknown, unknown>;

/**
 * The schemas of serialization/schema, each named by the JS that builds it, so
 * a port builds the same one from the name.
 */
const SCHEMAS: Record<string, () => unknown> = {
  "aliasedValue(enumValue(['number', 'bullet']), {ol: 'number', ul: 'bullet'})":
    () =>
      aliasedValue(enumValue(['number', 'bullet']), {
        ol: 'number',
        ul: 'bullet',
      }),
  "arraySetValue(enumValue(['a', 'b']))": () =>
    arraySetValue(enumValue(['a', 'b'])),
  'arraySetValue(stringValue())': () => arraySetValue(stringValue()),
  'arrayValue(numberValue())': () => arrayValue(numberValue()),
  'booleanValue()': () => booleanValue(),
  'booleanValue(true)': () => booleanValue(true),
  "enumValue(['a', 'b', 1])": () => enumValue<'a' | 'b' | 1>(['a', 'b', 1]),
  "enumValue(['ltr', 'rtl', null], null)": () =>
    enumValue(['ltr', 'rtl', null], null),
  'nullable(objectValue({n: nullable(booleanValue())}))': () =>
    nullable(objectValue({n: nullable(booleanValue())})),
  "nullable(stringValue('x'))": () => nullable(stringValue('x')),
  "nullable(stringValue('x'), {defaultAsNull: true})": () =>
    nullable(stringValue('x'), {defaultAsNull: true}),
  'numberValue()': () => numberValue(),
  'numberValue(0, {clamp: true, max: 1, min: -1})': () =>
    numberValue(0, {clamp: true, max: 1, min: -1}),
  'numberValue(0, {integer: true})': () => numberValue(0, {integer: true}),
  'numberValue(1, {clamp: true, integer: true, max: 3.5, min: 0.5})': () =>
    numberValue(1, {clamp: true, integer: true, max: 3.5, min: 0.5}),
  'numberValue(5, {max: 10, min: 0})': () => numberValue(5, {max: 10, min: 0}),
  'objectValue({a: numberValue(), b: optional(stringValue())})': () =>
    objectValue({a: numberValue(), b: optional(stringValue())}),
  'optional(numberValue(1))': () => optional(numberValue(1)),
  'optional(numberValue(1), {omitDefault: true})': () =>
    optional(numberValue(1), {omitDefault: true}),
  'rawValue()': () => rawValue(),
  "stringValue('x')": () => stringValue('x'),
  'stringValue()': () => stringValue(),
  "transformValue(stringValue(), s => s + '!')": () =>
    transformValue(stringValue(), s => s + '!'),
  "unionValue([booleanValue(), stringValue('d')], 'd')": () =>
    unionValue([booleanValue(), stringValue('d')], 'd'),
  'unionValue([numberValue(), rawValue()])': () =>
    unionValue([numberValue(), rawValue()]),
  'unionValue([numberValue(), stringValue()])': () =>
    unionValue([numberValue(), stringValue()]),
};

/** Declarations each builder refuses, with JS's message. */
const REFUSED_SCHEMAS: Record<string, () => unknown> = {
  'arraySetValue(numberValue())': () => arraySetValue(numberValue() as never),
  'arrayValue(arraySetValue(stringValue()))': () =>
    arrayValue(arraySetValue(stringValue())),
  "enumValue(['a'], 'b')": () => enumValue(['a'], 'b' as never),
  'enumValue([])': () => enumValue([] as never),
  'numberValue(0, {integer: true, max: 1.8, min: 1.2})': () =>
    numberValue(0, {integer: true, max: 1.8, min: 1.2}),
  'numberValue(0, {max: 1, min: 5})': () => numberValue(0, {max: 1, min: 5}),
  'objectValue({toString: numberValue()})': () =>
    objectValue({toString: numberValue()}),
  'unionValue([])': () => unionValue([] as never),
};

/** The inputs each schema of serialization/schema parses; `undefined` first. */
const SCHEMA_INPUTS: unknown[] = [
  undefined,
  null,
  true,
  false,
  0,
  1,
  2.5,
  -3,
  11,
  1e300,
  '',
  'a',
  'b',
  'x',
  'd',
  '1',
  '2.5',
  ' 1',
  '0x10',
  '1e999',
  'ol',
  'ul',
  'bullet',
  'rtl',
  'toString',
  '__proto__',
  [],
  ['b', 'a', 'b'],
  ['\u00e9', 'e', '\u{1F600}', '\uffff', 'a'],
  [1, 'a', null],
  {},
  {a: 2, b: 'c'},
  {a: 'no', b: 1},
  {n: true},
];

// Each schema builder of LexicalSchema.ts parsing a table of inputs: invalid
// values falling back to the default, clamping and integer bounds, aliases,
// unions (with a catch-all member), nullable and optional values, objects,
// and sets in canonical (code unit) order without duplicates. A case is one
// schema: its default, and per input the parsed value (`JSON.stringify`, left
// out for `undefined`) and whether it is the schema's default
// (`isSchemaDefault`); or, for a declaration the builder refuses, its message.
test('serialization/schema', () => {
  type Result = {input?: unknown; parsed?: string; isDefault: boolean};
  type SchemaCase =
    | {schema: string; defaultValue?: string; results: Result[]}
    | {schema: string; error: string};
  const cases: SchemaCase[] = [
    ...Object.entries(SCHEMAS),
    ...Object.entries(REFUSED_SCHEMAS),
  ].map(([name, build]) => {
    let schema: AnySchema;
    try {
      schema = build() as AnySchema;
    } catch (e) {
      return {error: (e as Error).message, schema: name};
    }
    // `isSchemaDefault` (@internal in lexical): identity, or the schema's
    // own `isEqual`.
    const isDefault = (value: unknown) =>
      value === schema.defaultValue ||
      (schema.isEqual !== undefined &&
        schema.isEqual(value, schema.defaultValue));
    return {
      defaultValue: JSON.stringify(schema.defaultValue),
      results: SCHEMA_INPUTS.map(input => {
        const parsed = schema(input);
        return {
          input,
          isDefault: isDefault(parsed),
          parsed: JSON.stringify(parsed),
        };
      }),
      schema: name,
    };
  });
  emitVectors(
    'serialization/schema',
    'schema',
    'Each LexicalSchema.ts value builder parsing a table of inputs (the parsed value and isSchemaDefault), or the message of a declaration it refuses.',
    cases,
  );
});

// TestSchemaNode (ops.ts), whose `$config` declares a `json` schema, imported
// from `input` (inside a root) and exported in both forms; a port imports
// `input` and each form and writes the same bytes, and so does a clone of
// the node (`getWritable` in a later update), which carries every field.
test('serialization/schema-node', () => {
  const base = {
    children: [],
    direction: null,
    format: '',
    indent: 0,
    type: 'test-schema',
    version: 1,
  };
  const inputs: Record<string, Record<string, unknown>> = {
    'a fractional count, a negative width, a non-string tag': {
      count: 2.5,
      tags: [1],
      width: -3,
    },
    'a numeric string, null and empty values': {
      count: '7',
      note: '',
      shown: 'hidden at count 1',
      tags: [],
      width: null,
    },
    'aliases, clamping, an array kept in order': {
      count: 99,
      kind: 'beta',
      tags: ['\u00e9', 'e', '\u{1F600}', '\uffff', 'e'],
    },
    defaults: {},
    'every property set': {
      $: {flags: ['y', 'x', 'y']},
      count: 5,
      kind: 'b',
      note: 'n',
      shown: 's',
      tags: ['b', 'a', 'b'],
      width: 3,
    },
    'invalid values': {
      $: {flags: 'nope'},
      count: 'x',
      kind: 'zzz',
      label: 'ignored',
      note: 7,
      shown: 1,
      tags: 'nope',
      width: 'big',
    },
  };
  const cases = Object.entries(inputs).map(([name, json]) => {
    const editor = createHeadlessEditor({
      nodes: [TestSchemaNode],
      onError: e => {
        throw e;
      },
    });
    const input = JSON.stringify({
      root: {
        children: [{...base, ...json}],
        direction: null,
        format: '',
        indent: 0,
        type: 'root',
        version: 1,
      },
    });
    const state = editor.parseEditorState(input);
    const legacy = JSON.stringify(state.toJSON());
    const compact = JSON.stringify(state.toJSON(true));
    for (const form of [legacy, compact]) {
      const reparsed = editor.parseEditorState(form);
      expect(JSON.stringify(reparsed.toJSON())).toBe(legacy);
      expect(JSON.stringify(reparsed.toJSON(true))).toBe(compact);
    }
    editor.setEditorState(state);
    editor.update(() => $getRoot().getFirstChildOrThrow().getWritable(), {
      discrete: true,
    });
    expect(JSON.stringify(editor.getEditorState().toJSON())).toBe(legacy);
    return {compact, input, legacy, name};
  });
  emitVectors(
    'serialization/schema-node',
    'serialization-input',
    'A node whose $config declares a json schema (TestSchemaNode), imported from `input` and the exact JSON text JS writes for it, legacy and compact.',
    cases,
  );
});

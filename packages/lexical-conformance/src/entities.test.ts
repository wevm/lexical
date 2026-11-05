/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {buildEditorFromExtensions} from '@lexical/extension';
import {HashtagExtension} from '@lexical/hashtag';
import {
  $createParagraphNode,
  $createTextNode,
  $getRoot,
  $getSelection,
  $isRangeSelection,
} from 'lexical';
import {test} from 'vitest';

import {emitVectors} from './vectors';

// @lexical/hashtag's default matcher over text in many scripts, and the
// documents its transforms (registerLexicalTextEntity from @lexical/text)
// make of text that is set at once or typed a character at a time.

const TEXTS = [
  '',
  '#',
  '#a',
  '#1',
  '#123',
  '#1a',
  '#_',
  '#a_1',
  'a#b',
  'a #b',
  '&#b',
  '/#b',
  ' #tag.',
  '#tag#other',
  '#tag #other',
  '#Lexical',
  '#ÄÖÜ',
  '#café',
  '#naïve',
  '#e\u0301',
  '#\u0301e',
  '#×',
  '#a×b',
  '#÷',
  '#привет мир',
  '#שלום',
  '#مرحبا',
  '#ไทย',
  '#한국어',
  '#日本語',
  '#カタカナ',
  '#ｶﾀｶﾅ',
  '＃full',
  '#ＡＢＣ',
  '#１２３',
  '#１２３ａ',
  '#\u2c00',
  '#\ua700',
  '#ß',
  '#ſ',
  '#\u0130',
  '#\u212a',
  '#ﬀ',
  '#😀',
  '#a😀b',
  '#\u{20000}',
  'x\u{1f600}#y',
  '#\u200c',
  '#a\u200cb',
  '##a',
  '# a',
  'one #two three #four',
  '#\u00d7\u00f7a',
  '#ǅ',
  '#ᾈ',
];

test('entities/hashtag-match', () => {
  const getMatch = HashtagExtension.config!.getHashtagMatch;
  emitVectors(
    'entities/hashtag-match',
    'entities',
    "The match @lexical/hashtag's default getHashtagMatch finds in each text (UTF-16 offsets).",
    TEXTS.map(text => ({match: getMatch(text), text})),
  );
});

type Piece = {text: string; format?: number};

const DOCUMENTS: Record<string, Piece[]> = {
  'a tag after formatted text': [{format: 1, text: 'bold '}, {text: '#tag'}],
  'a tag in the middle': [{text: 'say #hello world'}],
  'a tag split over two formats': [{text: '#ta'}, {format: 2, text: 'g end'}],
  'adjacent tags': [{text: '#one#two'}],
  'several tags': [{text: '#a b #c d #e'}],
  'tags in scripts': [{text: '#привет #日本 #한국 ＃ｆｕｌｌ'}],
  'text without tags': [{text: 'no tags here & / #'}],
};

const TYPED = ['#tag', 'hello #world.', 'a#b #c', '#abc def', '#1 #1a', '#é#e'];

function build() {
  return buildEditorFromExtensions({
    dependencies: [HashtagExtension],
    name: 'conformance-hashtag',
    onError: (e: Error) => {
      throw e;
    },
  });
}

test('entities/hashtag-documents', () => {
  const cases: {name: string; pieces: Piece[]; json: string}[] = [];
  for (const [name, pieces] of Object.entries(DOCUMENTS)) {
    using editor = build();
    editor.update(
      () => {
        $getRoot()
          .clear()
          .append(
            $createParagraphNode().append(
              ...pieces.map(piece =>
                $createTextNode(piece.text).setFormat(piece.format ?? 0),
              ),
            ),
          );
      },
      {discrete: true},
    );
    cases.push({
      json: JSON.stringify(editor.getEditorState().toJSON()),
      name,
      pieces,
    });
  }
  emitVectors(
    'entities/hashtag-documents',
    'entities',
    'Paragraphs of text pieces (with formats) set at once in an editor with HashtagExtension, and the JSON the hashtag transforms leave.',
    cases,
  );
});

test('entities/hashtag-typing', () => {
  const cases: {text: string; json: string; selection: number[]}[] = [];
  for (const text of TYPED) {
    using editor = build();
    editor.update(
      () => {
        $getRoot().clear().append($createParagraphNode()).selectEnd();
      },
      {discrete: true},
    );
    for (const ch of text) {
      editor.update(
        () => {
          const selection = $getSelection();
          if ($isRangeSelection(selection)) {
            selection.insertText(ch);
          }
        },
        {discrete: true},
      );
    }
    const selection = editor.read(() => {
      const s = $getSelection();
      if (!$isRangeSelection(s)) {
        return [];
      }
      const root = $getRoot();
      const texts = root.getAllTextNodes();
      const index = (key: string) => texts.findIndex(n => n.getKey() === key);
      return [
        index(s.anchor.key),
        s.anchor.offset,
        index(s.focus.key),
        s.focus.offset,
      ];
    });
    cases.push({
      json: JSON.stringify(editor.getEditorState().toJSON()),
      selection,
      text,
    });
  }
  emitVectors(
    'entities/hashtag-typing',
    'entities',
    'Text typed a character at a time at the end of an empty paragraph, in an editor with HashtagExtension: the JSON left and the selection (as the index of its text node among all text nodes, and offset, for anchor then focus).',
    cases,
  );
});

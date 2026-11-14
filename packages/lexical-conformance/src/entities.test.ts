/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {buildEditorFromExtensions} from '@lexical/extension';
import {HashtagExtension} from '@lexical/hashtag';
import {autoLinkEmailMatcher, autoLinkUrlMatcher} from '@lexical/link';
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
  // Letters only the `i` flag's case folding adds to the classes.
  '#\u037b',
  '#\u13f8',
  '#\u1c80',
  '#\u1c90',
  '#1\u2c62',
  '\u037b#a',
  // Surrogate pairs, which the regular expression (no `u`) sees as two code
  // units each.
  'a\u{1f600}#b',
  '#\u{1d400}',
  '#a\u{1d400}',
  '\u{1d400}#a',
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

// @lexical/link's autoLinkUrlMatcher (a `u` regex over code points, with
// \p{L} and \p{N}, and parenthesis balancing) and autoLinkEmailMatcher (a
// regex without `u`, over UTF-16 code units), on text in many scripts and at
// the bounds of their repetitions.
const LINK_TEXTS = [
  '',
  'lexical.dev',
  'www.lexical.dev',
  'https://lexical.dev',
  'http://www.lexical.dev/docs?x=1&y=two#top',
  'see https://lexical.dev/docs, then',
  'https://en.wikipedia.org/wiki/Fish_(disambiguation)',
  '(see https://en.wikipedia.org/wiki/Fish_(disambiguation))',
  '(https://lexical.dev)',
  '((https://lexical.dev/a_(b)))',
  'https://lexical.dev/(a',
  'https://lexical.dev).',
  'https://lexical.dev/a_(b)_(c)',
  'https://例え.jp/パス',
  'https://ru.wikipedia.org/wiki/Москва',
  'https://x.com/٣٤',
  'https://٣٤.com',
  'https://x.com/１２３',
  'www.ŝ.com/ß',
  'https://𝐀.com/𝐀',
  'https://' + '𝐀'.repeat(200) + '.com',
  'https://lexical.dev😀',
  '😀https://lexical.dev 😀',
  'https://' + 'a'.repeat(256) + '.com',
  'https://' + 'a'.repeat(257) + '.com',
  'https://example.abcdef',
  'https://example.abcdefg',
  'https://a.b',
  'https://lexical.dev\u0085x',
  'x\ufeffwww.lexical.dev',
  'user@example.com',
  'mailto:user@example.com',
  'user@sub.example.co.uk',
  'a.b@c.de',
  'a..b@c.de',
  'user@exämple.de',
  'üser@example.de',
  '"quoted name"@example.com',
  '"a\u2028b"@example.com',
  '"a\u0085b"@example.com',
  'user@[192.168.0.1]',
  'x@y',
  'a@b.' + 'c'.repeat(64),
  'a'.repeat(64) + '@x.com',
  'a'.repeat(65) + '@x.com',
  '😀'.repeat(32) + '@x.com',
  '😀'.repeat(33) + '@x.com',
  'a' + '😀'.repeat(32) + '@x.com',
  'a\ufeffb@x.com',
  'a\u0085b@x.com',
  'a\u00a0b@x.com',
  'see www.lexical.dev or user@example.com',
];

test('entities/autolink-match', () => {
  emitVectors(
    'entities/autolink-match',
    'entities',
    "What @lexical/link's autoLinkUrlMatcher and autoLinkEmailMatcher return for each text (index and length in UTF-16 code units).",
    LINK_TEXTS.map(text => ({
      email: autoLinkEmailMatcher(text),
      text,
      url: autoLinkUrlMatcher(text),
    })),
  );
});

/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
// Generates rust/lexical-ui/src/overflow/grapheme_tables.rs, the
// properties UAX #29's extended grapheme cluster rules read, for the Rust
// stand-in of the `Intl.Segmenter` that @lexical/react's character limit
// (`findOffset` in shared/useCharacterLimit.ts) segments with.
//
// JS is the reference, so the Unicode version is the one this Node's ICU
// implements (`process.versions.unicode`). The script reads that version's
// GraphemeBreakProperty.txt, emoji-data.txt (Extended_Pictographic),
// DerivedCoreProperties.txt (Indic_Conjunct_Break) and GraphemeBreakTest.txt
// from unicode.org, or from a directory given with `--ucd <dir>`. Before it
// writes anything it checks that `Intl.Segmenter` splits every line of
// GraphemeBreakTest.txt as the file says, so the tables and the rules they
// feed agree with JS; the test lines go into the file too, for the Rust
// unit test that checks the rules against them.
//
// Run from the repository root:
//   node rust/scripts/generate-grapheme-tables.mjs [--ucd <dir>]
import {readFileSync, writeFileSync} from 'node:fs';

const OUT = 'rust/lexical-ui/src/overflow/grapheme_tables.rs';
const version = `${process.versions.unicode}.0`
  .split('.')
  .slice(0, 3)
  .join('.');
const ucdIndex = process.argv.indexOf('--ucd');
const ucdDir = ucdIndex < 0 ? null : process.argv[ucdIndex + 1];

const FILES = {
  break: 'auxiliary/GraphemeBreakProperty.txt',
  derived: 'DerivedCoreProperties.txt',
  emoji: 'emoji/emoji-data.txt',
  test: 'auxiliary/GraphemeBreakTest.txt',
};

async function load(path) {
  if (ucdDir !== null) {
    return readFileSync(`${ucdDir}/${path.split('/').pop()}`, 'utf8');
  }
  const url = `https://www.unicode.org/Public/${version}/ucd/${path}`;
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`${url}: ${response.status}`);
  }
  return response.text();
}

const [breakText, derivedText, emojiText, testText] = await Promise.all(
  [FILES.break, FILES.derived, FILES.emoji, FILES.test].map(load),
);
for (const [name, text] of [
  [FILES.break, breakText],
  [FILES.derived, derivedText],
  [FILES.emoji, emojiText],
  [FILES.test, testText],
]) {
  // Each file names its version in its first lines (emoji-data.txt as
  // "Version: 17.0").
  const head = text.slice(0, 400);
  if (
    !head.includes(`-${version}.txt`) &&
    !head.includes(`Version: ${process.versions.unicode}\n`)
  ) {
    throw new Error(`${name} is not Unicode ${version}`);
  }
}

// The Grapheme_Cluster_Break values, in the order of the Rust enum `Gcb`.
const GCB = [
  'Other',
  'CR',
  'LF',
  'Control',
  'Extend',
  'ZWJ',
  'Regional_Indicator',
  'Prepend',
  'SpacingMark',
  'L',
  'V',
  'T',
  'LV',
  'LVT',
];
// Indic_Conjunct_Break, in the order of the Rust enum `InCB`.
const INCB = ['None', 'Consonant', 'Extend', 'Linker'];
const EXT_PICT = 1 << 4;
const INCB_SHIFT = 5;

const props = new Uint8Array(0x110000);

/** Each `lo..hi ; value` line of a UCD file, with `#` comments dropped. */
function* entries(text) {
  for (const line of text.split('\n')) {
    const data = line.split('#')[0].trim();
    if (data === '') {
      continue;
    }
    const fields = data.split(';').map(f => f.trim());
    const [lo, hi = lo] = fields[0].split('..').map(h => parseInt(h, 16));
    yield [lo, hi, fields.slice(1)];
  }
}

for (const [lo, hi, [value]] of entries(breakText)) {
  const gcb = GCB.indexOf(value);
  if (gcb < 0) {
    throw new Error(`unknown Grapheme_Cluster_Break ${value}`);
  }
  for (let c = lo; c <= hi; c++) {
    props[c] = (props[c] & ~0xf) | gcb;
  }
}
for (const [lo, hi, [property]] of entries(emojiText)) {
  if (property === 'Extended_Pictographic') {
    for (let c = lo; c <= hi; c++) {
      props[c] |= EXT_PICT;
    }
  }
}
for (const [lo, hi, [property, value]] of entries(derivedText)) {
  if (property === 'InCB') {
    const incb = INCB.indexOf(value);
    if (incb < 0) {
      throw new Error(`unknown Indic_Conjunct_Break ${value}`);
    }
    for (let c = lo; c <= hi; c++) {
      props[c] |= incb << INCB_SHIFT;
    }
  }
}

// GraphemeBreakTest.txt, as JS splits it: every line must match.
const tests = [];
// eslint-disable-next-line compat/compat -- Node has it; JS's findOffset uses it.
const segmenter = new Intl.Segmenter();
for (const line of testText.split('\n')) {
  const data = line.split('#')[0].trim();
  if (data === '') {
    continue;
  }
  const tokens = data.split(/\s+/);
  let text = '';
  const breaks = [];
  for (const token of tokens) {
    if (token === '÷') {
      breaks.push(text.length);
    } else if (token !== '×') {
      text += String.fromCodePoint(parseInt(token, 16));
    }
  }
  const js = [
    0,
    ...[...segmenter.segment(text)].map(s => s.index + s.segment.length),
  ];
  if (js.join() !== breaks.join()) {
    throw new Error(
      `Intl.Segmenter splits ${data} at ${js} (Node's ICU ${process.versions.icu} is not Unicode ${version}?)`,
    );
  }
  tests.push(tokens.join(' '));
}

const ranges = [];
for (let c = 0; c < 0x110000; c++) {
  const value = props[c];
  if (value === 0) {
    continue;
  }
  const last = ranges[ranges.length - 1];
  if (last !== undefined && last[1] === c - 1 && last[2] === value) {
    last[1] = c;
  } else {
    ranges.push([c, c, value]);
  }
}

const hex = n => `0x${n.toString(16).toUpperCase().padStart(4, '0')}`;
const out = `// Generated by rust/scripts/generate-grapheme-tables.mjs from Unicode
// ${version}'s GraphemeBreakProperty.txt, emoji-data.txt and
// DerivedCoreProperties.txt, the version of Node's ICU ${process.versions.icu},
// whose Intl.Segmenter splits every line of GraphemeBreakTest.txt as the file
// does. Do not edit.

/// \`(first, last, properties)\` of each run of code points with properties
/// other than \`Other\`, sorted: the low four bits are the
/// Grapheme_Cluster_Break (\`Gcb\`'s order), bit 4 is Extended_Pictographic,
/// and bits 5 and 6 the Indic_Conjunct_Break (\`InCB\`'s order).
#[rustfmt::skip]
pub(crate) const GRAPHEME_PROPERTIES: &[(u32, u32, u8)] = &[
${ranges.map(([lo, hi, v]) => `    (${hex(lo)}, ${hex(hi)}, ${v}),`).join('\n')}
];

/// GraphemeBreakTest.txt's cases, one per line (\`÷\` a boundary, \`×\` none).
#[cfg(test)]
pub(crate) const GRAPHEME_BREAK_TEST: &str = "\\
${tests.join('\n')}
";
`;
writeFileSync(OUT, out);
console.warn(
  `wrote ${OUT}: ${ranges.length} ranges, ${tests.length} test cases (Unicode ${version})`,
);

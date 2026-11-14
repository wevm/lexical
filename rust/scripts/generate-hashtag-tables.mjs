/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
// Generates rust/lexical-editor/src/hashtag/tables.rs from
// packages/lexical-hashtag/src/LexicalHashtagExtension.ts.
//
// It takes `getHashtagRegexStringChars` and `getHashtagRegexString` out of
// the source, transpiles them with TypeScript, checks that the pattern is
// still `(^|$|[^&/A])([H])(A*LA*)` (which the Rust matcher assumes), and
// then asks JS's own regular expressions, with the `i` flag and without
// `u` as upstream builds them, which UTF-16 code units each class matches.
// So the tables include what JS's case folding (`Canonicalize`) adds and
// what `String.fromCharCode` truncates the CJK extension ranges to.
//
// Run from the repository root:
//   node rust/scripts/generate-hashtag-tables.mjs
import {readFileSync, writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';

const require = createRequire(import.meta.url);
const ts = require(`${process.cwd()}/node_modules/typescript`);

const SOURCE = 'packages/lexical-hashtag/src/LexicalHashtagExtension.ts';
const OUT = 'rust/lexical-editor/src/hashtag/tables.rs';

const source = readFileSync(SOURCE, 'utf8');

/** The text of `function name(...) {...}`, by matching braces. */
function extractFunction(name) {
  const start = source.indexOf(`function ${name}(`);
  if (start < 0) {
    throw new Error(`${name} not found in ${SOURCE}`);
  }
  // The body starts at the first `{` after the parameter list's `)` and
  // the return type (which may hold braces of its own).
  const paramsEnd = source.indexOf(')', start);
  let i = source.indexOf('{', paramsEnd);
  // Skip a `Readonly<{...}>` return type.
  const arrow = source.slice(paramsEnd, i);
  if (arrow.includes('<')) {
    let depth = 0;
    for (let j = paramsEnd; ; j++) {
      if (source[j] === '<') depth++;
      if (source[j] === '>') {
        depth--;
        if (depth === 0) {
          i = source.indexOf('{', j);
          break;
        }
      }
    }
  }
  let depth = 0;
  let inString = null;
  for (let j = i; j < source.length; j++) {
    const c = source[j];
    if (inString) {
      if (c === '\\') {
        j++;
      } else if (c === inString) {
        inString = null;
      }
      continue;
    }
    if (c === "'" || c === '"' || c === '`') {
      inString = c;
    } else if (c === '/' && source[j + 1] === '/') {
      j = source.indexOf('\n', j);
    } else if (c === '{') {
      depth++;
    } else if (c === '}') {
      depth--;
      if (depth === 0) {
        return source.slice(start, j + 1);
      }
    }
  }
  throw new Error(`unbalanced braces in ${name}`);
}

const code =
  extractFunction('getHashtagRegexStringChars') +
  '\n' +
  extractFunction('getHashtagRegexString') +
  '\nmodule.exports = {chars: getHashtagRegexStringChars(), pattern: getHashtagRegexString()};';
const js = ts.transpileModule(code, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText;
const module = {exports: {}};
// Runs upstream's own two functions, transpiled from the checked-in source.
// eslint-disable-next-line no-new-func
new Function('module', 'exports', js)(module, module.exports);
const {chars, pattern} = module.exports;
const {alpha, alphanumeric, hashChars} = chars;

const expected =
  '(^|$|[^&/' +
  alphanumeric +
  '])([' +
  hashChars +
  '])([' +
  alphanumeric +
  ']*[' +
  alpha +
  '][' +
  alphanumeric +
  ']*)';
if (pattern !== expected) {
  throw new Error(
    'getHashtagRegexString() no longer has the shape the Rust matcher assumes',
  );
}

/** The code units `[class]` matches with the `i` flag, as sorted ranges. */
function ranges(cls) {
  const re = new RegExp('^' + cls + '$', 'i');
  const out = [];
  let start = -1;
  for (let u = 0; u <= 0x10000; u++) {
    const member = u < 0x10000 && re.test(String.fromCharCode(u));
    if (member && start < 0) {
      start = u;
    } else if (!member && start >= 0) {
      out.push([start, u - 1]);
      start = -1;
    }
  }
  return out;
}

const hex = n => '0x' + n.toString(16).toUpperCase().padStart(4, '0');
function table(name, doc, cls) {
  const rs = ranges(cls);
  const lines = rs.map(([a, b]) => `    (${hex(a)}, ${hex(b)}),`);
  return (
    doc.map(line => `/// ${line}`).join('\n') +
    `\npub(super) const ${name}: &[(u16, u16)] = &[\n${lines.join('\n')}\n];\n`
  );
}

const out =
  `//! @generated from \`packages/lexical-hashtag/src/LexicalHashtagExtension.ts\`
//! by \`rust/scripts/generate-hashtag-tables.mjs\`, which asks JS's
//! regular expressions (\`i\`, no \`u\`, as upstream builds them) which
//! UTF-16 code units each class of \`getHashtagRegexString()\` matches. Do
//! not edit by hand; run the script again after an upstream change.

` +
  table(
    'HASHTAG_ALPHA',
    ['`[alpha]`: a character a hashtag needs one of.'],
    '[' + alpha + ']',
  ) +
  '\n' +
  table(
    'HASHTAG_ALPHANUMERIC',
    ['`[alphanumeric]`: the characters of a hashtag.'],
    '[' + alphanumeric + ']',
  ) +
  '\n' +
  table(
    'HASHTAG_BOUNDARY',
    ['`[^&/alphanumeric]`: a character before a hashtag.'],
    '[^&/' + alphanumeric + ']',
  ) +
  '\n' +
  table(
    'HASH_CHARS',
    ['`[hashChars]`: `#` and the full-width `＃`.'],
    '[' + hashChars + ']',
  );

writeFileSync(OUT, out);

// Report what the `i` flag adds to each class, for the vectors.
for (const [name, cls] of [
  ['alpha', alpha],
  ['alphanumeric', alphanumeric],
]) {
  const sensitive = new RegExp('^[' + cls + ']$');
  const insensitive = new RegExp('^[' + cls + ']$', 'i');
  const added = [];
  for (let u = 0; u < 0x10000; u++) {
    const s = String.fromCharCode(u);
    if (insensitive.test(s) && !sensitive.test(s)) {
      added.push(hex(u));
    }
  }
  console.warn(`${name}: the i flag adds ${added.length}: ${added.join(' ')}`);
}
console.warn(`wrote ${OUT}`);

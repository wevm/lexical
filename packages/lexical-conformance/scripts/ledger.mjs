#!/usr/bin/env node
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {execFileSync} from 'node:child_process';
import {mkdtempSync, readdirSync, readFileSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join, relative} from 'node:path';
import {fileURLToPath} from 'node:url';

// The parity ledgers of the Swift port (swift/parity) and the Rust port
// (rust/parity). Both share one list of upstream unit tests,
// swift/parity/upstream-tests.json, which this script generates; each port
// has its own packages.json (which packages are enforced) and skips.json.
const root = fileURLToPath(new URL('../../../', import.meta.url));
const upstreamPath = join(root, 'swift/parity/upstream-tests.json');
const args = process.argv.slice(2);

const readJSON = path => JSON.parse(readFileSync(path, 'utf8'));

const ports = [
  {
    collectPorted: collectSwiftPorted,
    name: 'Swift',
    parity: 'swift/parity',
  },
  {
    collectPorted: collectRustPorted,
    name: 'Rust',
    parity: 'rust/parity',
  },
].map(port => ({
  ...port,
  packages: readJSON(join(root, port.parity, 'packages.json')),
}));

// Swift's packages first, in its order, then any only Rust tracks.
const trackedPackages = [
  ...new Set(ports.flatMap(port => Object.keys(port.packages))),
];

function collectUpstream() {
  const out = join(mkdtempSync(join(tmpdir(), 'ledger-')), 'run.json');
  try {
    execFileSync(
      'pnpm',
      [
        'exec',
        'vitest',
        'run',
        '--project',
        'unit',
        '--reporter=json',
        `--outputFile=${out}`,
        ...trackedPackages.map(name => `packages/${name}/`),
      ],
      {cwd: root, stdio: ['ignore', 'ignore', 'inherit']},
    );
  } catch (error) {
    // A failing upstream test is still a test to port; the report is written.
    if (!readFileSync(out, 'utf8')) {
      throw error;
    }
  }
  const byPackage = Object.fromEntries(trackedPackages.map(name => [name, []]));
  const report = readJSON(out);
  const titlesOf = file =>
    file.assertionResults.map(test =>
      [...test.ancestorTitles, test.title].join(' > '),
    );
  const byName = new Map(report.testResults.map(file => [file.name, file]));
  for (const file of report.testResults.sort((a, b) =>
    a.name.localeCompare(b.name),
  )) {
    const path = relative(join(root, 'packages'), file.name);
    const name = path.split('/')[0];
    // A test file that imports another test module (for its test nodes)
    // also runs that module's suites; they belong to the imported file.
    const imported = new Map();
    for (const [, specifier] of readFileSync(file.name, 'utf8').matchAll(
      /from '(\.{1,2}\/[^']*\.test)'/g,
    )) {
      const base = join(dirname(file.name), specifier);
      const other = ['.ts', '.tsx']
        .map(ext => byName.get(base + ext))
        .find(Boolean);
      for (const title of other ? titlesOf(other) : []) {
        imported.set(title, (imported.get(title) ?? 0) + 1);
      }
    }
    const seen = new Map();
    for (const title of titlesOf(file)) {
      if (imported.get(title)) {
        imported.set(title, imported.get(title) - 1);
        continue;
      }
      const count = (seen.get(title) ?? 0) + 1;
      seen.set(title, count);
      byPackage[name].push(`${path}::${title}${count > 1 ? ` #${count}` : ''}`);
    }
  }
  const {version: lexicalVersion} = readJSON(
    join(root, 'packages/lexical/package.json'),
  );
  const lines = Object.entries(byPackage).map(
    ([name, ids]) =>
      `${JSON.stringify(name)}:[\n${ids.map(id => JSON.stringify(id)).join(',\n')}\n]`,
  );
  return `{"lexicalVersion":${JSON.stringify(lexicalVersion)},"packages":{\n${lines.join(',\n')}\n}}\n`;
}

// The files under `dir` with `extension`, leaving out build directories
// (Cargo's `target`, SwiftPM's `.build`).
function sourceFiles(dir, extension) {
  const files = [];
  for (const entry of readdirSync(dir, {withFileTypes: true})) {
    const path = join(dir, entry.name);
    if (entry.isDirectory() && !['target', '.build'].includes(entry.name)) {
      files.push(...sourceFiles(path, extension));
    } else if (entry.isFile() && entry.name.endsWith(extension)) {
      files.push(path);
    }
  }
  return files.sort();
}

// `@Test(.upstream("<id>"))`, one string literal per id.
function collectSwiftPorted() {
  const marks = [];
  for (const file of sourceFiles(join(root, 'swift/Tests'), '.swift')) {
    for (const [, literal] of readFileSync(file, 'utf8').matchAll(
      /\.upstream\(\s*("(?:[^"\\]|\\.)*")\s*\)/g,
    )) {
      marks.push({at: relative(root, file), id: JSON.parse(literal)});
    }
  }
  return {errors: [], marks};
}

// A `// upstream: <id>` line directly above a `#[test]` (other attributes,
// such as `#[cfg(...)]`, may come between), one id per line. An id a line
// comment can't hold as written (leading or trailing whitespace, control
// characters) is written as a JSON string literal.
function collectRustPorted() {
  const marks = [];
  const errors = [];
  for (const file of sourceFiles(join(root, 'rust'), '.rs')) {
    const lines = readFileSync(file, 'utf8').split('\n');
    for (let i = 0; i < lines.length; i++) {
      const match = /^\s*\/\/ upstream: (.*)$/.exec(lines[i]);
      if (!match) {
        continue;
      }
      const at = `${relative(root, file)}:${i + 1}`;
      let id = match[1];
      if (id.startsWith('"')) {
        try {
          id = JSON.parse(id);
        } catch {
          errors.push(`${at}: ${id} is not a JSON string literal`);
          continue;
        }
      }
      marks.push({at, id});
      let next = i + 1;
      while (
        next < lines.length &&
        (/^\s*\/\/ upstream: /.test(lines[next]) ||
          (/^\s*#\[/.test(lines[next]) && lines[next].trim() !== '#[test]'))
      ) {
        next++;
      }
      if (next >= lines.length || lines[next].trim() !== '#[test]') {
        errors.push(`${at}: \`// upstream:\` must be directly above a #[test]`);
      }
    }
  }
  return {errors, marks};
}

// The keys of a JSON object, in order and with repeats (JSON.parse keeps
// only the last of a repeated key).
function objectKeys(text) {
  const keys = [];
  let depth = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"') {
      let end = i + 1;
      while (text[end] !== '"') {
        end += text[end] === '\\' ? 2 : 1;
      }
      if (depth === 1 && /^\s*:/.test(text.slice(end + 1, end + 16))) {
        keys.push(JSON.parse(text.slice(i, end + 1)));
      }
      i = end;
    } else if (c === '{' || c === '[') {
      depth++;
    } else if (c === '}' || c === ']') {
      depth--;
    }
  }
  return keys;
}

function duplicateKeys(path) {
  const seen = new Set();
  const repeated = new Set();
  for (const key of objectKeys(readFileSync(path, 'utf8'))) {
    (seen.has(key) ? repeated : seen).add(key);
  }
  return [...repeated];
}

const reasons = [
  'dom-only',
  'react-only',
  'out-of-scope-package',
  'covered-by-vector',
  'deviation',
];

function checkPort(port, upstream, known) {
  const {name, parity, packages} = port;
  const errors = [];
  for (const file of ['packages.json', 'skips.json']) {
    for (const key of duplicateKeys(join(root, parity, file))) {
      errors.push(
        `${parity}/${file}: ${JSON.stringify(key)} is listed more than once`,
      );
    }
  }
  for (const [pkg, status] of Object.entries(packages)) {
    if (!(pkg in upstream.packages)) {
      errors.push(
        `${parity}/packages.json: ${pkg} is not in swift/parity/upstream-tests.json; run \`pnpm run ledger\` in packages/lexical-conformance`,
      );
    }
    if (status !== 'enforced' && status !== 'pending') {
      errors.push(
        `${parity}/packages.json: ${pkg} has unknown status ${JSON.stringify(status)} (enforced or pending)`,
      );
    }
  }
  const collected = port.collectPorted();
  errors.push(...collected.errors);
  const ported = new Map();
  for (const {at, id} of collected.marks) {
    if (!known.has(id)) {
      errors.push(`${at}: unknown upstream test ${JSON.stringify(id)}`);
    }
    // Swift's tests may share an id; a Rust id is marked once.
    if (name === 'Rust' && ported.has(id)) {
      errors.push(
        `${at}: ${JSON.stringify(id)} is already marked at ${ported.get(id)}`,
      );
    }
    if (!ported.has(id)) {
      ported.set(id, at);
    }
  }
  const skips = readJSON(join(root, parity, 'skips.json'));
  for (const [id, skip] of Object.entries(skips)) {
    const where = `${parity}/skips.json: ${JSON.stringify(id)}`;
    if (!known.has(id)) {
      errors.push(
        `${parity}/skips.json: unknown upstream test ${JSON.stringify(id)}`,
      );
    }
    if (!reasons.includes(skip.reason)) {
      errors.push(`${where} has unknown reason ${JSON.stringify(skip.reason)}`);
    }
    if (typeof skip.note !== 'string' || skip.note === '') {
      errors.push(`${where} needs a note saying why`);
    }
    if (ported.has(id)) {
      errors.push(`${where} is also ported, at ${ported.get(id)}`);
    }
    // An out-of-scope-package skip names the packages it needs; once they
    // are all enforced, it must be ported with them.
    if (skip.reason === 'out-of-scope-package') {
      if (!Array.isArray(skip.packages) || skip.packages.length === 0) {
        errors.push(`${where} must list the packages it needs`);
      } else if (skip.packages.every(pkg => packages[pkg] === 'enforced')) {
        errors.push(
          `${where} needs only enforced packages (${skip.packages.join(', ')}); port it`,
        );
      }
    }
  }
  const rows = [];
  const total = {done: 0, ids: 0, missing: 0, skipped: 0};
  const byReason = new Map(reasons.map(reason => [reason, 0]));
  for (const [pkg, status] of Object.entries(packages)) {
    const ids = upstream.packages[pkg] ?? [];
    const done = ids.filter(id => ported.has(id)).length;
    const skipped = ids.filter(id => id in skips);
    const missing = ids.filter(id => !ported.has(id) && !(id in skips));
    if (status === 'enforced') {
      errors.push(
        ...missing.map(
          id =>
            `${name}: ${pkg} is enforced but ${JSON.stringify(id)} is neither ported nor skipped`,
        ),
      );
    }
    for (const id of skipped) {
      byReason.set(skips[id].reason, (byReason.get(skips[id].reason) ?? 0) + 1);
    }
    total.ids += ids.length;
    total.done += done;
    total.skipped += skipped.length;
    total.missing += missing.length;
    rows.push(
      `| ${pkg} | ${status} | ${ids.length} | ${done} | ${skipped.length} | ${missing.length} |`,
    );
  }
  rows.push(
    `| **Total** | | ${total.ids} | ${total.done} | ${total.skipped} | ${total.missing} |`,
  );
  const reasonText = [...byReason]
    .filter(([, count]) => count > 0)
    .map(([reason, count]) => `${reason} ${count}`)
    .join(', ');
  process.stdout.write(
    `\n${name} (${parity})\n\n| Package | Status | Upstream tests | Ported | Skipped | Remaining |\n|---|---|---|---|---|---|\n${rows.join('\n')}\n\nSkipped by reason: ${reasonText || 'none'}\n`,
  );
  return errors;
}

function check() {
  const text = readFileSync(upstreamPath, 'utf8');
  if (args.includes('--fresh') && collectUpstream() !== text) {
    fail(
      'swift/parity/upstream-tests.json is stale. Run `pnpm run ledger` in packages/lexical-conformance.',
    );
  }
  const upstream = readJSON(upstreamPath);
  const known = new Map();
  for (const [name, ids] of Object.entries(upstream.packages)) {
    for (const id of ids) {
      known.set(id, name);
    }
  }
  process.stdout.write(`Upstream lexical ${upstream.lexicalVersion}\n`);
  const errors = ports.flatMap(port => checkPort(port, upstream, known));
  if (errors.length) {
    fail(errors.join('\n'));
  }
}

function fail(message) {
  console.error(`\n${message}`);
  process.exit(1);
}

if (!args.includes('--check')) {
  writeFileSync(upstreamPath, collectUpstream());
}
check();

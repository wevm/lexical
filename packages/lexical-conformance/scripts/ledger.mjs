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
import {join, relative} from 'node:path';
import {fileURLToPath} from 'node:url';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const parity = join(root, 'swift/parity');
const upstreamPath = join(parity, 'upstream-tests.json');
const args = process.argv.slice(2);

const readJSON = path => JSON.parse(readFileSync(path, 'utf8'));
const packages = readJSON(join(parity, 'packages.json'));

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
        ...Object.keys(packages).map(name => `packages/${name}/`),
      ],
      {cwd: root, stdio: ['ignore', 'ignore', 'inherit']},
    );
  } catch (error) {
    // A failing upstream test is still a test to port; the report is written.
    if (!readFileSync(out, 'utf8')) {
      throw error;
    }
  }
  const byPackage = Object.fromEntries(
    Object.keys(packages).map(name => [name, []]),
  );
  const report = readJSON(out);
  for (const file of report.testResults.sort((a, b) =>
    a.name.localeCompare(b.name),
  )) {
    const path = relative(join(root, 'packages'), file.name);
    const name = path.split('/')[0];
    const seen = new Map();
    for (const test of file.assertionResults) {
      const title = [...test.ancestorTitles, test.title].join(' > ');
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

function swiftFiles(dir) {
  return readdirSync(dir, {recursive: true, withFileTypes: true})
    .filter(entry => entry.isFile() && entry.name.endsWith('.swift'))
    .map(entry => join(entry.parentPath, entry.name));
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
  const errors = [];
  const ported = new Set();
  for (const file of swiftFiles(join(root, 'swift/Tests'))) {
    for (const [, literal] of readFileSync(file, 'utf8').matchAll(
      /\.upstream\(\s*("(?:[^"\\]|\\.)*")\s*\)/g,
    )) {
      const id = JSON.parse(literal);
      if (!known.has(id)) {
        errors.push(
          `${relative(root, file)}: unknown upstream test ${literal}`,
        );
      }
      ported.add(id);
    }
  }
  const skips = readJSON(join(parity, 'skips.json'));
  const reasons = new Set([
    'dom-only',
    'react-only',
    'out-of-scope-package',
    'covered-by-vector',
    'deviation',
  ]);
  for (const [id, skip] of Object.entries(skips)) {
    if (!known.has(id)) {
      errors.push(`skips.json: unknown upstream test ${JSON.stringify(id)}`);
    }
    if (!reasons.has(skip.reason)) {
      errors.push(
        `skips.json: ${JSON.stringify(id)} has unknown reason ${JSON.stringify(skip.reason)}`,
      );
    }
    if (ported.has(id)) {
      errors.push(`skips.json: ${JSON.stringify(id)} is also ported`);
    }
  }
  const rows = [];
  for (const [name, ids] of Object.entries(upstream.packages)) {
    const done = ids.filter(id => ported.has(id)).length;
    const skipped = ids.filter(id => id in skips).length;
    const missing = ids.filter(id => !ported.has(id) && !(id in skips));
    if (packages[name] === 'enforced') {
      errors.push(
        ...missing.map(
          id =>
            `${name} is enforced but ${JSON.stringify(id)} is neither ported nor skipped`,
        ),
      );
    }
    rows.push(
      `| ${name} | ${packages[name]} | ${ids.length} | ${done} | ${skipped} | ${missing.length} |`,
    );
  }
  process.stdout.write(`Upstream lexical ${upstream.lexicalVersion}\n\n`);
  process.stdout.write(
    '| Package | Status | Upstream tests | Ported | Skipped | Remaining |\n|---|---|---|---|---|---|\n',
  );
  process.stdout.write(rows.join('\n') + '\n');
  if (errors.length) {
    fail(errors.join('\n'));
  }
}

function fail(message) {
  console.error(`\n${message}`);
  process.exit(1);
}

if (args.includes('--check')) {
  check();
} else {
  writeFileSync(upstreamPath, collectUpstream());
}

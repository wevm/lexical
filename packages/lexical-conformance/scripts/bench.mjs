/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

// JS baselines for swift/Benchmarks/LexicalBenchmarks (see bench/harness.ts):
// headless in Node (the model), and mounted in Chromium and WebKit through
// Playwright (the model and the DOM reconcile). The upstream packages are
// bundled from their TypeScript sources (the `source` export condition)
// with NODE_ENV=production, as an app ships them.
//
//   node scripts/bench.mjs [--node] [--browser chromium,webkit]
//     [--only <prefix>] [--markdown <path>]
//
// With neither --node nor --browser, it runs all three.

import * as esbuild from 'esbuild';
import {appendFileSync, mkdtempSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {parseArgs} from 'node:util';

const here = dirname(fileURLToPath(import.meta.url));
const {values} = parseArgs({
  options: {
    browser: {type: 'string'},
    markdown: {type: 'string'},
    node: {type: 'boolean'},
    only: {type: 'string'},
  },
});
const runAll = !values.node && values.browser === undefined;
const browsers = runAll
  ? ['chromium', 'webkit']
  : values.browser
    ? values.browser.split(',')
    : [];

async function bundle(entry, platform) {
  const result = await esbuild.build({
    bundle: true,
    conditions: ['source'],
    define: {'process.env.NODE_ENV': '"production"'},
    entryPoints: [join(here, '..', 'bench', entry)],
    format: platform === 'node' ? 'esm' : 'iife',
    logLevel: 'warning',
    platform,
    target: 'es2022',
    write: false,
  });
  return result.outputFiles[0].text;
}

// Swift's `%-44@ p50 %8.3f ms  p95 %8.3f ms` line and `--markdown` table.
function formatResult(r) {
  const goal =
    r.target === undefined
      ? ''
      : ` (target ${r.target.toFixed(1)} ms p95, ${
          r.p95 <= r.target ? 'met' : 'NOT met'
        })`;
  return `${r.name.padEnd(44)} p50 ${r.p50.toFixed(3).padStart(8)} ms  p95 ${r.p95
    .toFixed(3)
    .padStart(8)} ms${goal}`;
}

function markdownTable(title, results) {
  let table = `### ${title}\n\n| Benchmark | p50 (ms) | p95 (ms) | Target p95 (ms) |\n| --- | ---: | ---: | ---: |\n`;
  for (const r of results) {
    const target =
      r.target === undefined
        ? ''
        : `${r.target}${r.p95 <= r.target ? ' ✅' : ' ❌'}`;
    table += `| ${r.name} | ${r.p50.toFixed(3)} | ${r.p95.toFixed(3)} | ${target} |\n`;
  }
  return table + '\n';
}

function output(title, {fixture, results}) {
  process.stdout.write(`\n${title}\n${fixture}\n`);
  for (const r of results) {
    process.stdout.write(formatResult(r) + '\n');
  }
  if (values.markdown) {
    appendFileSync(
      values.markdown,
      markdownTable(`${title} (${fixture.replace(/^Fixture: /, '')})`, results),
    );
  }
}

const dir = mkdtempSync(join(tmpdir(), 'lexical-bench-'));

if (runAll || values.node) {
  const file = join(dir, 'node.mjs');
  writeFileSync(file, await bundle('node.ts', 'node'));
  const {runNode} = await import(pathToFileURL(file).href);
  output(
    `JS headless, Node ${process.versions.node} (model only)`,
    runNode(values.only),
  );
}

if (browsers.length > 0) {
  const playwright = await import('playwright');
  const script = await bundle('browser.ts', 'browser');
  for (const name of browsers) {
    const browser = await playwright[name].launch();
    try {
      const page = await browser.newPage();
      page.on('pageerror', error => console.error(error));
      await page.setContent('<!doctype html><html><body></body></html>');
      await page.addScriptTag({content: script});
      const result = await page.evaluate(
        only => globalThis.runBrowser(only ?? undefined),
        values.only ?? null,
      );
      output(
        `JS ${name} ${browser.version()} (model and DOM reconcile)`,
        result,
      );
    } finally {
      await browser.close();
    }
  }
}

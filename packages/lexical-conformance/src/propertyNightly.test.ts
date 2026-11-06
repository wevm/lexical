/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import * as fc from 'fast-check';
import {type ChildProcess, execFileSync, spawn} from 'node:child_process';
import {
  appendFileSync,
  createReadStream,
  createWriteStream,
  mkdtempSync,
  rmSync,
} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {test} from 'vitest';

import {
  type AbstractSequence,
  documentArbitrary,
  runDocument,
  runSequence,
  sequenceArbitrary,
  withJSDOM,
} from './property';

// The nightly property run (swift/DESIGN.md, Conformance tooling): fresh
// seeds, with Swift as the system under test for every candidate fast-check
// draws or shrinks to. Swift answers over two named pipes from the
// `PropertyServer` suite of swift/Tests/ConformanceTests/PropertyTests.swift,
// so a failure shrinks against Swift's real answers. Opt in with
// PROPERTY_NIGHTLY=1, after `swift build --build-tests` in swift/:
//
//   PROPERTY_NIGHTLY=1 PROPERTY_SEED=123 PROPERTY_RUNS=2000 \
//     pnpm --filter lexical-conformance exec vitest run src/propertyNightly.test.ts
//
// On a failure it prints the seed and the minimized input; add the input to
// src/propertyRegressions.ts and regenerate the vectors.

const enabled = process.env.PROPERTY_NIGHTLY === '1';
const seed = Number(process.env.PROPERTY_SEED ?? Date.now() % 2 ** 31);
const runs = Number(process.env.PROPERTY_RUNS ?? 1000);
const swiftDir = fileURLToPath(new URL('../../../swift/', import.meta.url));

type Json = unknown;

/** The Swift `PropertyServer`, restarted after it crashes. */
class SwiftServer {
  private dir = mkdtempSync(join(tmpdir(), 'lexical-property-'));
  private process: ChildProcess | null = null;
  private write: ((line: string) => void) | null = null;
  private pending: ((line: string | null) => void) | null = null;
  private stderr = '';

  private start(): void {
    rmSync(this.dir, {force: true, recursive: true});
    this.dir = mkdtempSync(join(tmpdir(), 'lexical-property-'));
    const requests = join(this.dir, 'requests');
    const responses = join(this.dir, 'responses');
    execFileSync('mkfifo', [requests, responses]);
    this.stderr = '';
    // The test runner itself, not `swift test`, which would wait on the
    // build directory's lock while an earlier server is still exiting.
    const child = spawn(
      join(swiftDir, '.build/debug/ConformanceTests-test-runner'),
      ['--testing-library', 'swift-testing', '--filter', 'PropertyServer'],
      {
        cwd: swiftDir,
        env: {...process.env, LEXICAL_PROPERTY_SERVER: this.dir},
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    );
    const keep = (chunk: Buffer) => {
      this.stderr = (this.stderr + chunk.toString()).slice(-200000);
    };
    child.stdout!.on('data', keep);
    child.stderr!.on('data', keep);
    this.process = child;
    // Opening a FIFO blocks until the other end opens it, off the main
    // thread, so these resolve once Swift is listening.
    const out = createWriteStream(requests);
    out.on('error', () => {});
    this.write = line => out.write(line + '\n');
    // Split on `\n` only: readline also ends lines at U+2028 and U+2029,
    // which JSON leaves unescaped in strings.
    const input = createReadStream(responses, {encoding: 'utf8'});
    let buffered = '';
    input.on('data', chunk => {
      buffered += chunk;
      let newline;
      while ((newline = buffered.indexOf('\n')) >= 0) {
        const reply = buffered.slice(0, newline);
        buffered = buffered.slice(newline + 1);
        const resolve = this.pending;
        this.pending = null;
        if (resolve !== null) {
          resolve(reply);
        }
      }
    });
    input.on('error', () => {});
    const died = () => {
      // A server that already gave way to a new one is not this one.
      if (this.process !== child) {
        return;
      }
      this.process = null;
      const resolve = this.pending;
      this.pending = null;
      if (resolve !== null) {
        resolve(null);
      }
    };
    input.on('close', died);
    child.on('exit', died);
  }

  /** Swift's answer, or `{crashed}` when the process died on it. */
  async ask(request: Json): Promise<Json> {
    if (this.process === null) {
      this.start();
    }
    const answer = new Promise<string | null>(resolve => {
      this.pending = resolve;
    });
    this.write!(JSON.stringify(request));
    const line = await answer;
    if (line === null) {
      // The trap's message and the first frames of the crashed thread.
      const fatal = this.stderr.search(/Fatal error|Program crashed/);
      return {
        crashed: this.stderr
          .slice(fatal < 0 ? -3000 : fatal)
          .split('\n')
          .filter(text => !/^\s*(r\w\w|rip|rflags|cs|fs|gs) 0x/.test(text))
          .slice(0, 40)
          .join('\n'),
      };
    }
    return JSON.parse(line);
  }

  close(): void {
    if (this.process !== null) {
      this.process.kill();
    }
    rmSync(this.dir, {force: true, recursive: true});
  }
}

/** The fields of `js` and `swift` that differ, as `field: js ≠ swift`. */
function differences(js: Json, swift: Json): string[] {
  const a = (js ?? {}) as {[key: string]: Json};
  const b = (swift ?? {}) as {[key: string]: Json};
  const keys = [...new Set([...Object.keys(a), ...Object.keys(b)])].sort();
  return keys
    .filter(key => JSON.stringify(a[key]) !== JSON.stringify(b[key]))
    .map(
      key =>
        `${key}:\n    js:    ${JSON.stringify(a[key])}\n    swift: ${JSON.stringify(b[key])}`,
    );
}

function summary(line: string): void {
  process.stdout.write(line + '\n');
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, line + '\n\n');
  }
}

/** Runs `property` and records its minimized counterexample, if any. */
async function check<T>(
  kind: string,
  property: fc.IAsyncProperty<[T]>,
  failures: string[],
): Promise<void> {
  const result = await fc.check(property, {numRuns: runs, seed});
  if (!result.failed) {
    summary(`${kind}: ${result.numRuns} cases matched.`);
    return;
  }
  const counterexample =
    result.counterexample === null ? null : result.counterexample[0];
  const failure = `${kind}: counterexample ${JSON.stringify(counterexample)}\n(${result.numShrinks} shrinks, path ${result.counterexamplePath})\n${String(result.errorInstance)}`;
  failures.push(failure);
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(
      process.env.GITHUB_STEP_SUMMARY,
      `### ${kind}: failed\n\n\`\`\`\n${failure.slice(0, 20000)}\n\`\`\`\n`,
    );
  }
}

test.runIf(enabled)(
  'property/nightly',
  async () => {
    summary(`Seed ${seed}, ${runs} runs of each kind.`);
    const swift = new SwiftServer();
    const failures: string[] = [];
    try {
      const documents = fc.asyncProperty(
        documentArbitrary(false),
        async document => {
          const input = JSON.stringify(document);
          const js = withJSDOM(() => runDocument(input));
          const answer = await swift.ask({input, kind: 'document'});
          const diff = differences(js, answer);
          if (diff.length > 0) {
            throw new Error(`input ${input}\n  ${diff.join('\n  ')}`);
          }
        },
      );
      const sequences = fc.asyncProperty(
        // Longer sessions than the checked-in vectors, which keep to 6 steps.
        sequenceArbitrary(12),
        async (sequence: AbstractSequence) => {
          const js = runSequence('nightly', sequence);
          const answer = (await swift.ask({
            case: {document: js.document, steps: js.steps},
            kind: 'sequence',
          })) as Json[] | {crashed: string};
          const results = Array.isArray(answer) ? answer : [answer];
          for (let i = 0; i < js.results.length; i++) {
            const diff = differences(js.results[i], results[i]);
            if (diff.length > 0) {
              throw new Error(
                `step ${i} ${JSON.stringify(js.steps[i])}\n  ${diff.join('\n  ')}`,
              );
            }
          }
        },
      );
      await check('documents', documents, failures);
      await check('sequences', sequences, failures);
    } finally {
      swift.close();
    }
    if (failures.length > 0) {
      throw new Error(
        `Property run failed (seed ${seed}). Add each counterexample to src/propertyRegressions.ts and run \`pnpm run generate\`.\n\n${failures.join('\n\n')}`,
      );
    }
  },
  6 * 60 * 60 * 1000,
);

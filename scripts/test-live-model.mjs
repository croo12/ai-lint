// Explicit opt-in smoke test: sends only synthetic fixtures to the configured server.
// Build with npm run build:cli, then supply AI_LINT_MODEL_* env vars or a root .env.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { performance } from 'node:perf_hooks';

const root = fileURLToPath(new URL('../', import.meta.url));
const binary = join(root, 'target/release', process.platform === 'win32' ? 'ai-lint.exe' : 'ai-lint');
for (const [name, expected] of [['pass', 0], ['violation', 1], ['no-candidate', 0]]) {
  const start = performance.now();
  const result = await new Promise((accept, reject) => {
    execFile(binary, ['check', '--format', 'json', '--env-file', join(root, '.env'),
      '--rules', 'contextual-effect',
      join(root, `tests/fixtures/live-model/${name}.tsx`)],
    { cwd: root, windowsHide: true, timeout: 45000, maxBuffer: 1024 * 1024 }, (error, stdout) => {
      if (error && ![1, 2].includes(error.code)) return reject(error);
      try { accept({ code: error?.code ?? 0, report: JSON.parse(stdout) }); } catch (error) { reject(error); }
    });
  });
  const elapsedMs = Math.round(performance.now() - start);
  console.log(JSON.stringify({ case: name, elapsedMs, ...result }));
  assert.equal(result.code, expected, `${name}: unexpected process exit`);
  assert.equal(result.report.exitCode, expected);
  assert.deepEqual(result.report.errors, []);
  assert.equal(result.report.files.length, 1);
  assert.deepEqual(result.report.files[0].syntaxErrors, []);
  assert.equal(result.report.files[0].violations.length, expected);
  if (expected) assert.equal(result.report.files[0].violations[0].ruleId, 'contextual-effect');
}
console.log('Live model smoke test: 3/3 passed');

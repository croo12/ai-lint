import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { installHooks } from '../dist/index.js';

const repository = fileURLToPath(new URL('../../../', import.meta.url));
const binary = join(repository, 'target/debug', process.platform === 'win32' ? 'ai-lint.exe' : 'ai-lint');
const entry = fileURLToPath(new URL('../dist/cli.js', import.meta.url));
const json = async path => JSON.parse(await readFile(path, 'utf8'));
async function fixture(t) {
  const workspace = await mkdtemp(join(tmpdir(), 'ai-lint-install-'));
  t.after(() => rm(workspace, { recursive: true, force: true }));
  await mkdir(join(workspace, 'src'));
  return { workspace, binary, sourceRoots: ['src'] };
}

test('installation preserves existing settings, backs up originals and is idempotent', async t => {
  const options = await fixture(t);
  const path = join(options.workspace, '.claude/settings.json');
  await mkdir(join(options.workspace, '.claude'));
  const existing = JSON.stringify({ permissions: { allow: ['Read'] }, hooks: {
    PostToolUse: [{ matcher: 'Write', hooks: [{ type: 'command', command: 'echo existing' }] }],
    SessionStart: [{ hooks: [{ type: 'command', command: 'echo start' }] }],
  } });
  await writeFile(path, existing);
  const result = await installHooks(options);
  assert.equal(result.changed.length, 3);
  assert.equal(result.backups.length, 1);
  assert.equal(await readFile(result.backups[0], 'utf8'), existing);
  const settings = await json(path);
  assert.deepEqual(settings.permissions, { allow: ['Read'] });
  assert.equal(settings.hooks.PostToolUse[0].hooks[0].command, 'echo existing');
  assert.equal(settings.hooks.SessionStart.length, 1);
  assert.deepEqual(await installHooks(options), { changed: [], backups: [], dryRun: false });
  await installHooks({ ...options, timeoutMs: 120000 });
  const updated = await json(path);
  assert.equal(updated.hooks.PostToolUse.length, 2);
  assert.equal(updated.hooks.Stop.length, 1);
  assert.equal(updated.hooks.Stop[0].hooks[0].timeout, 150);
});

test('dry run reports changes without creating configuration directories', async t => {
  const options = await fixture(t);
  assert.equal((await installHooks({ ...options, dryRun: true })).changed.length, 3);
  for (const directory of ['.ai-lint', '.claude', '.codex']) {
    await assert.rejects(access(join(options.workspace, directory)), { code: 'ENOENT' });
  }
});

test('malformed second host settings prevents all configuration writes', async t => {
  const options = await fixture(t);
  await mkdir(join(options.workspace, '.codex'));
  const path = join(options.workspace, '.codex/hooks.json');
  await writeFile(path, '{invalid');
  await assert.rejects(installHooks(options), SyntaxError);
  assert.equal(await readFile(path, 'utf8'), '{invalid');
  await assert.rejects(access(join(options.workspace, '.ai-lint')), { code: 'ENOENT' });
  await assert.rejects(access(join(options.workspace, '.claude')), { code: 'ENOENT' });
});

test('invalid binary, source roots and agent fail before installation', async t => {
  const options = await fixture(t);
  await assert.rejects(installHooks({ ...options, binary: 'missing' }));
  await assert.rejects(installHooks({ ...options, sourceRoots: ['missing'] }));
  await assert.rejects(installHooks({ ...options, sourceRoots: [] }));
  await assert.rejects(installHooks({ ...options, sourceRoots: [tmpdir()] }), /inside workspace/);
  await assert.rejects(installHooks({ ...options, agent: 'unknown' }), /agent/);
  await assert.rejects(access(join(options.workspace, '.ai-lint')), { code: 'ENOENT' });
});

test('global Claude installer targets user settings and preserves them on repeat', async t => {
  const options = await fixture(t);
  const args = [entry, 'install', '--global', '--agent', 'claude-code', '--bin', binary];
  const env = { ...process.env, USERPROFILE: options.workspace, HOME: options.workspace };
  await promisify(execFile)(process.execPath, args, { env, windowsHide: true });
  const path = join(options.workspace, '.claude/ai-lint/adapter.json');
  assert.equal((await json(path)).workspaceFromCwd, true);
  assert.equal((await json(join(options.workspace, '.claude/settings.json'))).hooks.Stop.length, 1);
  const repeated = await promisify(execFile)(process.execPath, args, { env, windowsHide: true });
  assert.match(repeated.stdout, /"changed": \[\]/);
  await assert.rejects(access(join(options.workspace, '.codex')), { code: 'ENOENT' });
  await assert.rejects(installHooks({ global: true, agent: 'codex' }), /claude-code/);
});

test('install CLI supports selecting only Codex', async t => {
  const options = await fixture(t);
  const result = await promisify(execFile)(process.execPath, [entry, 'install', '--workspace', options.workspace,
    '--agent', 'codex', '--bin', binary, '--source-root', 'src'], { windowsHide: true });
  assert.match(result.stdout, /hooks.json/);
  assert.equal(result.stderr, '');
  assert.equal((await json(join(options.workspace, '.codex/hooks.json'))).hooks.Stop.length, 1);
  assert.deepEqual((await json(join(options.workspace, '.ai-lint/adapter.json'))).sourceRoots, ['src']);
  await assert.rejects(access(join(options.workspace, '.claude')), { code: 'ENOENT' });
});

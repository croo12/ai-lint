import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import { AiLintAdapter, handleHook, claudeCodeHooks, codexHooks } from '../dist/index.js';

const exec = promisify(execFile);
const repository = fileURLToPath(new URL('../../../', import.meta.url));
const binary = join(repository, 'target/debug', process.platform === 'win32' ? 'ai-lint.exe' : 'ai-lint');
const entry = fileURLToPath(new URL('../dist/cli.js', import.meta.url));
const bad = "useEffect(() => { setState(1); }, []);";
const good = "const greeting: string = '한글';";

async function fixture(t) {
  const workspace = await mkdtemp(join(tmpdir(), 'ai-lint-hooks-'));
  t.after(() => rm(workspace, { recursive: true, force: true }));
  await mkdir(join(workspace, 'src'));
  await exec('git', ['init', workspace]);
  await writeFile(join(workspace, 'src', 'file with spaces.tsx'), bad);
  return { workspace, binary, sourceRoots: ['src'], envFile: join(repository, '.env.example'), timeoutMs: 10000 };
}

test('Claude Write/Edit inputs block findings, allow clean files and ignore non-source tools', async t => {
  const config = await fixture(t);
  for (const tool_name of ['Write', 'Edit', 'MultiEdit']) {
    const output = await handleHook(config, { hook_event_name: 'PostToolUse', tool_name, cwd: config.workspace, tool_input: { file_path: 'src/file with spaces.tsx' } });
    assert.equal(output.decision, 'block');
    assert.match(output.reason, /no-set-state-in-effect/);
  }
  assert.deepEqual(await handleHook(config, { hook_event_name: 'PostToolUse', tool_name: 'Read' }), {});
  await writeFile(join(config.workspace, 'src/file with spaces.tsx'), good);
  assert.deepEqual(await handleHook(config, { hook_event_name: 'PostToolUse', tool_name: 'Write', tool_input: { file_path: 'src/file with spaces.tsx' } }), {});
});

test('Codex apply_patch and shell events detect untracked and staged edits', async t => {
  const config = await fixture(t);
  for (const tool_name of ['apply_patch', 'Bash', 'exec_command', 'write_stdin']) {
    const output = await handleHook(config, { hook_event_name: 'PostToolUse', tool_name, cwd: config.workspace,
      tool_input: { command: '*** Begin Patch\n*** Add File: src/file with spaces.tsx\n+x\n*** End Patch' } });
    assert.equal(output.decision, 'block', output.reason);
  }
  await exec('git', ['-C', config.workspace, 'add', '--', 'src']);
  const output = await handleHook(config, { hook_event_name: 'PostToolUse', tool_name: 'Bash', tool_input: { command: 'git add src' } });
  assert.equal(output.decision, 'block');
});

test('Stop scans scoped sources and repeated Stop has explicit loop protection', async t => {
  const config = await fixture(t);
  assert.equal((await handleHook(config, { hook_event_name: 'Stop', stop_hook_active: false })).decision, 'block');
  const repeated = await handleHook(config, { hook_event_name: 'Stop', stop_hook_active: true });
  assert.equal(repeated.decision, undefined);
  assert.match(repeated.systemMessage, /무한/);
  await writeFile(join(config.workspace, 'src/file with spaces.tsx'), good);
  await writeFile(join(config.workspace, 'outside-scope.ts'), bad);
  assert.deepEqual(await handleHook(config, { hook_event_name: 'Stop' }), {});
  assert.equal((await handleHook({ ...config, sourceRoots: ['missing-directory'] }, { hook_event_name: 'Stop' })).decision, 'block');
});

test('runner handles syntax errors, missing binaries and workspace escape', async t => {
  const config = await fixture(t);
  await writeFile(join(config.workspace, 'src/file with spaces.tsx'), 'const broken: = 1;');
  const report = await new AiLintAdapter(config).check(['src/file with spaces.tsx']);
  assert.equal(report.exitCode, 1);
  assert.ok(report.files[0].syntaxErrors.length);
  await assert.rejects(new AiLintAdapter(config).check([join(repository, 'apps/web/src/App.tsx')]), /inside/);
  const output = await handleHook({ ...config, binary: join(config.workspace, 'missing.exe') }, { hook_event_name: 'Stop' });
  assert.equal(output.decision, 'block');
  assert.match(output.reason, /executable not found/);
});

test('hook executable returns valid blocking JSON on stdin events and malformed input', async t => {
  const config = await fixture(t);
  const configPath = join(config.workspace, 'adapter.json');
  await writeFile(configPath, JSON.stringify(config));
  for (const input of [JSON.stringify({ hook_event_name: 'Stop' }), 'not json']) {
    const output = await new Promise((accept, reject) => {
      const child = spawn(process.execPath, [entry, '--config', configPath], { windowsHide: true });
      let stdout = ''; let stderr = '';
      child.stdout.on('data', data => { stdout += data; });
      child.stderr.on('data', data => { stderr += data; });
      child.on('error', reject);
      child.on('close', code => accept({ code, stdout, stderr }));
      child.stdin.end(input);
    });
    assert.equal(output.code, 0);
    assert.equal(output.stderr, '');
    assert.equal(JSON.parse(output.stdout).decision, 'block');
  }
});

test('global hooks follow each event cwd instead of the installation workspace', async t => {
  const config = await fixture(t);
  const global = { ...config, workspace: repository, workspaceFromCwd: true };
  const output = await handleHook(global, { hook_event_name: 'Stop', cwd: config.workspace });
  assert.equal(output.decision, 'block');
  assert.match(output.reason, /no-set-state-in-effect/);
  assert.match((await handleHook(global, { hook_event_name: 'Stop' })).reason, /absolute cwd/);
  await writeFile(join(config.workspace, 'src/file with spaces.tsx'), good);
  assert.deepEqual(await handleHook(global, { hook_event_name: 'Stop', cwd: config.workspace }), {});
});

test('host config generators include synchronous PostToolUse and Stop commands', () => {
  for (const create of [claudeCodeHooks, codexHooks]) {
    const config = create('/path with spaces/cli.js', '/path with spaces/adapter.json');
    assert.ok(config.hooks.PostToolUse[0].hooks[0].command.includes('--config'));
    assert.equal(config.hooks.PostToolUse[0].hooks[0].async, undefined);
    assert.equal(config.hooks.Stop.length, 1);
    assert.throws(() => create('/bad$path/cli.js', '/config.json'), /shell/);
  }
  assert.ok(new RegExp(codexHooks('/cli.js', '/config.json').hooks.PostToolUse[0].matcher).test('apply_patch'));
});

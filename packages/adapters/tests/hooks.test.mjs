import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFile, spawn, spawnSync } from 'node:child_process';
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

test('query hook mocks report MSW guidance only in TypeScript test files', async t => {
  const config = { ...await fixture(t), ruleIds: ['no-query-hook-mocking'] };
  const mock = "vi.mock('@tanstack/react-query', () => ({ useQuery: vi.fn() }));";
  for (const name of ['query.test.ts', 'query.test.tsx', 'query.ts']) {
    await writeFile(join(config.workspace, 'src', name), mock);
    const result = await handleHook(config, {
      hook_event_name: 'PostToolUse', tool_name: 'Edit',
      tool_input: { file_path: `src/${name}` },
    });
    if (name === 'query.ts') {
      assert.deepEqual(result, {});
    } else {
      assert.equal(result.decision, 'block');
      assert.match(result.reason, /no-query-hook-mocking/);
      assert.match(result.reason, /MSW/);
      await writeFile(join(config.workspace, 'src', name),
        "server.use(http.get('/projects', () => HttpResponse.json([])));");
    }
  }
  assert.deepEqual(await handleHook(config, { hook_event_name: 'Stop' }), {});
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

test('Stop checks scoped Git changes and repeated Stop has explicit loop protection', async t => {
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

async function commitFixture(config) {
  await exec('git', ['-C', config.workspace, 'add', '.']);
  await exec('git', ['-C', config.workspace, '-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '-m', 'fixture']);
}

test('Stop ignores committed violations and checks staged, unstaged and untracked files', async t => {
  const config = await fixture(t);
  await writeFile(join(config.workspace, 'src/changed.ts'), good);
  await writeFile(join(config.workspace, '.gitignore'), 'src/ignored.ts\n');
  await commitFixture(config);
  await writeFile(join(config.workspace, 'src/ignored.ts'), bad);
  assert.deepEqual(await handleHook(config, { hook_event_name: 'Stop' }), {});
  await writeFile(join(config.workspace, 'src/changed.ts'), bad);
  let result = await handleHook(config, { hook_event_name: 'Stop' });
  assert.equal(result.decision, 'block');
  assert.match(result.reason, /changed\.ts/);
  assert.ok(!result.reason.includes('file with spaces'));
  await exec('git', ['-C', config.workspace, 'add', 'src/changed.ts']);
  assert.equal((await handleHook(config, { hook_event_name: 'Stop' })).decision, 'block');
  await writeFile(join(config.workspace, 'src/changed.ts'), good);
  assert.deepEqual(await handleHook(config, { hook_event_name: 'Stop' }), {}); // Current file contents, not index contents.
  await writeFile(join(config.workspace, 'src/new\nfile.ts'), bad);
  result = await handleHook(config, { hook_event_name: 'Stop' });
  assert.equal(result.decision, 'block');
  assert.ok(result.reason.includes('new\nfile.ts'));
  await rm(join(config.workspace, 'src/new\nfile.ts'));
  await rm(join(config.workspace, 'src/file with spaces.tsx'));
  assert.deepEqual(await handleHook(config, { hook_event_name: 'Stop' }), {});
});

test('Stop supports staged renames and repository subdirectories', async t => {
  const config = await fixture(t);
  await commitFixture(config);
  await exec('git', ['-C', config.workspace, 'mv', 'src/file with spaces.tsx', 'src/renamed.tsx']);
  const scoped = { ...config, workspace: join(config.workspace, 'src'), sourceRoots: ['.'] };
  const result = await handleHook(scoped, { hook_event_name: 'Stop' });
  assert.equal(result.decision, 'block');
  assert.match(result.reason, /renamed\.tsx/);
});

test('Stop skips non-Git projects without scanning their sources', async t => {
  const workspace = await mkdtemp(join(tmpdir(), 'ai-lint-non-git-'));
  t.after(() => rm(workspace, { recursive: true, force: true }));
  await writeFile(join(workspace, 'bad.ts'), bad);
  const result = await handleHook({ workspace, binary, sourceRoots: ['.'], timeoutMs: 10000 }, { hook_event_name: 'Stop' });
  assert.equal(result.decision, undefined);
  assert.match(result.systemMessage, /Git 저장소가 아니므로/);
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

test('CLI separates configuration and stdin errors without exposing their contents', async t => {
  const config = await fixture(t);
  const path = join(config.workspace, 'adapter.json');
  const payload = JSON.stringify({ hook_event_name: 'PostToolUse', tool_name: 'Read' });
  const invoke = input => {
    const result = spawnSync(process.execPath, [entry, '--config', path], { input, encoding: 'utf8' });
    assert.equal(result.status, 0);
    assert.equal(result.stderr, '');
    assert.ok(!result.stdout.includes('secret-value'));
    return JSON.parse(result.stdout);
  };
  assert.match(invoke(payload).reason, /configuration file not found/);
  await writeFile(path, '{secret-value');
  assert.match(invoke(payload).reason, /configuration file contains invalid JSON/);
  await writeFile(path, JSON.stringify({ ...config, ruleFiles: ['secret-value'] }));
  assert.match(invoke(payload).reason, /Legacy ruleFiles.*reinstall/);
  await writeFile(path, JSON.stringify({ ...config, timeoutMs: 'secret-value' }));
  assert.match(invoke(payload).reason, /configuration schema/);
  await writeFile(path, '\uFEFF' + JSON.stringify(config));
  assert.match(invoke('secret-value').reason, /stdin JSON/);
  assert.match(invoke('').reason, /stdin JSON/);
  assert.deepEqual(invoke('\uFEFF' + payload), {});
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

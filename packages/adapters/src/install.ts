import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { randomUUID } from 'node:crypto';
import { homedir } from 'node:os';
import { mkdir, readFile, realpath, rename, rm, stat, writeFile } from 'node:fs/promises';
import { dirname, isAbsolute, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { claudeCodeHooks, codexHooks } from './config.js';
import { loadHookConfig, type HookConfig } from './hooks.js';

export interface InstallOptions {
  global?: boolean;
  workspace?: string;
  agent?: 'claude-code' | 'codex' | 'both';
  binary?: string;
  sourceRoots?: string[];
  ruleIds?: string[];
  envFile?: string;
  timeoutMs?: number;
  hookTimeoutSeconds?: number;
  dryRun?: boolean;
}
export interface InstallResult { changed: string[]; backups: string[]; dryRun: boolean }
type JsonObject = Record<string, unknown>;
const marker = 'ai-lint managed hook';

function object(value: unknown): value is JsonObject { return typeof value === 'object' && value !== null && !Array.isArray(value); }
function within(root: string, path: string) {
  const part = relative(root, path);
  return part !== '..' && !part.startsWith('../') && !part.startsWith('..\\') && !isAbsolute(part);
}
async function readOptional(path: string) {
  try { return await readFile(path, 'utf8'); }
  catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return null; throw error; }
}
async function checkTarget(workspace: string, path: string) {
  let parent = path;
  for (;;) {
    try {
      if (!within(workspace, await realpath(parent))) throw new Error('Installation target resolves outside workspace');
      return;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
      const next = dirname(parent);
      if (next === parent) throw new Error('Cannot resolve installation target');
      parent = next;
    }
  }
}
async function atomicWrite(path: string, content: string) {
  await mkdir(dirname(path), { recursive: true });
  const temporary = `${path}.${randomUUID()}.tmp`;
  try { await writeFile(temporary, content, { flag: 'wx' }); await rename(temporary, path); }
  finally { await rm(temporary, { force: true }); }
}

function mergeSettings(text: string | null, generated: ReturnType<typeof codexHooks>): string {
  const settings: unknown = text === null ? {} : JSON.parse(text.replace(/^\uFEFF/, ''));
  if (!object(settings) || (settings.hooks !== undefined && !object(settings.hooks))) throw new Error('Existing hook settings must be a JSON object');
  const result = { ...settings };
  const hooks: JsonObject = { ...(settings.hooks as JsonObject | undefined) };
  for (const event of ['PostToolUse', 'Stop'] as const) {
    const groups = hooks[event] ?? [];
    if (!Array.isArray(groups)) throw new Error(`Existing ${event} hooks must be an array`);
    const addition = generated.hooks[event][0];
    const command = addition.hooks[0].command;
    const kept = [];
    for (const group of groups) {
      if (!object(group) || !Array.isArray(group.hooks) || !group.hooks.every(object)) throw new Error(`Invalid existing ${event} hook group`);
      const handlers = group.hooks.filter(handler => !(handler.type === 'command' && (handler.statusMessage === marker || handler.command === command)));
      if (handlers.length) kept.push({ ...group, hooks: handlers });
    }
    kept.push({ ...addition, hooks: addition.hooks.map(handler => ({ ...handler, statusMessage: marker })) });
    hooks[event] = kept;
  }
  result.hooks = hooks;
  return `${JSON.stringify(result, null, 2)}\n`;
}

/** Validates all files before writing; global installation currently supports Claude Code only. */
export async function installHooks(options: InstallOptions = {}): Promise<InstallResult> {
  if (options.global && options.agent !== 'claude-code') throw new Error('Global installation requires --agent claude-code');
  if (options.global && options.workspace) throw new Error('--global cannot be combined with --workspace');
  if (options.global && options.sourceRoots) throw new Error('Global installation uses the current project root; omit --source-root');
  const workspace = await realpath(resolve(options.global ? homedir() : options.workspace ?? process.cwd()));
  const agent = options.agent ?? 'both';
  if (!['claude-code', 'codex', 'both'].includes(agent)) throw new Error('agent must be claude-code, codex, or both');
  const entry = fileURLToPath(new URL('./cli.js', import.meta.url));
  const repository = fileURLToPath(new URL('../../../', import.meta.url));
  const configPath = join(workspace, options.global ? '.claude/ai-lint' : '.ai-lint', 'adapter.json');
  await checkTarget(workspace, configPath);
  const oldConfig = await readOptional(configPath);
  const previous = oldConfig === null ? undefined : await loadHookConfig(configPath, options.ruleIds);
  if (previous && await realpath(previous.workspace) !== workspace) throw new Error('Existing adapter config targets a different workspace');
  const config: HookConfig = {
    workspace,
    ...(options.global ? { workspaceFromCwd: true } : {}),
    binary: options.binary ? resolve(workspace, options.binary) : previous?.binary ?? join(repository, 'target/release', process.platform === 'win32' ? 'ai-lint.exe' : 'ai-lint'),
    sourceRoots: options.sourceRoots ?? previous?.sourceRoots ?? ['.'],
    ruleIds: options.ruleIds ?? previous?.ruleIds,
    envFile: options.envFile ?? previous?.envFile ?? '.env',
    timeoutMs: options.timeoutMs ?? previous?.timeoutMs ?? 60000,
  };
  if (!Number.isInteger(config.timeoutMs) || config.timeoutMs < 1 || config.timeoutMs > 3600000) throw new Error('Invalid timeoutMs');
  if (!(await stat(resolve(workspace, config.binary))).isFile() || !(await stat(entry)).isFile()) throw new Error('Build the Rust CLI and adapter before installing');
  if (!config.sourceRoots.length) throw new Error('sourceRoots must not be empty');
  for (const root of config.sourceRoots) {
    if (!within(workspace, await realpath(resolve(workspace, root)))) throw new Error('sourceRoots must stay inside workspace');
  }
  if (config.ruleIds?.length) {
    const { stdout } = await promisify(execFile)(resolve(workspace, config.binary), ['rules'], { timeout: 10000 });
    const available = new Set(stdout.trim().split(/\r?\n/));
    if (new Set(config.ruleIds).size !== config.ruleIds.length || config.ruleIds.some(id => !available.has(id))) throw new Error('Unknown or duplicate rule ID; run ai-lint rules');
  }
  const timeout = options.hookTimeoutSeconds ?? Math.ceil(config.timeoutMs / 1000) + 30;
  const plan = [{ path: configPath, old: oldConfig, content: `${JSON.stringify(config, null, 2)}\n` }];
  for (const host of agent === 'both' ? ['claude-code', 'codex'] : [agent]) {
    const path = join(workspace, host === 'codex' ? '.codex/hooks.json' : '.claude/settings.json');
    await checkTarget(workspace, path);
    const old = await readOptional(path);
    const generated = (host === 'codex' ? codexHooks : claudeCodeHooks)(entry, configPath, timeout);
    plan.push({ path, old, content: mergeSettings(old, generated) });
  }
  const changed = plan.filter(file => file.old !== file.content);
  if (options.dryRun || !changed.length) return { changed: changed.map(file => file.path), backups: [], dryRun: options.dryRun ?? false };
  const backupRoot = join(workspace, options.global ? '.claude/ai-lint/backups' : '.ai-lint/backups', `${Date.now()}-${randomUUID()}`);
  await checkTarget(workspace, backupRoot);
  const backups = [];
  for (const file of changed) {
    if (file.old !== null) {
      const backup = join(backupRoot, relative(workspace, file.path));
      await atomicWrite(backup, file.old);
      backups.push(backup);
    }
  }
  const written = [];
  try {
    for (const file of changed) {
      if (await readOptional(file.path) !== file.old) throw new Error('Configuration changed during installation; retry');
      await atomicWrite(file.path, file.content);
      written.push(file);
    }
  } catch (error) {
    for (const file of written.reverse()) {
      if (file.old === null) await rm(file.path, { force: true });
      else await atomicWrite(file.path, file.old);
    }
    throw error;
  }
  return { changed: changed.map(file => file.path), backups, dryRun: false };
}

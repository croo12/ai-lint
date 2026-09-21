import { readFile, realpath, stat } from 'node:fs/promises';
import { dirname, extname, isAbsolute, relative, resolve } from 'node:path';
import { z } from 'zod';
import { AiLintAdapter, type LintReport } from './runner.js';
import { changedFiles } from './git-changes.js';
import { changedSince, fingerprints, readSnapshot, snapshotPath, writeSnapshot } from './change-snapshot.js';

const configSchema = z.object({
  workspace: z.string().min(1), binary: z.string().min(1),
  workspaceFromCwd: z.boolean().optional(),
  ruleIds: z.array(z.string().min(1)).optional(), envFile: z.string().min(1).optional(),
  sourceRoots: z.array(z.string().min(1)).min(1).default(['.']),
  timeoutMs: z.number().int().min(1).max(3600000).default(60000),
}).strict();
export type HookConfig = z.infer<typeof configSchema>;
export type HookOutput = { decision?: 'block'; reason?: string; systemMessage?: string };

/** Messages are deliberately limited to fixed diagnostics, never raw config values. */
export class HookConfigError extends Error {}

export async function loadHookConfig(path: string, replacementRuleIds?: string[]): Promise<HookConfig> {
  let text: string;
  try { text = await readFile(path, 'utf8'); }
  catch (error) {
    // readFile rejects with a plain Error whose ErrnoException code separates a missing
    // file from an unreadable one; the two need different installation advice.
    throw new HookConfigError((error as NodeJS.ErrnoException).code === 'ENOENT'
      ? 'configuration file not found; check the hook --config path'
      : 'configuration file cannot be read; check its permissions and --config path');
  }
  let raw: unknown;
  try { raw = JSON.parse(text.replace(/^\uFEFF/, '')); }
  catch { throw new HookConfigError('configuration file contains invalid JSON'); }
  if (raw && typeof raw === 'object' && 'ruleFiles' in raw) {
    if (!replacementRuleIds) throw new HookConfigError('Legacy ruleFiles configuration; reinstall this hook with --rules ID');
    delete raw.ruleFiles;
    Object.assign(raw, { ruleIds: replacementRuleIds });
  }
  const parsed = configSchema.safeParse(raw);
  if (!parsed.success) throw new HookConfigError('configuration schema is invalid; reinstall this hook to generate a valid adapter.json');
  const config = parsed.data;
  config.workspace = resolve(dirname(resolve(path)), config.workspace);
  return config;
}

function within(root: string, path: string) {
  const part = relative(root, path);
  return part !== '..' && !part.startsWith('../') && !part.startsWith('..\\') && !isAbsolute(part);
}
function source(path: string) { return ['.ts', '.tsx', '.js', '.jsx', '.mts', '.cts', '.mjs', '.cjs'].includes(extname(path).toLowerCase()); }

const excludedDirectories = new Set(['node_modules', '.git', 'target', 'dist', 'build', '.next', 'coverage', 'test-results', 'playwright-report']);

function scoped(workspace: string, paths: string[]) {
  return paths.filter(path => source(path) && !relative(workspace, path).split(/[\\/]/).slice(0, -1).some(part => excludedDirectories.has(part)));
}

const toolInputSchema = z.object({ file_path: z.string().optional(), command: z.string().optional() }).passthrough();

function feedback(reports: LintReport[]) {
  const lines = ['ai-lint 검사에 실패했습니다. 아래 오류를 수정한 뒤 다시 검증하세요.'];
  for (const report of reports) {
    for (const error of report.errors) lines.push(error);
    for (const file of report.files) {
      for (const error of file.syntaxErrors) lines.push(`${file.path}: ${error}`);
      for (const violation of file.violations) lines.push(`${file.path}:${violation.line}:${violation.column} [${violation.ruleId}] ${violation.message}`);
    }
  }
  const text = lines.join('\n');
  return text.length > 20000 ? `${text.slice(0, 20000)}\n... 추가 결과 생략` : text;
}

/**
 * Both hosts use command-hook stdin JSON and decision:block stdout JSON.
 * `stateDirectory` holds the per-workspace change snapshot that tells a shell event
 * apart from one that changed nothing; without it every Git change is rechecked.
 */
export async function handleHook(configInput: HookConfig, payload: unknown, stateDirectory?: string): Promise<HookOutput> {
  try {
    const config = configSchema.parse(configInput);
    const input = z.object({ hook_event_name: z.string(), cwd: z.string().optional(),
      tool_name: z.string().optional(), tool_input: z.unknown().optional(), stop_hook_active: z.boolean().optional(),
    }).passthrough().parse(payload);
    if (!['PostToolUse', 'Stop'].includes(input.hook_event_name)) return {};
    if (config.workspaceFromCwd && (!input.cwd || !isAbsolute(input.cwd))) throw new Error('Global hook requires an absolute cwd');
    const workspace = await realpath(config.workspaceFromCwd ? input.cwd! : config.workspace);
    const roots = await Promise.all(config.sourceRoots.map(root => realpath(resolve(workspace, root))));
    if (roots.some(root => !within(workspace, root))) throw new Error('sourceRoots must stay inside workspace');
    const cwd = input.cwd ? await realpath(input.cwd) : workspace;
    if (!within(workspace, cwd)) throw new Error('Hook cwd is outside configured workspace');
    const snapshot = stateDirectory ? snapshotPath(stateDirectory, workspace) : null;
    let candidates: string[];
    if (input.hook_event_name === 'Stop') {
      const changed = await changedFiles(workspace);
      if (changed === null) return { systemMessage: 'ai-lint: Git 저장소가 아니므로 Stop 변경 파일 검사를 건너뜁니다.' };
      candidates = scoped(workspace, changed);
      if (snapshot) await writeSnapshot(snapshot, await fingerprints(candidates));
    } else {
      const tool = input.tool_name ?? '';
      if (!['Write', 'Edit', 'MultiEdit', 'apply_patch', 'Bash', 'exec_command', 'write_stdin'].includes(tool)) return {};
      const parsed = toolInputSchema.safeParse(input.tool_input);
      const data = parsed.success ? parsed.data : {};
      if (data.file_path !== undefined) {
        candidates = [resolve(cwd, data.file_path)];
        // The event names the file it wrote, so mark it as already checked and keep
        // the next shell event from reporting this same edit as its own change.
        if (snapshot) await writeSnapshot(snapshot, { ...await readSnapshot(snapshot), ...await fingerprints(candidates.filter(source)) });
      } else {
        // A shell or patch changes files the event never names. Compare the Git working
        // tree with the snapshot from the previous event and take only what this command
        // touched; a patch body also names its own targets for non-Git workspaces.
        const patch = data.command ?? (typeof input.tool_input === 'string' ? input.tool_input : '');
        candidates = ['apply_patch', 'Edit', 'Write'].includes(tool)
          ? [...patch.matchAll(/^\*\*\* (?:Add File|Update File|Move to): (.+)\r?$/gm)].map(match => resolve(cwd, match[1].trim()))
          : [];
        const changed = await changedFiles(workspace);
        if (changed !== null) {
          const current = await fingerprints(scoped(workspace, changed));
          candidates.push(...changedSince(snapshot ? await readSnapshot(snapshot) : {}, current));
          if (snapshot) await writeSnapshot(snapshot, current);
        }
        if (!candidates.length) return {};
      }
    }
    const files = [];
    for (const path of new Set(candidates)) {
      if (!source(path) || !roots.some(root => within(root, path))) continue;
      try {
        const canonical = await realpath(path);
        if (!within(workspace, canonical) || !roots.some(root => within(root, canonical))) throw new Error('Source symlink leaves the configured source scope');
        if ((await stat(canonical)).isFile()) files.push(canonical);
      } catch (error) {
        // A candidate deleted between the tool call and this check is not a failure,
        // and only the ErrnoException code distinguishes it from a real scope error.
        if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
      }
    }
    const adapter = new AiLintAdapter({ ...config, workspace });
    const reports: LintReport[] = [];
    for (let index = 0; index < files.length; index += 100) reports.push(await adapter.check(files.slice(index, index + 100)));
    if (!reports.some(report => report.exitCode !== 0)) return {};
    if (input.hook_event_name === 'Stop' && input.stop_hook_active) {
      return { systemMessage: `ai-lint: 재검사에서도 오류가 남았습니다. 무한 반복을 막기 위해 이번 종료는 차단하지 않습니다. 미해결 오류를 최종 응답에 명시하세요.\n${feedback(reports)}` };
    }
    return { decision: 'block', reason: feedback(reports) };
  } catch (error) {
    return { decision: 'block', reason: `ai-lint hook 실행 오류: ${error instanceof Error ? error.message : 'unknown error'}` };
  }
}

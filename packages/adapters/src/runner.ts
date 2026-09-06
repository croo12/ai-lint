import { execFile } from 'node:child_process';
import { realpath, stat } from 'node:fs/promises';
import { extname, isAbsolute, relative, resolve } from 'node:path';
import { z } from 'zod';

const reportSchema = z.object({
  schemaVersion: z.literal(1),
  exitCode: z.union([z.literal(0), z.literal(1), z.literal(2)]),
  errors: z.array(z.string()),
  files: z.array(z.object({
    path: z.string(), syntaxErrors: z.array(z.string()),
    violations: z.array(z.object({ ruleId: z.string(), message: z.string(), start: z.number().int().nonnegative(), end: z.number().int().nonnegative(), line: z.number().int().positive(), column: z.number().int().positive() })),
  })),
});
export type LintReport = z.infer<typeof reportSchema>;
export interface AdapterOptions {
  workspace: string;
  binary: string;
  ruleFiles?: string[];
  envFile?: string;
  timeoutMs?: number;
}

/** The executable, rule paths and env file are configured by the host, not tool input. */
export class AiLintAdapter {
  readonly options: AdapterOptions;
  constructor(options: AdapterOptions) {
    if (!options.workspace || !options.binary) throw new Error('workspace and binary are required');
    const timeoutMs = options.timeoutMs ?? 60000;
    if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 3600000) throw new Error('timeoutMs must be between 1 and 3600000');
    const workspace = resolve(options.workspace);
    this.options = { workspace, binary: resolve(workspace, options.binary), ruleFiles: options.ruleFiles?.map(path => resolve(workspace, path)), envFile: resolve(workspace, options.envFile ?? '.env'), timeoutMs };
  }

  async check(files: string[], signal?: AbortSignal): Promise<LintReport> {
    if (!Array.isArray(files) || !files.length || files.length > 100) throw new Error('Provide between 1 and 100 source files');
    const workspace = await realpath(this.options.workspace);
    const paths = [];
    for (const file of files) {
      if (typeof file !== 'string' || !file || file.includes('\0')) throw new Error('Invalid source path');
      const path = await realpath(resolve(workspace, file));
      const inside = relative(workspace, path);
      if (inside === '..' || inside.startsWith('../') || inside.startsWith('..\\') || isAbsolute(inside)) throw new Error('Source files must be inside the configured workspace');
      if (!['.js', '.jsx', '.ts', '.tsx', '.mjs', '.cjs', '.mts', '.cts'].includes(extname(path).toLowerCase()) || !(await stat(path)).isFile()) throw new Error('Expected a JavaScript or TypeScript source file');
      paths.push(path);
    }
    const args = ['check', '--format', 'json', '--env-file', this.options.envFile!];
    for (const rule of this.options.ruleFiles ?? []) args.push('--rules', rule);
    args.push('--', ...new Set(paths));
    const { stdout, code } = await new Promise<{ stdout: string; code: number }>((accept, reject) => {
      execFile(this.options.binary, args, { cwd: workspace, timeout: this.options.timeoutMs, maxBuffer: 4 * 1024 * 1024, encoding: 'utf8', windowsHide: true, signal }, (error, stdout) => {
        if (error && error.code !== 1 && error.code !== 2) {
          reject(new Error(error.code === 'ENOENT' ? 'ai-lint executable not found; build the CLI and configure its binary path' : 'ai-lint execution failed, was cancelled, timed out, or exceeded the output limit'));
        } else accept({ stdout, code: error ? Number(error.code) : 0 });
      });
    });
    let report: LintReport;
    try { report = reportSchema.parse(JSON.parse(stdout)); } catch { throw new Error('Invalid ai-lint JSON output; rebuild the CLI with --format json support'); }
    if (report.exitCode !== code) throw new Error('ai-lint output and process exit code disagree');
    return report;
  }
}

import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { resolve } from 'node:path';

const exec = promisify(execFile);

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await exec('git', ['-C', cwd, ...args], {
    encoding: 'utf8', timeout: 10000, maxBuffer: 4 * 1024 * 1024,
    env: { ...process.env, LC_ALL: 'C', GIT_OPTIONAL_LOCKS: '0' },
  });
  return stdout;
}

/** null means no Git repository. Other failures must not be treated as a clean diff. */
export async function changedFiles(workspace: string): Promise<string[] | null> {
  let root: string;
  try { root = (await git(workspace, ['rev-parse', '--show-toplevel'])).replace(/\r?\n$/, ''); }
  catch (error) {
    const failure = error as { code?: number | string; stderr?: string };
    if (failure.code === 128 && failure.stderr?.includes('not a git repository')) return null;
    throw new Error('Cannot determine Git repository for Stop hook');
  }
  try {
    // Separate index/worktree diffs retain staged changes even if the worktree undoes them.
    // --no-renames reports rename destinations as additions; deleted sources are excluded.
    const outputs = await Promise.all([
      git(root, ['diff', '--no-ext-diff', '--no-renames', '--name-only', '-z', '--diff-filter=ACMRTUXB', '--']),
      git(root, ['diff', '--cached', '--no-ext-diff', '--no-renames', '--name-only', '-z', '--diff-filter=ACMRTUXB', '--']),
      git(root, ['ls-files', '--others', '--exclude-standard', '-z', '--']),
    ]);
    return [...new Set(outputs.flatMap(output => output.split('\0').filter(Boolean)))].map(path => resolve(root, path));
  } catch {
    throw new Error('Cannot list Git changes for Stop hook (Git failed, timed out, or exceeded its output limit)');
  }
}

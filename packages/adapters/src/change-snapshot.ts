import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readFile, rename, rm, stat, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { z } from 'zod';

const snapshotSchema = z.record(z.string(), z.string());
export type Snapshot = z.infer<typeof snapshotSchema>;

function errorCode(error: unknown) {
  return typeof error === 'object' && error !== null && 'code' in error ? error.code : undefined;
}

/** One snapshot per workspace; the hashed name keeps arbitrary paths out of the file name. */
export function snapshotPath(directory: string, workspace: string) {
  return join(directory, `${createHash('sha256').update(workspace).digest('hex').slice(0, 32)}.json`);
}

/** A modification time can repeat within its millisecond, so the size is part of each mark. */
export async function fingerprints(paths: string[]): Promise<Snapshot> {
  const marks = await Promise.all(paths.map(async path => {
    try {
      const info = await stat(path);
      return info.isFile() ? { path, mark: `${info.mtimeMs}:${info.size}` } : null;
    } catch (error) {
      if (errorCode(error) !== 'ENOENT') throw error;
      return null;
    }
  }));
  return Object.fromEntries(marks.filter(mark => mark !== null).map((mark): [string, string] => [mark.path, mark.mark]));
}

export function changedSince(previous: Snapshot, current: Snapshot) {
  return Object.keys(current).filter(path => previous[path] !== current[path]);
}

/** A missing, unreadable or unwritable snapshot widens the next check; it never skips one. */
export async function readSnapshot(path: string): Promise<Snapshot> {
  try {
    const parsed = snapshotSchema.safeParse(JSON.parse(await readFile(path, 'utf8')));
    return parsed.success ? parsed.data : {};
  } catch { return {}; }
}

export async function writeSnapshot(path: string, snapshot: Snapshot) {
  const temporary = `${path}.${randomUUID()}.tmp`;
  try {
    await mkdir(dirname(path), { recursive: true });
    await writeFile(temporary, `${JSON.stringify(snapshot)}\n`, { flag: 'wx' });
    await rename(temporary, path);
  } catch {
    await rm(temporary, { force: true }).catch(() => undefined);
  }
}

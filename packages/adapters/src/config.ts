import { resolve } from 'node:path';

function quotePath(path: string) {
  const absolute = resolve(path).replaceAll('\\', '/');
  if (/["\r\n$`%!]/.test(absolute)) throw new Error('Hook command paths cannot contain shell expansion characters');
  return `"${absolute}"`;
}
function hooks(entry: string, config: string, matcher: string, timeout: number) {
  if (!Number.isInteger(timeout) || timeout < 1) throw new Error('Hook timeout must be a positive number of seconds');
  const handler = { type: 'command', command: `node ${quotePath(entry)} --config ${quotePath(config)}`, timeout };
  return { hooks: { PostToolUse: [{ matcher, hooks: [handler] }], Stop: [{ hooks: [{ ...handler }] }] } };
}
/** Merge into .claude/settings.json; does not mutate agent configuration. */
export function claudeCodeHooks(entry: string, config: string, timeoutSeconds = 90) {
  return hooks(entry, config, '^(Write|Edit|MultiEdit|Bash)$', timeoutSeconds);
}
/** Merge into .codex/hooks.json; requires a Codex version with lifecycle hooks. */
export function codexHooks(entry: string, config: string, timeoutSeconds = 90) {
  return hooks(entry, config, '^(Write|Edit|apply_patch|Bash|exec_command|write_stdin)$', timeoutSeconds);
}

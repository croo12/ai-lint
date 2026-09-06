#!/usr/bin/env node
import { parseArgs } from 'node:util';
import { handleHook, loadHookConfig } from './hooks.js';
import { installHooks } from './install.js';

const installing = process.argv[2] === 'install';

try {
  if (installing) {
    const { values } = parseArgs({ args: process.argv.slice(3), options: {
      agent: { type: 'string', default: 'both' }, workspace: { type: 'string' }, bin: { type: 'string' },
      'source-root': { type: 'string', multiple: true }, rules: { type: 'string', multiple: true },
      'env-file': { type: 'string' }, 'timeout-ms': { type: 'string' }, 'hook-timeout': { type: 'string' },
      'dry-run': { type: 'boolean' }, global: { type: 'boolean' }, help: { type: 'boolean' },
    } });
    if (values.help) {
      process.stdout.write('ai-lint-hook install [--agent both|claude-code|codex] [--global (claude-code only)] [--workspace PATH] [--bin PATH] [--source-root PATH ...] [--rules FILE ...] [--env-file FILE] [--timeout-ms 60000] [--hook-timeout 90] [--dry-run]\n');
    } else {
      const result = await installHooks({ agent: values.agent as 'both' | 'claude-code' | 'codex', workspace: values.workspace,
        global: values.global, binary: values.bin, sourceRoots: values['source-root'], ruleFiles: values.rules, envFile: values['env-file'],
        timeoutMs: values['timeout-ms'] ? Number(values['timeout-ms']) : undefined,
        hookTimeoutSeconds: values['hook-timeout'] ? Number(values['hook-timeout']) : undefined, dryRun: values['dry-run'] });
      process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
      if (!result.dryRun) process.stdout.write('설정 완료. 에이전트를 다시 열고 /hooks에서 등록 상태와 신뢰 승인을 확인하세요.\n');
    }
  } else {
  const { values } = parseArgs({ options: { config: { type: 'string' }, help: { type: 'boolean' } } });
  if (values.help) {
    process.stderr.write('ai-lint-hook --config PATH < hook-input.json\n');
  } else {
    if (!values.config) throw new Error('--config is required');
    const chunks: Buffer[] = [];
    let size = 0;
    for await (const chunk of process.stdin) {
      const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      size += buffer.length;
      if (size > 2 * 1024 * 1024) throw new Error('Hook input exceeds 2 MiB');
      chunks.push(buffer);
    }
    const output = await handleHook(await loadHookConfig(values.config), JSON.parse(Buffer.concat(chunks).toString('utf8')));
    process.stdout.write(`${JSON.stringify(output)}\n`);
  }
  }
} catch (error) {
  if (installing) {
    process.stderr.write(`ai-lint install failed: ${error instanceof Error ? error.message : 'unknown error'}\n`);
    process.exitCode = 1;
  } else {
  process.stdout.write(`${JSON.stringify({ decision: 'block', reason: 'ai-lint hook: invalid configuration or stdin JSON; check --config and rebuild the adapter' })}\n`);
  }
}

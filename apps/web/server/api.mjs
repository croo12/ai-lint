import { execFile } from 'node:child_process';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const repository = fileURLToPath(new URL('../../../', import.meta.url));
const catalog = [
  { id: 'no-set-state-in-effect', name: 'Effect 안의 상태 변경', description: 'useEffect 콜백에 포함된 state setter 호출을 찾습니다.', tag: 'React', file: join(repository, 'rules/no-set-state-in-effect.yaml') },
  { id: 'no-console-log', name: '디버깅 로그', description: '코드에 남아 있는 console.log 호출을 찾습니다.', tag: 'Quality', file: join(root, 'rules/no-console-log.yaml') },
  { id: 'no-alert', name: '브라우저 알림', description: 'alert와 window.alert 호출을 찾습니다.', tag: 'UX', file: join(root, 'rules/no-alert.yaml') },
];

function respond(res, status, body) {
  res.writeHead(status, { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' });
  res.end(JSON.stringify(body));
}

async function inspect(source, selected) {
  const folder = await mkdtemp(join(tmpdir(), 'ai-lint-playground-'));
  try {
    const file = join(folder, 'playground.tsx');
    await writeFile(file, source, 'utf8');
    const executable = join(repository, 'target/debug', process.platform === 'win32' ? 'ai-lint.exe' : 'ai-lint');
    const args = ['check', '--env-file', join(folder, 'no-model.env')];
    for (const rule of selected) args.push('--rules', rule.file);
    args.push(file);
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('AI_LINT_MODEL_')));
    const result = await new Promise((resolve, reject) => {
      execFile(executable, args, { cwd: repository, env, timeout: 15000, maxBuffer: 1024 * 1024, windowsHide: true, encoding: 'utf8' }, (error, stdout, stderr) => {
        if (error && error.code !== 1) return reject(new Error(error.code === 2 ? stderr.replaceAll(file, 'playground.tsx') : '검사기를 실행할 수 없습니다. cargo build 후 다시 시도하세요.'));
        resolve({ stdout, stderr, exitCode: error ? 1 : 0 });
      });
    });
    const diagnostics = [];
    for (const line of result.stderr.split(/\r?\n/)) {
      const match = line.match(/:(\d+)\.\.(\d+): \[([^\]]+)\] (.*)$/);
      if (!match) continue;
      const prefix = Buffer.from(source).subarray(0, Number(match[1])).toString('utf8');
      const lines = prefix.split('\n');
      diagnostics.push({ ruleId: match[3], message: match[4], line: lines.length, column: [...lines.at(-1)].length + 1 });
    }
    return { ...result, diagnostics, output: (result.stderr || result.stdout).replaceAll(file, 'playground.tsx'), syntaxError: result.exitCode === 1 && diagnostics.length === 0 };
  } finally { await rm(folder, { recursive: true, force: true }); }
}

export async function apiMiddleware(req, res, next) {
  const path = req.url?.split('?')[0];
  if (!path?.startsWith('/api/')) return next();
  // Local development API: browser requests must be same-origin.
  if (req.headers.origin && req.headers.origin !== `http://${req.headers.host}`) return respond(res, 403, { error: '허용되지 않은 요청입니다.' });
  try {
    if (path === '/api/rules' && req.method === 'GET') {
      const rules = await Promise.all(catalog.map(async ({ file, ...rule }) => ({ ...rule, yaml: await readFile(file, 'utf8') })));
      return respond(res, 200, { rules });
    }
    if (path !== '/api/check' || req.method !== 'POST') return respond(res, 404, { error: '요청을 찾을 수 없습니다.' });
    const chunks = [];
    let size = 0;
    for await (const chunk of req) {
      size += chunk.length;
      if (size > 128 * 1024) return respond(res, 413, { error: '코드는 128 KiB 이하로 입력하세요.' });
      chunks.push(chunk);
    }
    const text = Buffer.concat(chunks).toString('utf8');
    let body;
    try { body = JSON.parse(text); } catch { return respond(res, 400, { error: '잘못된 JSON 요청입니다.' }); }
    if (!body || typeof body.source !== 'string' || !body.source.trim() || !Array.isArray(body.rules) || !body.rules.length || body.rules.some(id => typeof id !== 'string' || !catalog.some(rule => rule.id === id))) {
      return respond(res, 400, { error: '코드와 하나 이상의 유효한 규칙을 선택하세요.' });
    }
    const selected = catalog.filter(rule => body.rules.includes(rule.id));
    return respond(res, 200, await inspect(body.source, selected));
  } catch (error) { return respond(res, 500, { error: error instanceof Error ? error.message : '검사에 실패했습니다.' }); }
}

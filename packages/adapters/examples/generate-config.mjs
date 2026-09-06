import { claudeCodeHooks, codexHooks } from '../dist/index.js';

const [agent, entry, config] = process.argv.slice(2);
if (!['claude-code', 'codex'].includes(agent) || !entry || !config) {
  throw new Error('Usage: node generate-config.mjs claude-code|codex /absolute/dist/cli.js /absolute/adapter.config.json');
}
console.log(JSON.stringify((agent === 'codex' ? codexHooks : claudeCodeHooks)(entry, config), null, 2));

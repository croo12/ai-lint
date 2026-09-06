import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { apiMiddleware } from './server/api.mjs';

export default defineConfig({ plugins: [react(), {
  name: 'ai-lint-api',
  configureServer(server) { server.middlewares.use(apiMiddleware); },
  configurePreviewServer(server) { server.middlewares.use(apiMiddleware); },
}] });

import { fileURLToPath } from 'url';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import { docsPlugin } from './src/docs/plugin';

const root = fileURLToPath(new URL('.', import.meta.url));

export default defineConfig({
  base: '/',
  plugins: [react(), docsPlugin({ contentDir: 'content', root })],
});

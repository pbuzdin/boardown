import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

const here = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  root: here,
  // base './' so the packaged index.html references its assets relatively.
  base: './',
  plugins: [react()],
  // Tauri expects a fixed dev-server port and fails if it is taken.
  server: { port: 1420, strictPort: true },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
});

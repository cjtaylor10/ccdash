import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import path from 'node:path';

export default defineConfig({
  plugins: [
    svelte({
      compilerOptions: {
        // Force client/DOM compilation; mount() is browser-only.
        generate: 'client',
      },
    }),
  ],
  resolve: {
    alias: {
      $lib: path.resolve(__dirname, 'src/lib'),
    },
    conditions: ['browser'],
  },
  build: {
    outDir: 'build',
    emptyOutDir: true,
    target: 'esnext',
  },
  server: {
    port: 1420,
    strictPort: true,
    host: '127.0.0.1',
    hmr: { protocol: 'ws', host: '127.0.0.1', port: 1421 },
    watch: { ignored: ['**/src-tauri/**'] },
  },
  clearScreen: false,
});

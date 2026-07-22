import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import path from 'node:path';

export default defineConfig({
  plugins: [vue()],
  resolve: { alias: { '@': path.resolve(__dirname, './src') } },
  base: './',
  server: { port: 5174, strictPort: true },
  build: { outDir: 'dist', emptyOutDir: true, target: 'esnext' },
});
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

const TAURI_DEV_HOST = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  // Tauri expects a fixed port and disables HMR ping over WebSocket
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: TAURI_DEV_HOST ?? false,
    hmr: TAURI_DEV_HOST
      ? { protocol: 'ws', host: TAURI_DEV_HOST, port: 1421 }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },

  envPrefix: ['VITE_', 'TAURI_ENV_*'],

  build: {
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    rollupOptions: {
      output: {
        // Lazy chunks per-shell to keep initial bundle small.
        // Budget: initial chunk < 350KB gz (enforced in CI).
        manualChunks(id) {
          if (id.includes('/shells/developer/')) return 'shell-developer';
          if (id.includes('/shells/executive/')) return 'shell-executive';
          if (id.includes('/shells/manager/')) return 'shell-manager';
          if (id.includes('/shells/employee/')) return 'shell-employee';
          if (id.includes('/packages/ui-kit/')) return 'ui-kit';
          if (id.includes('/packages/glove-kit/')) return 'glove-kit';
          if (id.includes('/packages/module-sdk/')) return 'module-sdk';
          if (id.includes('/packages/i18n/')) return 'i18n';
          if (id.includes('node_modules/three') || id.includes('@react-three')) return 'three';
          if (id.includes('node_modules/@supabase')) return 'supabase';
          return undefined;
        },
      },
    },
  },

  test: {
    environment: 'happy-dom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
  },
});

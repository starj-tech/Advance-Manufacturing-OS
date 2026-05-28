import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

// Aplikasi IT — for the client company's IT team: account management,
// device/protocol bindings, module install/kill, security & audit. Scaffold.
export default defineConfig({
  plugins: [react()],

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  envPrefix: ['VITE_'],

  build: {
    target: 'es2020',
    minify: 'esbuild',
    sourcemap: false,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('/packages/ui-kit/')) return 'ui-kit';
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

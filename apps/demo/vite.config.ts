import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

// Web SPA build. AETHER-OS ships as a browser application served as a
// static bundle from a CDN/host; backend logic lives in Supabase Edge
// Functions (Deno) reached over HTTP. (Migrated off the Tauri desktop
// shell — no WebView-specific build target or fixed dev port.)
export default defineConfig({
  plugins: [react()],

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  envPrefix: ['VITE_'],

  build: {
    // Broad evergreen-browser baseline (Chrome/Edge 80+, Safari 14+,
    // Firefox 78+). No WebView-specific downleveling.
    target: 'es2020',
    minify: 'esbuild',
    sourcemap: false,
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

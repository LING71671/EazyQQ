import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { resolve } from 'path';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': resolve(__dirname, './src')
    }
  },
  // Ensure Vite only scans the application entry, ignoring reference repos
  optimizeDeps: {
    entries: ['index.html'],
    // Pre-bundle up front. Without this the first page load has to discover these
    // mid-flight, which makes Vite re-optimize and force a full page reload - the
    // main reason the window used to sit on the splash far too long in dev.
    include: [
      'react',
      'react-dom',
      'react-dom/client',
      'lucide-react',
      'clsx',
      'tailwind-merge',
      'zod',
      'qrcode',
      '@tauri-apps/api/core',
      '@tauri-apps/api/event',
      '@tauri-apps/api/window',
      '@tauri-apps/plugin-dialog',
      '@tauri-apps/plugin-fs',
      '@tauri-apps/plugin-shell'
    ]
  },
  // Vite options tailored for Tauri development
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: false,
    // Transform the entry graph during server startup instead of on first request.
    warmup: {
      clientFiles: [
        './src/main.tsx',
        './src/App.tsx',
        './src/components/**/*.tsx',
        './src/views/*.tsx'
      ]
    },
    watch: {
      // Ignore backend, reference repos, and user data directories
      ignored: ['**/src-tauri/**', '**/refer/**', '**/EazyQQ_Data/**']
    }
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    target: 'esnext',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    rollupOptions: {
      output: {
        // Split the heavy vendors so the app shell can paint without waiting on them.
        manualChunks: {
          react: ['react', 'react-dom', 'react-dom/client'],
          icons: ['lucide-react']
        }
      }
    }
  }
});

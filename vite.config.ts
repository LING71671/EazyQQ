import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { resolve } from 'path';
import { appendFileSync } from 'fs';

// https://vitejs.dev/config/

/**
 * Logs every request the dev server receives.
 *
 * When the desktop window shows nothing, the first question is whether the WebView
 * fetched the page at all. Without this the dev server is silent, so "the frontend
 * never loaded" and "the frontend loaded but did not paint" look identical from the
 * outside - which is exactly the ambiguity that made a blank window hard to diagnose.
 */
const requestLogger = (): Plugin => ({
  name: 'eazyqq-request-logger',
  configureServer(server) {
    server.middlewares.use((req, res, next) => {
      const started = Date.now();
      res.on('finish', () => {
        const line = `[vite] ${req.method} ${req.url} (${Date.now() - started}ms)`;
        console.log(line);
        try {
          appendFileSync('EazyQQ_Data/vite_requests.log', line + '\n');
        } catch {
          /* logging must never break the dev server */
        }
      });
      next();
    });
  }
});

export default defineConfig({
  plugins: [requestLogger(), react(), tailwindcss()],
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
    // Bind IPv4 loopback explicitly.
    //
    // `host: false` means "listen on localhost", and Node resolves that to `::1` first, so
    // the dev server ended up listening on IPv6 only - visible as `[::1]:1420` in netstat.
    // WebView2 resolves `localhost` to `127.0.0.1`, found nothing there, and could not load
    // the frontend at all: the window stayed hidden and the app looked broken. Naming the
    // address removes the resolution step from both sides.
    host: '127.0.0.1',
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

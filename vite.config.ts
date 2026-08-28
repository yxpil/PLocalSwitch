import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'node:path';

// React 18 + Vite 配置
// 明确路径别名，严格区分「源码」与「构建产物」
export default defineConfig(async () => ({
  plugins: [react()],

  resolve: {
    alias: {
      '@':          resolve(__dirname, 'src'),
      '@components':resolve(__dirname, 'src/components'),
      '@pages':     resolve(__dirname, 'src/pages'),
      '@stores':    resolve(__dirname, 'src/stores'),
      '@styles':    resolve(__dirname, 'src/styles'),
      '@utils':     resolve(__dirname, 'src/utils'),
      '@types':     resolve(__dirname, 'src/types'),
      '@assets':    resolve(__dirname, 'src/assets'),
      '@public':    resolve(__dirname, 'public'),
      '@i18n':      resolve(__dirname, 'src/i18n'),
      '@commands':  resolve(__dirname, 'src/commands'),
      '@plugins':   resolve(__dirname, 'src/plugins'),
      '@logger':    resolve(__dirname, 'src/logger'),
      '@icons':     resolve(__dirname, 'src/icons'),
    },
  },

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: '127.0.0.1',
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },

  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    sourcemap: false,
    target: 'es2020',
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        tray: resolve(__dirname, 'tray-menu.html'),
      },
      output: {
        manualChunks: {
          'react-vendor': ['react', 'react-dom', 'react-router-dom'],
          'state':        ['zustand'],
          'i18n':         ['i18next', 'react-i18next', 'i18next-browser-languagedetector'],
          'ui-tools':     ['clsx', 'tailwind-merge'],
        },
        chunkFileNames: 'assets/js/[name]-[hash].js',
        entryFileNames: 'assets/js/[name]-[hash].js',
        assetFileNames: (assetInfo: { name?: string }) => {
          const name = assetInfo.name || '';
          if (/\.(css)$/.test(name)) return 'assets/css/[name]-[hash][extname]';
          if (/\.(png|jpe?g|svg|gif|webp|ico)$/.test(name)) return 'assets/img/[name]-[hash][extname]';
          if (/\.(woff2?|eot|ttf|otf)$/.test(name)) return 'assets/fonts/[name]-[hash][extname]';
          return 'assets/other/[name]-[hash][extname]';
        },
      },
    },
  },

  envPrefix: ['VITE_', 'TAURI_'],
}));

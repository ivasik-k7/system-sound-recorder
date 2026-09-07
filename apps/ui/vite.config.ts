import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// The desktop shell loads the built assets from disk, so paths must be relative.
// Port 5173 is fixed because the Tauri config points at it during development.
export default defineConfig({
  plugins: [react()],
  base: './',
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // Tauri ships its own webview, so there is no need to support old browsers.
    target: 'chrome110',
    sourcemap: true,
  },
})

import { readFileSync } from 'node:fs'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// The version comes from package.json rather than being written twice.
// scripts/check-versions.sh keeps that file in step with the Cargo manifests
// and tauri.conf.json, so the About box cannot disagree with the installer.
const pkg = JSON.parse(
  readFileSync(new URL('./package.json', import.meta.url), 'utf8'),
) as { version: string }

// A build reference for bug reports. Stays "dev" locally, so a local build is
// reproducible; CI stamps the commit.
const buildRef = (process.env.GITHUB_SHA ?? '').slice(0, 7) || 'dev'

// The desktop shell loads the built assets from disk, so paths must be relative.
// Port 5173 is fixed because the Tauri config points at it during development.
export default defineConfig({
  plugins: [react()],
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
    __BUILD_REF__: JSON.stringify(buildRef),
  },
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

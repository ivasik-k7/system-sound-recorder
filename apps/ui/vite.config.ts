import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// The version comes from package.json rather than being written twice.
// scripts/check-versions.sh keeps that file in step with the Cargo manifests
// and tauri.conf.json, so the About box cannot disagree with the installer.
const pkg = JSON.parse(
  readFileSync(new URL('./package.json', import.meta.url), 'utf8'),
) as { version: string }

// The commit this was built from, matching what the engine stamps into a
// session manifest. A checkout is the usual case; a build from a source
// archive still knows the commit if CI exported it; otherwise say so rather
// than print something that looks like a commit and is not one.
function buildRef(): string {
  try {
    const hash = execFileSync('git', ['rev-parse', '--short=7', 'HEAD'], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim()
    if (hash) return hash
  } catch {
    // Not a checkout, or no git. Fall through.
  }
  const sha = process.env.GITHUB_SHA ?? ''
  return sha.length >= 7 ? sha.slice(0, 7) : 'unknown'
}

// The desktop shell loads the built assets from disk, so paths must be relative.
// Port 5173 is fixed because the Tauri config points at it during development.
export default defineConfig({
  plugins: [react()],
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
    __BUILD_REF__: JSON.stringify(buildRef()),
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

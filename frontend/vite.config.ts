import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';
import type { Plugin, ProxyOptions } from 'vite';
import { playwright } from '@vitest/browser-playwright';
import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';

import { axumPrefixes } from './proxy-paths.ts';

// Dev-only shared origin (Caddy plays this role in docker): backend paths
// go to Axum, pages stay in SvelteKit. The SSR fetch rewrite in
// hooks.server.ts reads the same variable.
const AXUM_DEV_URL = process.env.AXUM_URL ?? 'http://127.0.0.1:3100';

// File events do not cross Docker's bind mount on every host;
// VITE_POLL_WATCH (set by docker-compose) switches the watcher to polling.
const watch = process.env.VITE_POLL_WATCH ? { usePolling: true, interval: 300 } : undefined;

const proxy: Record<string, ProxyOptions> = {};
for (const prefix of axumPrefixes) {
  proxy[prefix] = { target: AXUM_DEV_URL };
}

// The runtime's composed WebGPU render context is not in its package
// exports yet. Resolve it next to the public webgpu entry so both share one
// module URL, and with it one copy of every class.
const webgpuInternal: Plugin = {
  name: 'cjs-webgpu-internal',
  enforce: 'pre',
  async resolveId(source, importer) {
    if (source !== 'cjs-webgpu-internal') return null;
    const entry = await this.resolve('@carbonenginejs/runtime/trinityal/webgpu', importer, {
      skipSelf: true,
    });
    return entry && { ...entry, id: entry.id.replace(/index\.js(?=\?|$)/, 'internal.js') };
  },
};

// The runtime's classes register themselves with its schema when their
// module runs, but its package.json `sideEffects` list leaves the class files
// out, so a production build drops every class nothing names (EveShip2 among
// them). Keep every runtime module.
const runtimeSideEffects: Plugin = {
  name: 'cjs-runtime-side-effects',
  transform(code, id) {
    if (!id.includes('/@carbonenginejs/runtime/')) return null;
    return { code, map: null, moduleSideEffects: true };
  },
};

export default defineConfig({
  server: { proxy, watch },
  // The runtime resolves workers through import.meta.url and its own "#"
  // imports, which pre-bundling breaks.
  optimizeDeps: { exclude: ['@carbonenginejs/runtime'] },
  plugins: [
    webgpuInternal,
    runtimeSideEffects,
    tailwindcss(),
    sveltekit({
      compilerOptions: {
        // Force runes mode for the project, except for libraries. Can be removed in svelte 6.
        runes: ({ filename }) =>
          filename.split(/[/\\]/).includes('node_modules') ? undefined : true,
      },
      adapter: adapter(),
    }),
  ],
  test: {
    expect: { requireAssertions: true },
    projects: [
      {
        extends: './vite.config.ts',
        test: {
          name: 'client',
          setupFiles: ['./vitest-browser-setup.ts'],
          browser: {
            enabled: true,
            provider: playwright(),
            instances: [{ browser: 'chromium', headless: true }],
          },
          include: ['src/**/*.svelte.{test,spec}.{js,ts}'],
          exclude: ['src/lib/server/**'],
        },
      },
      {
        extends: './vite.config.ts',
        test: {
          name: 'server',
          environment: 'node',
          include: ['src/**/*.{test,spec}.{js,ts}'],
          exclude: ['src/**/*.svelte.{test,spec}.{js,ts}'],
        },
      },
    ],
  },
});

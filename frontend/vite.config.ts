import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// `VITE_BASE` is set by the GitHub Pages workflow (e.g. "/watten/") so the
// built bundle uses absolute paths under the repository sub-path. Locally
// it defaults to "/" which works for `vite dev` and `vite preview`.
const base = process.env.VITE_BASE ?? '/';

export default defineConfig({
  base,
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./test/setup.ts'],
    include: ['test/**/*.test.ts', 'test/**/*.test.tsx'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov', 'json-summary'],
      reportsDirectory: './coverage',
      include: ['src/**/*.{ts,tsx}'],
      exclude: ['src/**/*.d.ts'],
    },
  },
});

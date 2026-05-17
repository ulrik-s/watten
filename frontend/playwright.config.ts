import { defineConfig, devices } from '@playwright/test';
import { fileURLToPath } from 'url';
import path from 'path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  testDir: path.join(__dirname, 'e2e'),
  // Run a fresh build's served files in CI; reuse in dev.
  fullyParallel: false,
  // Run tests serially. Two parallel chromium tabs each running a full
  // WasmGame in `?fast=1` mode saturate the host CPU and the bot turn
  // never finishes in time — `.card.selectable` never appears and the
  // suite drowns in 30s timeouts. The total suite still completes in a
  // few minutes serially.
  workers: 1,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: 'http://localhost:4177',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] },
    },
    {
      name: 'webkit',
      use: { ...devices['Desktop Safari'] },
    },
  ],
  webServer: {
    command: 'npx --yes http-server dist -p 4177 -s',
    port: 4177,
    reuseExistingServer: true,
    cwd: __dirname,
  },
});

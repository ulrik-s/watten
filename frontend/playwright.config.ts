import { defineConfig, devices } from '@playwright/test';
import { fileURLToPath } from 'url';
import path from 'path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  testDir: path.join(__dirname, 'e2e'),
  // Run a fresh build's served files in CI; reuse in dev.
  fullyParallel: false,
  // Two workers gives a good speed/stability tradeoff: at higher
  // parallelism the WasmGame initialisation in each tab competes for the
  // same machine, which surfaces as `.card.selectable` never appearing
  // in time.
  workers: 2,
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

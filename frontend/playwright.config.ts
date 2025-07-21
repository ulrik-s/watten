import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'url';
import path from 'path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  testDir: __dirname,
  webServer: {
    command: 'npx --yes http-server dist -p 4177',
    port: 4177,
    reuseExistingServer: true,
    cwd: __dirname,
  },
});

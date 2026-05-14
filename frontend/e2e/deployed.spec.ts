// Smoke-tests the live GitHub Pages deployment. Skipped by default —
// run with DEPLOYED_URL set, e.g.
//   DEPLOYED_URL=https://ulrik-s.github.io/watten/ npx playwright test deployed.spec.ts
import { test, expect } from '@playwright/test';

const baseURL = process.env.DEPLOYED_URL;

test.describe('deployed site', () => {
  test.skip(!baseURL, 'set DEPLOYED_URL to run');

  test('loads the trainer and replaces the loader', async ({ page }) => {
    test.setTimeout(60000);
    await page.goto(baseURL!);
    // The static fallback shows a #fallback element; once React mounts that
    // node is replaced by the in-game heading.
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('Watten');
    // The Raise / Concede controls only render after the WasmGame is
    // constructed; if they appear, wasm has loaded and the React app is alive.
    await expect(page.getByRole('button', { name: /Raise/ })).toBeVisible({ timeout: 30000 });
    await expect(page.getByRole('button', { name: /Concede/ })).toBeVisible();
    // The fallback container should be gone.
    await expect(page.locator('#fallback')).toHaveCount(0);
  });
});

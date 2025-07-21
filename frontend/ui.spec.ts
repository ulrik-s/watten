import { test, expect } from '@playwright/test';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const baseURL = 'http://localhost:4177';

// Basic sanity check that the built UI loads
// and renders the game title.

test('loads Watten UI', async ({ page }) => {
  test.setTimeout(60000);
  await page.goto(baseURL);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Watten');
});

test('contains root container', async ({ page }) => {
  test.setTimeout(60000);
  await page.goto(baseURL);
  await expect(page.locator('#root')).toHaveCount(1);
});

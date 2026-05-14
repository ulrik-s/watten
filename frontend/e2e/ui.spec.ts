import { test, expect } from '@playwright/test';

test('loads Watten UI', async ({ page }) => {
  test.setTimeout(60000);
  await page.goto('/');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Watten');
});

test('contains root container', async ({ page }) => {
  test.setTimeout(60000);
  await page.goto('/');
  await expect(page.locator('#root')).toHaveCount(1);
});

test('renders raise and concede controls', async ({ page }) => {
  test.setTimeout(60000);
  await page.goto('/');
  await expect(page.getByRole('button', { name: /Raise/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /Concede/ })).toBeVisible();
});

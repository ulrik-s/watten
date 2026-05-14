import { test, expect, Page } from '@playwright/test';

const HAND_CARD = '.hand-slot .card';
const SELECTABLE = '.hand-slot .card.selectable';

async function waitForReady(page: Page) {
  // The loader text in index.html is replaced once React mounts.
  await page.goto('/');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Watten');
  // Wait for at least one playable card to appear — that means wasm has loaded
  // and the bots have advanced to the human's first move.
  await page.locator(SELECTABLE).first().waitFor({ state: 'visible', timeout: 30000 });
}

async function readRoundPoints(page: Page): Promise<number> {
  const text = await page.locator('p', { hasText: /Round worth:/ }).first().innerText();
  const m = text.match(/Round worth:\s*(\d+)/);
  if (!m) throw new Error('Cannot find round-worth: ' + text);
  return parseInt(m[1], 10);
}

async function readScores(page: Page): Promise<{ team1: number; team2: number; target: number }> {
  const text = await page
    .locator('p')
    .filter({ hasText: /Team 1/ })
    .first()
    .innerText();
  const m = text.match(/Team\s*1\s*(\d+)\s*[—-]\s*Team\s*2\s*(\d+)\s*\(to\s*(\d+)\)/);
  if (!m) throw new Error('Cannot parse scores: ' + text);
  return {
    team1: parseInt(m[1], 10),
    team2: parseInt(m[2], 10),
    target: parseInt(m[3], 10),
  };
}

test.describe('widgets', () => {
  test('Raise button increments the round-worth display', async ({ page }) => {
    test.setTimeout(60000);
    await waitForReady(page);
    const before = await readRoundPoints(page);
    await page.getByRole('button', { name: /Raise/ }).click();
    await expect
      .poll(() => readRoundPoints(page), { timeout: 5000 })
      .toBe(before + 1);
  });

  test('Concede ends the round and credits the opponent', async ({ page }) => {
    test.setTimeout(60000);
    await waitForReady(page);
    const before = await readScores(page);
    await page.getByRole('button', { name: /Concede/ }).click();
    // Concede credits team 2 with at least the round-worth (≥ 2).
    await expect
      .poll(async () => (await readScores(page)).team2, { timeout: 10000 })
      .toBeGreaterThanOrEqual(before.team2 + 2);
  });

  test('Clicking a hand card produces a log entry for the human play', async ({ page }) => {
    test.setTimeout(60000);
    await waitForReady(page);
    const beforeLogLines = await page.locator('.log > div').count();
    await page.locator(SELECTABLE).first().click();
    await expect
      .poll(() => page.locator('.log > div').count(), { timeout: 10000 })
      .toBeGreaterThan(beforeLogLines);
    await expect(page.locator('.log').getByText(/Player \d plays /).first()).toBeVisible();
  });

  test('Clicked card disappears from its slot (and other slots stay put)', async ({ page }) => {
    test.setTimeout(60000);
    await waitForReady(page);
    // Find the first selectable slot index and capture every slot's
    // card identity (suit+rank) before the click.
    const slots = page.locator('.hand-slot');
    const total = await slots.count();
    expect(total).toBe(5);

    const before: Array<{ filled: boolean; key: string }> = [];
    let firstSelectableSlot = -1;
    for (let i = 0; i < total; i++) {
      const slot = slots.nth(i);
      const card = slot.locator('.card');
      const placeholder = await slot.locator('.placeholder').count();
      if (placeholder > 0) {
        before.push({ filled: false, key: 'empty' });
        continue;
      }
      const rank = (await card.locator('.rank').innerText()).trim();
      // suit is the <img alt> on a real card; fall back to src path.
      const suitSrc = (await card.locator('img').getAttribute('src')) ?? '';
      before.push({ filled: true, key: `${rank}-${suitSrc}` });
      const isSel = await card.evaluate((el) => el.classList.contains('selectable'));
      if (isSel && firstSelectableSlot < 0) firstSelectableSlot = i;
    }
    expect(firstSelectableSlot).toBeGreaterThanOrEqual(0);

    await slots.nth(firstSelectableSlot).locator('.card.selectable').click();

    // The clicked slot should become a placeholder *immediately* (optimistic).
    await expect(
      slots.nth(firstSelectableSlot).locator('.placeholder')
    ).toBeVisible({ timeout: 2000 });

    // Other previously-filled slots should still hold the same card identity.
    for (let i = 0; i < total; i++) {
      if (i === firstSelectableSlot) continue;
      if (!before[i].filled) continue;
      const card = slots.nth(i).locator('.card');
      const rank = (await card.locator('.rank').innerText()).trim();
      const suitSrc = (await card.locator('img').getAttribute('src')) ?? '';
      expect(`${rank}-${suitSrc}`).toBe(before[i].key);
    }
  });

  test('Win-rate hints render per playable card', async ({ page }) => {
    test.setTimeout(60000);
    await waitForReady(page);
    // Every selectable card should have a sibling rate annotation that
    // ends in '%'. There should be at least one.
    const rates = page.locator('.hand-slot .card-rate');
    const count = await rates.count();
    expect(count).toBeGreaterThan(0);
    let anyPercent = false;
    for (let i = 0; i < count; i++) {
      const t = (await rates.nth(i).innerText()).trim();
      if (t.endsWith('%')) anyPercent = true;
    }
    expect(anyPercent).toBe(true);
  });

  test('Final score banner appears after concede flurry', async ({ page }) => {
    test.setTimeout(180000);
    await waitForReady(page);
    // Concede repeatedly until the game ends. Each concede gives the opponent
    // round_points (>= 2). After a concede the React app waits TRICK_DISPLAY_MS
    // (800ms) before starting the next round, so wait for the button to be
    // enabled again rather than guessing.
    const concede = page.getByRole('button', { name: /Concede/ });
    for (let i = 0; i < 30; i++) {
      const s = await readScores(page);
      if (s.team1 >= s.target || s.team2 >= s.target) break;
      await expect(concede).toBeEnabled({ timeout: 5000 }).catch(() => undefined);
      if (await concede.isDisabled()) break;
      await concede.click();
      // Wait until either the button is re-enabled (next round ready) or the
      // game-over banner appears.
      await Promise.race([
        page.locator('.game-over').waitFor({ state: 'visible', timeout: 5000 }),
        concede.elementHandle().then((h) =>
          page.waitForFunction((el) => el && !(el as HTMLButtonElement).disabled, h, {
            timeout: 5000,
          })
        ),
      ]).catch(() => undefined);
    }
    await expect(page.locator('.game-over')).toBeVisible({ timeout: 10000 });
  });
});

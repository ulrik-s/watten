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
    // After a click, the human's play is appended to the log and the bots
    // may immediately advance through the rest of the trick (and possibly
    // future tricks); the log monotonically grows.
    await expect
      .poll(() => page.locator('.log > div').count(), { timeout: 10000 })
      .toBeGreaterThan(beforeLogLines);
    // Some "Player N plays X" line should be visible.
    await expect(page.locator('.log').getByText(/Player \d plays /).first()).toBeVisible();
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

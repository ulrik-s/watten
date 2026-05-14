import { test, expect, Page } from '@playwright/test';

const SELECTABLE = '.hand-slot .card.selectable';

async function waitForReady(page: Page) {
  await page.goto('/');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Watten');
  await page.locator(SELECTABLE).first().waitFor({ state: 'visible', timeout: 30000 });
}

async function readScores(page: Page) {
  const text = await page.locator('p').filter({ hasText: /Team 1/ }).first().innerText();
  const m = text.match(/Team\s*1\s*(\d+)\s*[—-]\s*Team\s*2\s*(\d+)\s*\(to\s*(\d+)\)/);
  if (!m) throw new Error('Cannot parse scores: ' + text);
  return {
    team1: parseInt(m[1], 10),
    team2: parseInt(m[2], 10),
    target: parseInt(m[3], 10),
  };
}

async function readRoundPoints(page: Page) {
  const text = await page.locator('p', { hasText: /Round worth:/ }).first().innerText();
  const m = text.match(/Round worth:\s*(\d+)/);
  if (!m) throw new Error('Cannot find round-worth: ' + text);
  return parseInt(m[1], 10);
}

async function waitForHumanTurn(page: Page, timeoutMs = 25000): Promise<'turn' | 'gameover'> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if ((await page.locator('.game-over').count()) > 0) return 'gameover';
    const concede = page.getByRole('button', { name: /Concede/ });
    if (await concede.isEnabled()) return 'turn';
    await page.waitForTimeout(150);
  }
  throw new Error('timed out waiting for human turn');
}

/** Wait until Team 2 has answered the raise (accept or fold). Returns the
 *  resolution mode. */
async function waitForRaiseResolution(page: Page, timeoutMs = 10000): Promise<'accepted' | 'folded'> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if ((await page.locator('.log').getByText(/Team 2 accepts/).count()) > 0) return 'accepted';
    if ((await page.locator('.log').getByText(/Team 2 folds/).count()) > 0) return 'folded';
    await page.waitForTimeout(150);
  }
  throw new Error('timed out waiting for raise resolution');
}

test('full game using card clicks, raise-with-response, concede, and the trick-winner UI', async ({ page }) => {
  test.setTimeout(240000);
  await waitForReady(page);

  // === Round 1 — click a card and verify the trick-winner UI ===
  await page.locator(SELECTABLE).first().click();
  await expect(page.getByTestId('trick-winner')).toBeVisible({ timeout: 8000 });
  await expect(page.locator('.trick-slot.winner')).toHaveCount(1);
  await expect(
    page.locator('.log').getByText(/Player \d wins the trick/).first()
  ).toBeVisible();

  await waitForHumanTurn(page);

  // === Raise + response — covers the propose-respond path ===
  const ptsBefore = await readRoundPoints(page);
  const scoresBefore = await readScores(page);
  await page.getByRole('button', { name: /Raise/ }).click();
  await expect(page.locator('.log').getByText(/Team 1 proposes to raise/)).toBeVisible();
  const resolution = await waitForRaiseResolution(page);
  if (resolution === 'accepted') {
    expect(await readRoundPoints(page)).toBe(ptsBefore + 1);
    // Round continues — eventually concede so the round ends predictably.
    await waitForHumanTurn(page);
    const before2 = await readScores(page);
    await page.getByRole('button', { name: /Concede/ }).click();
    await expect
      .poll(async () => (await readScores(page)).team2, { timeout: 10000 })
      .toBeGreaterThanOrEqual(before2.team2 + ptsBefore + 1);
  } else {
    // Folded: Team 1 was awarded the pre-raise points.
    await expect
      .poll(async () => (await readScores(page)).team1, { timeout: 10000 })
      .toBeGreaterThanOrEqual(scoresBefore.team1 + ptsBefore);
  }

  // === Subsequent rounds — drive through concedes and raises ===
  for (let round = 0; round < 25; round++) {
    const state = await waitForHumanTurn(page);
    if (state === 'gameover') break;
    const s = await readScores(page);
    if (s.team1 >= s.target || s.team2 >= s.target) break;

    if (round % 3 === 0) {
      await page.getByRole('button', { name: /Concede/ }).click();
    } else if (round % 3 === 1) {
      // Try a raise, then whichever way it resolves, continue.
      await page.getByRole('button', { name: /Raise/ }).click();
      try {
        const r = await waitForRaiseResolution(page, 8000);
        if (r === 'accepted') {
          await waitForHumanTurn(page);
          const c = page.getByRole('button', { name: /Concede/ });
          if (await c.isEnabled()) await c.click();
        }
        // folded → round already ended, loop continues.
      } catch {
        /* timeout: continue anyway */
      }
    } else {
      // Play a card then concede.
      await page.locator(SELECTABLE).first().click();
      const next = await waitForHumanTurn(page);
      if (next === 'gameover') break;
      const c = page.getByRole('button', { name: /Concede/ });
      if (await c.isEnabled()) await c.click();
    }
  }

  // === Game must be over ===
  await expect(page.locator('.game-over')).toBeVisible({ timeout: 30000 });
  const final = await readScores(page);
  expect(Math.max(final.team1, final.team2)).toBeGreaterThanOrEqual(final.target);

  // The full log is kept in the DOM, so historical events from every
  // feature should be present.
  await expect(page.locator('.log').getByText(/proposes to raise/).first()).toHaveCount(1);
  // At least one of accept / fold must have been logged.
  const accepts = await page.locator('.log').getByText(/Team 2 accepts/).count();
  const folds = await page.locator('.log').getByText(/Team 2 folds/).count();
  expect(accepts + folds).toBeGreaterThan(0);
  await expect(page.locator('.log').getByText(/concedes/).first()).toHaveCount(1);
  await expect(page.locator('.log').getByText(/Player \d wins the trick/).first()).toHaveCount(1);
  await expect(page.locator('.log').getByText(/wins the game/).first()).toHaveCount(1);
});

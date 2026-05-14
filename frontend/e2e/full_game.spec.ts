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

/** Wait until the human can act again (Concede re-enables) or the game ends. */
async function waitForHumanTurn(page: Page, timeoutMs = 20000): Promise<'turn' | 'gameover'> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if ((await page.locator('.game-over').count()) > 0) return 'gameover';
    const concede = page.getByRole('button', { name: /Concede/ });
    if (await concede.isEnabled()) return 'turn';
    await page.waitForTimeout(150);
  }
  throw new Error('timed out waiting for human turn');
}

test('full game exercising raise, concede, card clicks and the trick-winner UI', async ({ page }) => {
  test.setTimeout(240000);
  await waitForReady(page);

  // === Round 1 — click a card, then verify the trick-winner UI ===
  await page.locator(SELECTABLE).first().click();
  await expect(page.getByTestId('trick-winner')).toBeVisible({ timeout: 8000 });
  await expect(page.locator('.trick-slot.winner')).toHaveCount(1);
  await expect(
    page.locator('.log').getByText(/Player \d wins the trick/).first()
  ).toBeVisible();

  await waitForHumanTurn(page);

  // === Still round 1 — raise then concede, verify scores and log ===
  await page.getByRole('button', { name: /Raise/ }).click();
  await expect.poll(() => readRoundPoints(page), { timeout: 5000 }).toBe(3);
  await expect(page.locator('.log').getByText(/Team 1 raises to 3/)).toBeVisible();

  const beforeConcede = await readScores(page);
  await page.getByRole('button', { name: /Concede/ }).click();
  await expect(page.locator('.log').getByText(/Team 1 concedes/)).toBeVisible();
  await expect
    .poll(async () => (await readScores(page)).team2, { timeout: 10000 })
    .toBeGreaterThanOrEqual(beforeConcede.team2 + 3);

  // === Round 2 — verify raise + card click can be combined ===
  await waitForHumanTurn(page);
  expect(await readRoundPoints(page)).toBe(2); // reset for the new round
  await page.getByRole('button', { name: /Raise/ }).click();
  await expect.poll(() => readRoundPoints(page), { timeout: 5000 }).toBe(3);
  await page.locator(SELECTABLE).first().click();

  // Trick winner should fire again after this human play completes a trick.
  await expect(page.getByTestId('trick-winner')).toBeVisible({ timeout: 10000 });

  // === Rounds 3+ — alternate concede / click+concede until game ends ===
  for (let round = 0; round < 25; round++) {
    const state = await waitForHumanTurn(page);
    if (state === 'gameover') break;
    const s = await readScores(page);
    if (s.team1 >= s.target || s.team2 >= s.target) break;

    if (round % 3 === 0) {
      // Plain concede.
      await page.getByRole('button', { name: /Concede/ }).click();
    } else if (round % 3 === 1) {
      // Raise then concede.
      const r = page.getByRole('button', { name: /Raise/ });
      if (await r.isEnabled()) await r.click();
      const c = page.getByRole('button', { name: /Concede/ });
      if (await c.isEnabled()) await c.click();
    } else {
      // Play a card then concede on the next turn.
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
  // The full log is kept in the DOM (the scroll area is fixed-height), so
  // historical events from every feature should be present.
  await expect(page.locator('.log').getByText(/raises/).first()).toHaveCount(1);
  await expect(page.locator('.log').getByText(/concedes/).first()).toHaveCount(1);
  await expect(page.locator('.log').getByText(/Player \d wins the trick/).first()).toHaveCount(1);
  await expect(page.locator('.log').getByText(/wins the game/).first()).toHaveCount(1);
});

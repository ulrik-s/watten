import { test, expect, Page } from '@playwright/test';

const SELECTABLE = '.hand-slot .card.selectable';

async function waitForReady(page: Page) {
  await page.goto('/?fast=1');
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

async function waitForRaiseResolution(page: Page, timeoutMs = 10000): Promise<'accepted' | 'folded'> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if ((await page.locator('.log').getByText(/Team 2 accepts/).count()) > 0) return 'accepted';
    if ((await page.locator('.log').getByText(/Team 2 folds/).count()) > 0) return 'folded';
    await page.waitForTimeout(150);
  }
  throw new Error('timed out waiting for raise resolution');
}

/** Click selectable cards until none are visible. */
async function clickThroughHand(page: Page, maxClicks = 25) {
  for (let i = 0; i < maxClicks; i++) {
    const card = page.locator(SELECTABLE).first();
    try {
      await card.waitFor({ state: 'visible', timeout: 3000 });
    } catch {
      return;
    }
    await card.click();
    await page.waitForTimeout(120);
  }
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

  // === Raise + response — covers both accept and fold paths ===
  const ptsBefore = await readRoundPoints(page);
  await page.getByRole('button', { name: /Raise/ }).click();
  await expect(page.locator('.log').getByText(/Team 1 proposes to raise/)).toBeVisible();
  const resolution = await waitForRaiseResolution(page);
  if (resolution === 'accepted') {
    expect(await readRoundPoints(page)).toBe(ptsBefore + 1);
    // Alternation: Team 1 cannot raise again immediately.
    const raise = page.getByRole('button', { name: /Raise/ });
    await expect(raise).toBeDisabled();
  } else {
    // Folded: round-decided indicator must appear.
    await expect(page.getByTestId('round-decided')).toBeVisible();
  }

  // === Whatever happened, the user MUST still play out their remaining
  // cards manually — no auto-play. Click through them. ===
  await clickThroughHand(page);

  // === Subsequent rounds — drive a mix of concedes, raises, plays ===
  for (let round = 0; round < 30; round++) {
    const s = await readScores(page);
    if (s.team1 >= s.target || s.team2 >= s.target) break;
    if ((await page.locator('.game-over').count()) > 0) break;

    const pick = round % 4;
    if (pick === 0) {
      const c = page.getByRole('button', { name: /Concede/ });
      if (await c.isEnabled()) await c.click();
    } else if (pick === 1) {
      const r = page.getByRole('button', { name: /Raise/ });
      if (await r.isEnabled()) {
        await r.click();
        try {
          await waitForRaiseResolution(page, 5000);
        } catch {
          /* ignore */
        }
      }
    }
    // Either way: play out the rest of the hand.
    await clickThroughHand(page);
  }

  // === Game must be over ===
  await expect(page.locator('.game-over')).toBeVisible({ timeout: 30000 });
  const final = await readScores(page);
  expect(Math.max(final.team1, final.team2)).toBeGreaterThanOrEqual(final.target);

  await expect(page.locator('.log').getByText(/proposes to raise/).first()).toHaveCount(1);
  const accepts = await page.locator('.log').getByText(/Team 2 accepts/).count();
  const folds = await page.locator('.log').getByText(/Team 2 folds/).count();
  expect(accepts + folds).toBeGreaterThan(0);
  await expect(page.locator('.log').getByText(/concedes/).first()).toHaveCount(1);
  await expect(page.locator('.log').getByText(/Player \d wins the trick/).first()).toHaveCount(1);
  await expect(page.locator('.log').getByText(/wins the game/).first()).toHaveCount(1);
});

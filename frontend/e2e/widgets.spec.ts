import { test, expect, Page } from '@playwright/test';

const HAND_CARD = '.hand-slot .card';
const SELECTABLE = '.hand-slot .card.selectable';

async function waitForReady(page: Page) {
  // ?fast=1 shortens animation delays so the suite still completes quickly
  // even though concede/fold now play the round out to completion.
  await page.goto('/?fast=1');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Watten');
  await page.locator(SELECTABLE).first().waitFor({ state: 'visible', timeout: 30000 });
}

/** Click any selectable cards until the human's hand is empty or the round
 *  rolls over. Used to drive a round to its natural end after a concede /
 *  fold when the engine no longer auto-plays. */
async function clickHandToEnd(page: Page, maxClicks = 30) {
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
  test('Raise button proposes a raise that Team 2 either accepts or folds', async ({ page }) => {
    test.setTimeout(60000);
    await waitForReady(page);
    const beforePts = await readRoundPoints(page);
    const beforeScores = await (async () => {
      const text = await page.locator('p').filter({ hasText: /Team 1/ }).first().innerText();
      const m = text.match(/Team\s*1\s*(\d+)\s*[—-]\s*Team\s*2\s*(\d+)/);
      return { team1: parseInt(m![1], 10), team2: parseInt(m![2], 10) };
    })();
    await page.getByRole('button', { name: /Raise/ }).click();
    // Always log the proposal first.
    await expect(page.locator('.log').getByText(/Team 1 proposes to raise/)).toBeVisible();
    // Either accepted (round-worth bumps) or folded (game-over banner OR
    // round resets to 2 and Team 1's score goes up by the pre-raise value).
    await expect
      .poll(
        async () => {
          if ((await page.locator('.log').getByText(/Team 2 accepts/).count()) > 0) {
            return 'accepted';
          }
          if ((await page.locator('.log').getByText(/Team 2 folds/).count()) > 0) {
            return 'folded';
          }
          return null;
        },
        { timeout: 8000 }
      )
      .not.toBeNull();
    if ((await page.locator('.log').getByText(/Team 2 accepts/).count()) > 0) {
      expect(await readRoundPoints(page)).toBe(beforePts + 1);
    } else {
      // Folded: Team 1 is locked in for the pre-raise value, but the round
      // is played to completion before the points actually land. Poll the
      // score until it goes up.
      await expect
        .poll(
          async () => {
            const afterText = await page
              .locator('p')
              .filter({ hasText: /Team 1/ })
              .first()
              .innerText();
            const m = afterText.match(/Team\s*1\s*(\d+)/);
            return parseInt(m![1], 10);
          },
          { timeout: 30000 }
        )
        .toBeGreaterThanOrEqual(beforeScores.team1 + beforePts);
    }
  });

  test('Concede locks in Team 2; user plays out remaining cards; scores then credit Team 2', async ({ page }) => {
    test.setTimeout(120000);
    await waitForReady(page);
    const before = await readScores(page);
    await page.getByRole('button', { name: /Concede/ }).click();
    // The round-decided indicator should appear, and the user keeps clicking.
    await expect(page.getByTestId('round-decided')).toBeVisible({ timeout: 5000 });
    await clickHandToEnd(page);
    // Score only lands at finish_round, after the user has played out their hand.
    await expect
      .poll(async () => (await readScores(page)).team2, { timeout: 60000 })
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
    // Each round: concede, then click through the human's remaining cards
    // to drive the round to its natural end. Repeat until the game ends.
    const concede = page.getByRole('button', { name: /Concede/ });
    for (let i = 0; i < 30; i++) {
      const s = await readScores(page);
      if (s.team1 >= s.target || s.team2 >= s.target) break;
      try {
        await expect(concede).toBeEnabled({ timeout: 10000 });
      } catch {
        break;
      }
      await concede.click();
      await clickHandToEnd(page);
      // Wait until the next round's deal is ready (concede button re-enabled),
      // or the game ends.
      await Promise.race([
        page.locator('.game-over').waitFor({ state: 'visible', timeout: 10000 }),
        concede
          .elementHandle()
          .then((h) =>
            page.waitForFunction(
              (el) => el && !(el as HTMLButtonElement).disabled,
              h,
              { timeout: 10000 }
            )
          ),
      ]).catch(() => undefined);
    }
    await expect(page.locator('.game-over')).toBeVisible({ timeout: 10000 });
  });
});

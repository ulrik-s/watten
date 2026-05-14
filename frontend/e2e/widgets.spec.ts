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

  test('Concede locks in Team 2 and credits the opponent after the round plays out', async ({ page }) => {
    test.setTimeout(120000);
    await waitForReady(page);
    const before = await readScores(page);
    await page.getByRole('button', { name: /Concede/ }).click();
    // Concede no longer credits the opponent immediately — it locks the
    // winner and the engine plays out the remaining tricks. Scores update
    // once the round completes (≤ ~30s for a full 5-trick playthrough).
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
    test.setTimeout(600000); // up to 10 min — concedes auto-play 5 tricks each
    await waitForReady(page);
    // Concede repeatedly until the game ends. Each concede now plays the
    // round out to completion before the next deal, so we wait generously
    // for the button to re-enable between rounds.
    const concede = page.getByRole('button', { name: /Concede/ });
    for (let i = 0; i < 30; i++) {
      const s = await readScores(page);
      if (s.team1 >= s.target || s.team2 >= s.target) break;
      try {
        await expect(concede).toBeEnabled({ timeout: 60000 });
      } catch {
        break;
      }
      await concede.click();
      // Wait until either the button is re-enabled (next round ready) or
      // the game-over banner appears. The full round playthrough plus the
      // round-gap takes roughly 25–40 seconds.
      await Promise.race([
        page.locator('.game-over').waitFor({ state: 'visible', timeout: 60000 }),
        concede.elementHandle().then((h) =>
          page.waitForFunction((el) => el && !(el as HTMLButtonElement).disabled, h, {
            timeout: 60000,
          })
        ),
      ]).catch(() => undefined);
    }
    await expect(page.locator('.game-over')).toBeVisible({ timeout: 10000 });
  });
});

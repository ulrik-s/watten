import { test, expect, Page } from '@playwright/test';

const SELECTABLE = '.hand-slot .card.selectable';
const HAND_SLOT = '.hand-slot';

async function waitForReady(page: Page) {
  await page.goto('/?fast=1');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Watten');
  await page.locator(SELECTABLE).first().waitFor({ state: 'visible', timeout: 30000 });
}

async function waitForHumanTurn(page: Page, timeoutMs = 30000): Promise<'turn' | 'gameover'> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if ((await page.locator('.game-over').count()) > 0) return 'gameover';
    if ((await page.locator(SELECTABLE).count()) > 0) return 'turn';
    await page.waitForTimeout(150);
  }
  throw new Error('timed out waiting for human turn');
}

interface Snapshot {
  slot: number;
  filled: boolean;
  key: string;
}

async function snapshotHand(page: Page): Promise<Snapshot[]> {
  const slots = page.locator(HAND_SLOT);
  const total = await slots.count();
  expect(total).toBe(5);
  const out: Snapshot[] = [];
  for (let i = 0; i < total; i++) {
    const slot = slots.nth(i);
    const placeholderCount = await slot.locator('.placeholder').count();
    if (placeholderCount > 0) {
      out.push({ slot: i, filled: false, key: 'empty' });
      continue;
    }
    const card = slot.locator('.card').first();
    const rank = (await card.locator('.rank').innerText()).trim();
    const suitSrc = (await card.locator('img').getAttribute('src')) ?? '';
    out.push({ slot: i, filled: true, key: `${rank}|${suitSrc}` });
  }
  return out;
}

async function readScores(page: Page) {
  const text = await page.locator('p').filter({ hasText: /Team 1/ }).first().innerText();
  const m = text.match(/Team\s*1\s*(\d+)\s*[—-]\s*Team\s*2\s*(\d+)\s*\(to\s*(\d+)\)/);
  if (!m) throw new Error('Cannot parse: ' + text);
  return { team1: parseInt(m[1], 10), team2: parseInt(m[2], 10), target: parseInt(m[3], 10) };
}

async function readRoundNumber(page: Page): Promise<number> {
  const text = await page.locator('p').filter({ hasText: /^Round\s+/ }).first().innerText();
  const m = text.match(/Round\s+(\d+)/);
  if (!m) throw new Error('cannot parse round number: ' + text);
  return parseInt(m[1], 10);
}

test('the human must play exactly 5 cards (5 clicks) to finish a round', async ({ page }) => {
  test.setTimeout(120000);
  await waitForReady(page);

  const startRound = await readRoundNumber(page);
  expect(startRound).toBe(1);

  let clicks = 0;
  for (let i = 0; i < 12; i++) {
    // If the round has rolled over, stop counting.
    if ((await readRoundNumber(page)) !== startRound) break;
    if ((await page.locator('.game-over').count()) > 0) break;

    const selectable = page.locator(SELECTABLE);
    if ((await selectable.count()) === 0) {
      // No card to click and the round hasn't rolled over — wait for the
      // bots to give us a turn, then loop.
      await page.waitForTimeout(150);
      continue;
    }
    await selectable.first().click();
    clicks += 1;
    // Let the bots advance.
    await page.waitForTimeout(200);
  }

  // Should be exactly 5: each round consists of 5 tricks, and the human
  // plays one card per trick.
  expect(clicks).toBe(5);
  // And after those 5 clicks the round counter has moved on.
  await expect.poll(() => readRoundNumber(page), { timeout: 30000 }).toBe(startRound + 1);
});

test('after a concede, the user still has to click through their remaining cards before the next deal', async ({ page }) => {
  test.setTimeout(120000);
  await waitForReady(page);

  const startRound = await readRoundNumber(page);
  expect((await snapshotHand(page)).filter((s) => s.filled).length).toBe(5);

  // Concede right after the round starts. Round outcome should be locked,
  // but the round counter must NOT roll over yet — the user still has 5
  // cards to play.
  await page.getByRole('button', { name: /Concede/ }).click();
  await expect(page.getByTestId('round-decided')).toBeVisible();
  expect(await readRoundNumber(page)).toBe(startRound);
  expect((await snapshotHand(page)).filter((s) => s.filled).length).toBe(5);

  // Now click through the human's hand. After 5 clicks the round should end
  // and the counter advance.
  for (let i = 0; i < 5; i++) {
    const card = page.locator('.hand-slot .card.selectable').first();
    await card.waitFor({ state: 'visible', timeout: 10000 });
    await card.click();
    await page.waitForTimeout(150);
  }

  await expect.poll(() => readRoundNumber(page), { timeout: 30000 }).toBe(startRound + 1);
  const after = await snapshotHand(page);
  expect(after.filter((s) => s.filled).length).toBe(5);
});

test('played slots stay empty until the round actually ends (no mid-round redeal)', async ({ page }) => {
  test.setTimeout(120000);
  await waitForReady(page);

  // Capture the full original 5-card hand for this round and the score.
  let roundInitial = await snapshotHand(page);
  expect(roundInitial.filter((s) => s.filled).length).toBe(5);
  const expectedEmpty = new Set<number>();

  for (let play = 0; play < 5; play++) {
    const beforeSnap = await snapshotHand(page);
    const beforeFilled = beforeSnap.filter((s) => s.filled).length;
    if (beforeFilled === 0) break;

    // Find the FIRST selectable slot.
    const slots = page.locator(HAND_SLOT);
    let clickedSlot = -1;
    for (let i = 0; i < 5; i++) {
      if (await slots.nth(i).locator('.card.selectable').count()) {
        clickedSlot = i;
        break;
      }
    }
    expect(clickedSlot).toBeGreaterThanOrEqual(0);

    await slots.nth(clickedSlot).locator('.card.selectable').click();
    expectedEmpty.add(clickedSlot);

    const state = await waitForHumanTurn(page);
    if (state === 'gameover') return;

    // Robust round-transition detection: if the fill count is now HIGHER
    // than (beforeFilled - 1), a new round was dealt while we waited. The
    // only way the count could go up mid-round would be a bug.
    const after = await snapshotHand(page);
    const afterFilled = after.filter((s) => s.filled).length;
    if (afterFilled > beforeFilled - 1) {
      // A new round has begun (round-end → re-deal). Reset our tracking
      // and continue with the new round.
      roundInitial = after;
      expectedEmpty.clear();
      continue;
    }

    // Same round — assert no slot was unexpectedly refilled and unplayed
    // slots still hold the original card identity.
    for (let i = 0; i < 5; i++) {
      if (expectedEmpty.has(i)) {
        expect
          .soft(after[i].filled, `slot ${i} unexpectedly refilled mid-round`)
          .toBe(false);
      } else {
        expect
          .soft(after[i].key, `slot ${i} card identity changed mid-round`)
          .toBe(roundInitial[i].key);
      }
    }
  }
});

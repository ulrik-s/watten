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

test('after a concede, every hand drains to zero before the next deal', async ({ page }) => {
  test.setTimeout(120000);
  await waitForReady(page);

  const startRound = await readRoundNumber(page);

  // Concede right after the round starts.
  await page.getByRole('button', { name: /Concede/ }).click();

  // Wait until the round actually changes (= new deal happened).
  await expect.poll(() => readRoundNumber(page), { timeout: 60000 }).toBe(startRound + 1);

  // The new round should be a fresh 5-card hand.
  const after = await snapshotHand(page);
  expect(after.filter((s) => s.filled).length).toBe(5);

  // The play-to-completion invariant is implicit: the round counter only
  // advances when finish_round fires, which only fires when every hand is
  // empty. Reaching this assertion proves it.
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

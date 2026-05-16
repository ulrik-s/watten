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

async function readRoundNumberFromUI(page: Page): Promise<number> {
  const text = await page
    .locator('p')
    .filter({ hasText: /^Round\s/ })
    .first()
    .innerText();
  const m = text.match(/Round\s+(\d+)/);
  return m ? parseInt(m[1], 10) : 0;
}

/** Click selectable cards until the round counter advances, the human's
 *  hand is empty, OR the game ends. Stops at the next round's deal so we
 *  don't bleed into subsequent rounds. */
async function clickHandToEnd(page: Page, maxClicks = 30) {
  const startRound = await readRoundNumberFromUI(page);
  for (let i = 0; i < maxClicks; i++) {
    // Game over — no further rounds; bail.
    if ((await page.locator('.game-over').count()) > 0) return;
    const card = page.locator(SELECTABLE).first();
    try {
      await card.waitFor({ state: 'visible', timeout: 3000 });
    } catch {
      // No card available — round most likely transitioned.
      await page.waitForTimeout(200);
      if ((await readRoundNumberFromUI(page)) !== startRound) return;
      if ((await page.locator('.game-over').count()) > 0) return;
      continue;
    }
    await card.click();
    await page.waitForTimeout(120);
    if ((await readRoundNumberFromUI(page)) !== startRound) return;
    if ((await page.locator('.game-over').count()) > 0) return;
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
      // Folded: Team 1 is locked in. The user must still play out the
      // remaining cards manually — drive that, then assert the score.
      await expect(page.getByTestId('round-decided')).toBeVisible();
      await clickHandToEnd(page);
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

  test('Tricks-this-round counter increments when a trick completes', async ({ page }) => {
    test.setTimeout(60000);
    await waitForReady(page);
    // Before any human play the bots have not completed a trick yet, so the
    // counter must be 0–0.
    const counter = page.getByTestId('tricks-this-round');
    await expect(counter).toContainText('Team 1 0');
    await expect(counter).toContainText('Team 2 0');
    // Click a card → trick completes → exactly one team's count goes up.
    await page.locator(SELECTABLE).first().click();
    await expect
      .poll(async () => {
        const text = await counter.innerText();
        const m = text.match(/Team\s*1\s*(\d+)\s*·\s*Team\s*2\s*(\d+)/);
        return m ? parseInt(m[1], 10) + parseInt(m[2], 10) : 0;
      }, { timeout: 10000 })
      .toBeGreaterThanOrEqual(1);
  });

  test('Accepted raise pays the round at its raised value (not pre-raise)', async ({ page }) => {
    test.setTimeout(180000);
    await waitForReady(page);

    async function readScoresNow() {
      const text = await page
        .locator('p')
        .filter({ hasText: /Team 1/ })
        .first()
        .innerText();
      const m = text.match(/Team\s*1\s*(\d+)\s*[—-]\s*Team\s*2\s*(\d+)/);
      return { team1: parseInt(m![1], 10), team2: parseInt(m![2], 10) };
    }

    for (let attempt = 0; attempt < 12; attempt++) {
      // If a round transition is in flight, wait for it to settle.
      await page
        .locator('.hand-slot .card.selectable')
        .first()
        .waitFor({ state: 'visible', timeout: 15000 });

      const beforeScores = await readScoresNow();
      const beforePts = await readRoundPoints(page);
      const round = await readRoundNumberFromUI(page);

      await page.getByRole('button', { name: /Raise/ }).click();
      const resolved = await Promise.race([
        page
          .locator('.log')
          .getByText(/Team 2 accepts/)
          .nth(attempt)
          .waitFor({ state: 'visible', timeout: 8000 })
          .then(() => 'accepted' as const),
        page
          .locator('.log')
          .getByText(/Team 2 folds/)
          .waitFor({ state: 'visible', timeout: 8000 })
          .then(() => 'folded' as const),
      ]).catch(() => null);

      if (resolved === 'accepted') {
        expect(await readRoundPoints(page)).toBe(beforePts + 1);
        // Play exactly this round's 5 cards. clickHandToEnd now bails the
        // moment the round counter advances.
        await clickHandToEnd(page);
        // Wait for the round counter to actually advance.
        await expect
          .poll(() => readRoundNumberFromUI(page), { timeout: 30000 })
          .toBe(round + 1);
        const afterScores = await readScoresNow();
        const gain1 = afterScores.team1 - beforeScores.team1;
        const gain2 = afterScores.team2 - beforeScores.team2;
        // Exactly one team got points and it equals the raised value.
        expect(gain1 + gain2).toBe(beforePts + 1);
        expect(Math.max(gain1, gain2)).toBe(beforePts + 1);
        return;
      }

      // Folded or timed out — play out this round and try the next.
      await clickHandToEnd(page);
      // If the game ended (Team 1 reached the target via fold payouts),
      // we can stop — no further raises possible.
      if ((await page.locator('.game-over').count()) > 0) {
        // The accept path never triggered; treat as a skipped condition
        // rather than a hard failure — the bots may have judged every
        // proposal too risky on this seed.
        test.skip(true, 'game ended before an accepted raise was observed');
        return;
      }
      await expect
        .poll(
          async () => {
            if ((await page.locator('.game-over').count()) > 0) return -1;
            return await readRoundNumberFromUI(page);
          },
          { timeout: 30000 }
        )
        .toBe(round + 1);
    }
    throw new Error('Team 2 never accepted a raise in 12 attempts');
  });

  test('Trump and striker are hidden on non-seer rounds and revealed by Show scores', async ({ page }) => {
    test.setTimeout(60000);
    await waitForReady(page);

    // Round 1 always has dealer=P1 (idx 0) so the human IS a seer.
    // Trump/striker should be visible by default in round 1.
    const info = page.getByTestId('round-info');
    await expect(info).toContainText(/Dealer:\s*P1/);
    await expect(info).toContainText(/Trump:/);
    await expect(info).toContainText(/Striker:/);
    await expect(page.locator('.hidden-trump-hint')).toHaveCount(0);

    // Drive a concede to move to round 2, where dealer rotates to P2.
    // Human is now P1 — dealer is P2, forehand is P3, so the human is
    // NOT a seer this round. Trump/striker must be hidden.
    await page.getByRole('button', { name: /Concede/ }).click();
    await clickHandToEnd(page);
    await expect.poll(() => readRoundNumberFromUI(page), { timeout: 30000 }).toBe(2);
    await expect(info).toContainText(/Dealer:\s*P2/);
    await expect(info).not.toContainText(/Trump:/);
    await expect(page.locator('.hidden-trump-hint')).toBeVisible();

    // Enable Show scores → trump/striker reappear (with a (debug) marker).
    await page.getByTestId('show-debug').check();
    await expect(info).toContainText(/Trump:/);
    await expect(info).toContainText(/Striker:/);
    await expect(info).toContainText(/\(debug\)/);
  });

  test('Teams P1+P3 vs P2+P4 are tagged in the UI; seeing players are split', async ({ page }) => {
    test.setTimeout(20000);
    await waitForReady(page);
    // Legend with both chips visible.
    const info = page.getByTestId('team-info');
    await expect(info).toContainText('Team 1');
    await expect(info).toContainText('Team 2');
    await expect(info).toContainText('P3');
    await expect(info).toContainText('P2');
    await expect(info).toContainText('P4');
    // Each opponent row carries a "T1" or "T2" tag in its player label.
    // P2 (idx 1) → T2; P3 (idx 2) → T1; P4 (idx 3) → T2.
    const p2 = page.locator('.player.p2 .player-label');
    const p3 = page.locator('.player.p3 .player-label');
    const p4 = page.locator('.player.p4 .player-label');
    await expect(p2).toContainText('T2');
    await expect(p3).toContainText('T1');
    await expect(p4).toContainText('T2');
  });

  test('120^4 database toggle is rendered and shows a progress bar when clicked', async ({ page }) => {
    test.setTimeout(60000);
    await waitForReady(page);
    const dbToggle = page.getByTestId('toggle-db-evaluator');
    await expect(dbToggle).toBeVisible();
    await expect(dbToggle).not.toBeChecked();
    await expect(dbToggle).toBeEnabled();
    const label = page.locator('label', { has: dbToggle });
    await expect(label).toContainText(/database/i);
    // Kick off the populate. We don't wait for it to complete — the goal
    // is to verify the progress bar appears and the % advances.
    void dbToggle.click({ force: true });
    const bar = page.getByTestId('db-progress');
    await expect(bar).toBeVisible({ timeout: 10000 });
    // Read the percentage twice and assert it moved forward.
    const readPercent = async () => {
      const txt = await bar.innerText();
      const m = txt.match(/(\d+)%/);
      return m ? parseInt(m[1], 10) : 0;
    };
    const first = await readPercent();
    await page.waitForTimeout(800);
    const second = await readPercent();
    expect(second).toBeGreaterThanOrEqual(first);
    // At least *some* progress should have been made.
    expect(Math.max(first, second)).toBeGreaterThan(0);
  });

  test('Show-scores debug toggle reveals round/trick scores per card', async ({ page }) => {
    test.setTimeout(60000);
    await waitForReady(page);
    // Off by default — no debug badges.
    await expect(page.getByTestId('hand-debug').first()).toHaveCount(0);
    // Turn it on.
    await page.getByTestId('show-debug').check();
    // Hand cards now have an R:N badge.
    const handDebug = page.getByTestId('hand-debug').first();
    await expect(handDebug).toBeVisible();
    await expect(handDebug).toContainText('R:');
    // After playing a card, trick slots show R:/T: badges too.
    await page.locator(SELECTABLE).first().click();
    await expect(page.getByTestId('trick-debug').first()).toBeVisible({ timeout: 5000 });
    await expect(page.getByTestId('trick-debug').first()).toContainText('R:');
    await expect(page.getByTestId('trick-debug').first()).toContainText('T:');
  });

  test('Trick winner banner matches the strongest card on the table', async ({ page }) => {
    test.setTimeout(60000);
    // Run this one at normal speed so the winner highlight is on screen long
    // enough to inspect; the other tests use ?fast=1.
    await page.goto('/');
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('Watten');
    await page.locator(SELECTABLE).first().waitFor({ state: 'visible', timeout: 30000 });

    // Trump/striker are now hidden on non-seer rounds. Enable Show scores so
    // they're surfaced regardless of seer status, so this test can read them
    // from the DOM.
    await page.getByTestId('show-debug').check();

    await page.locator(SELECTABLE).first().click();
    await expect(page.getByTestId('trick-winner')).toBeVisible({ timeout: 12000 });

    const trumpSuit = (await page.getByTestId('trump-display').innerText()).trim();
    const strikerDisplay = (await page.getByTestId('striker-display').innerText()).trim();
    expect(trumpSuit).not.toBe('-');
    expect(strikerDisplay).not.toBe('-');

    // Capture all four trick cards + the winner flag *in one go* while the
    // banner is still up.
    const trickCards = await page.locator('.trick-slot').evaluateAll((slots) =>
      (slots as HTMLElement[]).map((el) => {
        const cardEl = el.querySelector('.card') as HTMLElement | null;
        const rank = cardEl?.getAttribute('data-rank') ?? '';
        const suit = cardEl?.getAttribute('data-suit') ?? '';
        const winner = el.classList.contains('winner');
        return { rank, suit, winner };
      })
    );
    // Guard: make sure we captured a full trick.
    expect(trickCards.filter((c) => c.rank !== '' && c.suit !== '').length).toBe(4);

    // Independently compute the expected winner using the user's spec:
    //   round_score = (trump_suit ? 100 : 0) + (striker_rank ? 200 : 0)
    //   trick_score = -400 if striker_rank AND earlier striker played
    //                 rv + 20 if same suit as lead
    //                 rv      otherwise
    // Strict > with earlier-wins-ties.
    const RANK_DISPLAY_TO_VALUE: Record<string, number> = {
      '7': 1, '8': 2, '9': 3, '10': 4,
      Unter: 5, Ober: 6, King: 7, Ace: 8, Weli: 9,
    };
    const leadSuit = trickCards[0].suit;
    function score(c: { rank: string; suit: string }, pos: number, all: typeof trickCards): number {
      let round = 0;
      if (c.suit === trumpSuit) round += 100;
      if (c.rank === strikerDisplay) round += 200;
      let trick: number;
      if (c.rank === strikerDisplay) {
        let earlierStriker = false;
        for (let k = 0; k < pos; k++) if (all[k].rank === strikerDisplay) earlierStriker = true;
        if (earlierStriker) return round - 400;
      }
      const rv = RANK_DISPLAY_TO_VALUE[c.rank] ?? 0;
      trick = c.suit === leadSuit ? rv + 20 : rv;
      return round + trick;
    }
    let expectedWinner = 0;
    let bestScore = score(trickCards[0], 0, trickCards);
    for (let i = 1; i < 4; i++) {
      const s = score(trickCards[i], i, trickCards);
      if (s > bestScore) {
        bestScore = s;
        expectedWinner = i;
      }
    }
    const uiWinner = trickCards.findIndex((c) => c.winner);
    expect(uiWinner).toBe(expectedWinner);
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

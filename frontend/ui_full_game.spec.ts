import { test, expect } from '@playwright/test';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const baseURL = 'http://localhost:4177';

function parseScores(text: string): [number, number] {
  const m = text.match(/Scores:\s*(\d+)\s*-\s*(\d+)/);
  if (!m) throw new Error('Could not parse scores from: ' + text);
  return [parseInt(m[1], 10), parseInt(m[2], 10)];
}

test('play full game to 13 points', async ({ page, browserName }) => {
  test.setTimeout(120000);
  test.skip(browserName !== 'firefox', 'runs only on Firefox');
  await page.goto(baseURL);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Watten');

  const scoreLocator = page.locator('p', { hasText: /^Scores:/ });

  while (true) {
    const scoreText = await scoreLocator.innerText();
    const [a, b] = parseScores(scoreText);
    if (a >= 13 || b >= 13) break;
    const card = page.locator('.player.hand .card.selectable').first();
    await card.waitFor({ state: 'visible' });
    await card.click();
    await page.waitForTimeout(100);
  }

  const finalText = await scoreLocator.innerText();
  const [s1, s2] = parseScores(finalText);
  expect(s1 >= 13 || s2 >= 13).toBeTruthy();
});

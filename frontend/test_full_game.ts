import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

interface JsCard {
  suit: string;
  rank: string;
}

interface JsRoundStep {
  player: number;
  hand: JsCard[];
  allowed: number[];
  played: JsCard;
}

(async () => {
  const wasm = await import('../pkg-test/watten.js');
  const game = new wasm.WasmGame(0);
  if (game.set_perm_range_single) {
    game.set_perm_range_single(0);
  }
  const WINNING = 13;
  let round = 1;
  while (game.scores()[0] < WINNING && game.scores()[1] < WINNING) {
    console.log(`\n-- Round ${round} --`);
    const [result, steps] = game.play_round_logged() as [number, JsRoundStep[]];
    const remaining = [5, 5, 5, 5];
    for (const step of steps) {
      const handStr = step.hand.map((c) => `${c.rank} of ${c.suit}`).join(', ');
      const allowedStr = step.allowed.join(', ');
      console.log(
        `Player ${step.player + 1} hand before play: ${handStr} | allowed indices: [${allowedStr}]`
      );
      const playedStr = `${step.played.rank} of ${step.played.suit}`;
      console.log(`Player ${step.player + 1} plays ${playedStr}`);
      const idx = step.hand.findIndex(
        (c) => c.suit === step.played.suit && c.rank === step.played.rank
      );
      if (!step.allowed.includes(idx)) {
        throw new Error(`Illegal move by player ${step.player + 1}`);
      }
      if (step.hand.length !== remaining[step.player]) {
        throw new Error(
          `Unexpected hand size for player ${step.player + 1}: ${step.hand.length}`
        );
      }
      remaining[step.player]--;
    }
    const scores = game.scores();
    console.log('Scores after round', scores);
    console.log('Result code', result);
    round++;
  }
  const final = game.scores();
  console.log('\nFinal scores', final);
  if (final[0] < WINNING && final[1] < WINNING) {
    throw new Error('Game did not reach winning score');
  }
})();

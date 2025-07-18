const fs = require('fs');
const path = require('path');
(async () => {
  const wasm = await import('../pkg-test/watten.js');
  const bytes = fs.readFileSync(path.join(__dirname, '..', 'pkg-test', 'watten_bg.wasm'));
  wasm.initSync(bytes.buffer);
  const game = new wasm.WasmGame(0);
  // Limit permutation range so the test runs quickly
  if (game.set_perm_range_single) {
    game.set_perm_range_single(0);
  }
  const WINNING = 13;
  let round = 1;
  while (game.scores()[0] < WINNING && game.scores()[1] < WINNING) {
    console.log(`\n-- Round ${round} --`);
    const result = game.play_round();
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

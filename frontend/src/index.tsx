import React from 'react';
import { createRoot } from 'react-dom/client';
import init, { WasmGame } from '../../pkg/watten';

async function main() {
  await init();
  const game = new WasmGame(0);
  game.start_round();
  console.log('Trump:', game.trump_suit());
  console.log('Striker:', game.striker_rank());
  console.log('Player 1 hand:', game.hand(0));
}

main();

const root = createRoot(document.getElementById('root')!);
root.render(<h1>Watten</h1>);

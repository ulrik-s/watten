import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

(async () => {
  const wasm = await import('../pkg-test/watten.js');
  const game = new wasm.WasmGame(0);
  if (game.scores().length !== 2) {
    throw new Error('Unexpected scores length');
  }
  console.log('frontend test passed');
})().catch((err) => {
  console.error(err);
  process.exit(1);
});

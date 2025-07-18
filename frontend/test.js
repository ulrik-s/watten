const fs = require('fs');
const path = require('path');

(async () => {
  const wasm = await import('../pkg-test/watten.js');
  const bytes = fs.readFileSync(
    path.join(__dirname, '..', 'pkg-test', 'watten_bg.wasm')
  );
  wasm.initSync(bytes.buffer);
  const game = new wasm.WasmGame(0);
  if (game.scores().length !== 2) {
    throw new Error('Unexpected scores length');
  }
  console.log('frontend test passed');
})().catch((err) => {
  console.error(err);
  process.exit(1);
});

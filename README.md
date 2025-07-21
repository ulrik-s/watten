# Watten

Basic structures for a Watten card game trainer written in Rust.

The crate provides:

- A 33 card deck used in the game
- Utilities for enumerating all 120 possible orders in which a 5‑card hand can be played
- A database API that maps ordered plays of four players to a result (team 1/2 win, not played or rule violation)
- `GameState::play_round` returns the [`GameResult`] of the round
- `GameState::raise_round` lets teams increase the current round value
- `play_hand` plays a round with specific hand IDs and returns the result
- Functions for computing permutation ranges so partially played games can be matched

Run tests with `cargo test`. Heavier tests that populate the full
database are ignored by default and can be run with `cargo full`.
The frontend also has a small test suite which can be executed with
`npm test` from the `frontend` directory.

## WebAssembly Frontend

The crate can be compiled to WebAssembly and consumed by a small React
application.  You will need the [`wasm-pack`](https://rustwasm.github.io/wasm-pack/)
tool and a recent Node.js installation.

### Building and running

You can build the WebAssembly package, install JavaScript dependencies and
start the development server with a single command from the repository root.
The command runs `wasm-pack build` under the hood to compile the Rust code:

```bash
npm start
```

Open `http://localhost:5173` in your browser.

If you prefer to run the steps manually:

1. Compile the Rust code to WebAssembly from the repository root:

   ```bash
   npm --prefix frontend run build:wasm
   ```

   This invokes `wasm-pack build --target web` and outputs a `pkg/` directory.

2. Install JavaScript dependencies and start the development server:

   ```bash
   cd frontend
   npm install
   npm start
   ```

3. To create a production build run:

   ```bash
   npm run build
   ```

   You can then open `dist/index.html` directly or run `npm run serve` to preview
   it.

4. To run the frontend tests:

   ```bash
   npm test
   ```

   This builds the WebAssembly with `wasm-pack` and runs a small Node.js
   script that loads the module and verifies it can start a round.

5. To run the heavy database tests together with the frontend tests:

   ```bash
   npm run full-test
   ```

   This invokes `cargo full` to execute the ignored Rust tests that populate
   the full database and then runs the TypeScript tests.

`yarn` or `pnpm` can be used instead of `npm` if preferred.

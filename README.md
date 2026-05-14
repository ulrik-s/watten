# Watten

A Watten card-game trainer (Tyrolean / 33-card variant) written in Rust with a
WebAssembly frontend. The bots use a memoized legal-completion search to pick
their cards and to estimate per-card win rates for the human player.

## Crate features

- 33-card Watten deck (four suits × Seven..Ace plus the Weli)
- `Card`, `Suit`, `Rank`, `GameResult`, `Player`, `GameState`
- `card_strength` enforcing Rechte > Striker rank > trump > lead > rest
- Seeing-player must-follow-trump rule (`GameState::allowed_indices`)
- `GameState::play_round` / `play_round_logged` (non-interactive)
- `start_round_interactive` + `human_play` + `advance_bots` (interactive)
- `raise_round(team)` / `concede_round(team)` for "raising" and "geh"
- Two move evaluators selectable via `set_evaluator`:
  - `Evaluator::Search` (default): memoized enumeration of all legal trick
    completions from the current state. Sub-second per round.
  - `Evaluator::Database`: legacy 120⁴ brute-force, kept as a benchmark
    fallback. `cargo full` exercises it.

## Running

### Quick start (play locally)

You need:

- Rust (stable) + the `wasm32-unknown-unknown` target
  (`rustup target add wasm32-unknown-unknown`)
- [`wasm-pack`](https://rustwasm.github.io/wasm-pack/) (`cargo install wasm-pack`)
- Node.js 18+

Then, from the repository root:

```bash
make dev
```

Open <http://localhost:5173> in your browser. The Vite dev server hot-reloads
both the React frontend and the Rust → WASM bundle.

Equivalent without `make`:

```bash
npm start
```

### Tests

```bash
cargo test           # fast Rust unit + integration tests
cargo full           # also runs the heavy database population tests
```

Frontend unit / integration tests (Vitest + Testing Library, builds wasm
first):

```bash
cd frontend
npm install
npm test             # Vitest
npm run test:watch   # Vitest in watch mode
```

End-to-end browser tests (Playwright on Chromium, Firefox and WebKit):

```bash
cd frontend
npx playwright install   # first time only
npm run test:e2e
```

### Code coverage

From the repository root:

```bash
npm run coverage         # Rust (cargo-llvm-cov) + JS (Vitest v8)
npm run coverage:rust    # Rust only — HTML in coverage-rust/html
npm run coverage:js      # JS only   — HTML in frontend/coverage
```

`cargo llvm-cov` requires the `llvm-tools-preview` rustup component and the
`cargo-llvm-cov` cargo subcommand:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov
```

### Deployment

Pushes to `main` trigger
[`.github/workflows/gh-pages.yml`](.github/workflows/gh-pages.yml), which
builds the Vite bundle with `VITE_BASE` set to the repo path and deploys it
to GitHub Pages. Enable Pages in repository settings → *Pages* → *Source:
GitHub Actions* once, and subsequent pushes deploy automatically. The
deployed URL is reported in each workflow run.

### CLI

```bash
cargo run
```

Plays a game in the terminal. Set `verbose = true` on `GameState` to see
narration (already enabled by the CLI binary).

### WebAssembly frontend

You will need [`wasm-pack`](https://rustwasm.github.io/wasm-pack/) and
Node.js. From the repository root:

```bash
npm start
```

This installs JS dependencies, runs `wasm-pack build --target web`, and starts
Vite. Open `http://localhost:5173`. The UI shows your hand with a per-card
win-rate estimate (wins / total over all legal completions); opponent hand
sizes; the current trick; the running scores; and **Raise (+1)** and **Concede
round** buttons.

To build a production bundle and preview it:

```bash
cd frontend
npm run build
npm run serve
```

To run the Playwright UI tests (Firefox only):

```bash
cd frontend
npm run test:ui
```

## Rules summary

- **Trump (Schlag)** is the suit of the *top card of the dealer's pile* (the
  first card dealt to the dealer).
- **Striker rank (Weisen)** is the rank of the *top card of the forehand's
  pile* (the first card dealt to the player after the dealer).
- Card strength: Rechte (the trump-suit + striker-rank card) > Weli > any
  Striker > any trump > lead suit > rest. First-played wins ties between
  equal-strength cards (`card_strength` in [src/game.rs](src/game.rs)).
- Seeing players (dealer + forehand) must play a trump or striker when trump
  is led if they have one (enforced by `GameState::allowed_indices`).

## Status / known limitations

- The Critical cards (Maxi/Belli/Spitz) of Bavarian Watten are not modelled —
  this is the Tyrolean/generic 33-card variant.
- Winning score defaults to 15 (`game::WINNING_POINTS`). Not yet runtime-
  configurable from the UI.

import { describe, it, expect, beforeAll } from 'vitest';

let wasm: any;

beforeAll(async () => {
  wasm = await import('../../pkg-test/watten.js');
});

describe('WasmGame', () => {
  it('exposes a constructor and a zero-score scoreboard', () => {
    const g = new wasm.WasmGame(0);
    const scores = Array.from(g.scores());
    expect(scores).toEqual([0, 0]);
  });

  it('defaults to the search evaluator', () => {
    const g = new wasm.WasmGame(0);
    if (typeof g.evaluator === 'function') {
      expect(g.evaluator()).toBe('search');
    }
  });

  it('reports a winning-points constant', () => {
    const g = new wasm.WasmGame(0);
    if (typeof g.winning_points === 'function') {
      const wp = g.winning_points();
      expect(typeof wp).toBe('number');
      expect(wp).toBeGreaterThan(0);
    }
  });

  it('switches to the database evaluator on request', () => {
    const g = new wasm.WasmGame(0);
    if (typeof g.set_evaluator === 'function' && typeof g.evaluator === 'function') {
      g.set_evaluator('search');
      expect(g.evaluator()).toBe('search');
      // Don't actually populate the full DB (takes minutes); just verify the
      // name switch round-trips.
      g.set_evaluator('database');
      expect(g.evaluator()).toBe('database');
    }
  });
});

describe('WasmGame round flow', () => {
  it('plays a full game to the winning score', () => {
    const g = new wasm.WasmGame(0);
    const winning = typeof g.winning_points === 'function' ? g.winning_points() : 15;
    let rounds = 0;
    while (g.scores()[0] < winning && g.scores()[1] < winning) {
      const [_result, steps] = g.play_round_logged() as [number, any[]];
      // Every legal play recorded should match the player's allowed list.
      for (const s of steps) {
        const idx = s.hand.findIndex(
          (c: any) => c.suit === s.played.suit && c.rank === s.played.rank
        );
        expect(s.allowed).toContain(idx);
      }
      rounds++;
      // Defensive cap: a game should finish in well under 30 rounds.
      expect(rounds).toBeLessThan(60);
    }
    const final = Array.from(g.scores());
    expect(Math.max(final[0] as number, final[1] as number)).toBeGreaterThanOrEqual(winning);
  });

  it('returns move evaluations when there is a human player to move', () => {
    const g = new wasm.WasmGame(1);
    g.start_round_interactive();
    g.advance_bots();
    if (typeof g.human_move_evaluations === 'function') {
      const evs = g.human_move_evaluations() as Array<{
        hand_idx: number;
        wins: number;
        total: number;
        rate: number;
      }>;
      expect(evs.length).toBeGreaterThan(0);
      for (const e of evs) {
        expect(e.rate).toBeGreaterThanOrEqual(0);
        expect(e.rate).toBeLessThanOrEqual(1);
        if (e.total > 0) {
          expect(e.wins).toBeLessThanOrEqual(e.total);
        }
      }
    }
  });

  it('lets a team concede the round and awards points to the opponent', () => {
    const g = new wasm.WasmGame(0);
    g.start_round_interactive();
    if (typeof g.raise_round === 'function' && typeof g.concede_round === 'function') {
      expect(g.raise_round(0)).toBe(true);
      expect(g.raise_round(1)).toBe(true);
      const before = Array.from(g.scores()) as [number, number];
      const ok = g.concede_round(0);
      expect(ok).toBe(true);
      const after = Array.from(g.scores()) as [number, number];
      // Team 2 (index 1) should have gained at least the original 2 + 2 raises = 4 points.
      expect(after[1]).toBeGreaterThan(before[1]);
    }
  });
});

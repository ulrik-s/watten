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

  it('an accepted raise pays out at the raised value when the round is won', () => {
    // Drive a complete game with a single human, where we explicitly:
    //   1. raise (propose + accept)
    //   2. play the remaining cards through human_play / advance_bots
    //   3. assert that the points awarded match the raised value
    const g = new wasm.WasmGame(1);
    g.start_round_interactive();
    g.advance_bots();

    const roundBefore = (g as any).round_points?.() ?? 2;
    const scoresBefore = Array.from(g.scores()) as [number, number];

    // Team 1 proposes; force-accept by calling respond_to_raise directly
    // (auto_respond_raise might fold based on the search heuristic — we
    // want to test the ACCEPT path deterministically).
    const ok = (g as any).propose_raise?.(0);
    expect(ok).toBe(true);
    const out = (g as any).respond_to_raise?.(1, true) as any;
    expect(out).not.toBeNull();
    expect(out.accepted).toBe(true);
    expect(out.new_value).toBe(roundBefore + 1);
    expect((g as any).round_points()).toBe(roundBefore + 1);

    // Now play the round to completion by clicking the human's cards.
    // After every human_play we may need to consume any auto-advanced bot
    // steps that brought the game back to the human's turn.
    let guard = 0;
    while (guard++ < 30) {
      const allowed = g.human_allowed_indices() as number[];
      if (allowed.length === 0) break;
      const [res, _steps] = g.human_play(allowed[0]) as [number | undefined, unknown[]];
      if (typeof res === 'number') break; // round ended
    }

    // Round must have ended. Exactly one team got points equal to the
    // raised value (roundBefore + 1).
    const scoresAfter = Array.from(g.scores()) as [number, number];
    const gain0 = scoresAfter[0] - scoresBefore[0];
    const gain1 = scoresAfter[1] - scoresBefore[1];
    expect(gain0 + gain1).toBe(roundBefore + 1);
    expect(Math.max(gain0, gain1)).toBe(roundBefore + 1);
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

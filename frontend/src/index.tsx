import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import init, { WasmGame } from '../pkg/watten';
import './table.css';
import { CardView } from './Card';

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

interface MoveEval {
  hand_idx: number;
  wins: number;
  total: number;
  rate: number;
}

const NUM_PLAYERS = 4;
const TRICK_DISPLAY_MS = 800;

const App = () => {
  const [game, setGame] = useState<WasmGame | null>(null);
  const [hand, setHand] = useState<JsCard[]>([]);
  const [allowed, setAllowed] = useState<number[]>([]);
  const [evals, setEvals] = useState<MoveEval[]>([]);
  const [log, setLog] = useState<string[]>([]);
  const [trick, setTrick] = useState<Array<{ card: JsCard; player: number }>>([]);
  const [opponentHandSizes, setOpponentHandSizes] = useState<number[]>([5, 5, 5, 5]);
  const [scores, setScores] = useState<[number, number]>([0, 0]);
  const [trump, setTrump] = useState<string | null>(null);
  const [striker, setStriker] = useState<string | null>(null);
  const [roundPoints, setRoundPoints] = useState(2);
  const [winningPoints, setWinningPoints] = useState(15);
  const [gameOver, setGameOver] = useState<null | { winner: 1 | 2; final: [number, number] }>(null);
  const [busy, setBusy] = useState(false);
  const trickClearTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    init().then(() => {
      const g = new WasmGame(1);
      setWinningPoints((g as any).winning_points?.() ?? 15);
      g.start_round_interactive();
      setGame(g);
      setTrump(g.trump_suit());
      setStriker(g.striker_rank());
      const [_, steps] = g.advance_bots() as [number | null, JsRoundStep[]];
      processSteps(g, steps, false);
      refreshFromGame(g);
    });
    return () => {
      if (trickClearTimer.current) clearTimeout(trickClearTimer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function refreshFromGame(g: WasmGame) {
    setHand(g.hand(0) as JsCard[]);
    setAllowed(g.human_allowed_indices() as number[]);
    setScores(g.scores() as [number, number]);
    setRoundPoints((g as any).round_points?.() ?? 2);
    const ev = g.human_move_evaluations() as MoveEval[];
    setEvals(ev);
  }

  function handleRoundEnded(g: WasmGame) {
    const s = g.scores() as [number, number];
    setScores(s);
    if (s[0] >= winningPoints || s[1] >= winningPoints) {
      const winner = s[0] >= winningPoints ? 1 : 2;
      setGameOver({ winner, final: s });
      setLog((prev) => [...prev, `Team ${winner} wins the game!`]);
      setBusy(false);
      return true;
    }
    setTimeout(() => {
      g.start_round_interactive();
      setTrump(g.trump_suit());
      setStriker(g.striker_rank());
      setOpponentHandSizes([5, 5, 5, 5]);
      setTrick([]);
      const [, st] = g.advance_bots() as [number | null, JsRoundStep[]];
      processSteps(g, st, false);
      refreshFromGame(g);
      setBusy(false);
    }, TRICK_DISPLAY_MS);
    return false;
  }

  function raise() {
    if (!game || busy || gameOver) return;
    const ok = (game as any).raise_round?.(0);
    if (ok) {
      setRoundPoints((game as any).round_points?.() ?? roundPoints + 1);
      setLog((prev) => [...prev, `Team 1 raises to ${(game as any).round_points()} points`]);
    }
  }

  function concede() {
    if (!game || busy || gameOver) return;
    const ok = (game as any).concede_round?.(0);
    if (!ok) return;
    setLog((prev) => [...prev, `Team 1 concedes; Team 2 takes ${roundPoints} points`]);
    setBusy(true);
    handleRoundEnded(game);
  }

  function computeOpponentHandSizes(steps: JsRoundStep[], base: number[]): number[] {
    const next = [...base];
    for (const s of steps) {
      next[s.player] = Math.max(0, next[s.player] - 1);
    }
    return next;
  }

  function processSteps(g: WasmGame, steps: JsRoundStep[], roundEnded: boolean) {
    if (steps.length === 0) return;
    setLog((prev) => [
      ...prev,
      ...steps.map((s) => `Player ${s.player + 1} plays ${s.played.rank} of ${s.played.suit}`),
    ]);
    setOpponentHandSizes((prev) => computeOpponentHandSizes(steps, prev));

    // Visually accumulate cards into the trick area; when 4 are reached, keep
    // them visible briefly before clearing for the next trick.
    setTrick((prev) => {
      const combined = [...prev];
      for (const s of steps) {
        combined.push({ card: s.played, player: s.player });
        if (combined.length === NUM_PLAYERS) {
          // schedule clear
          if (trickClearTimer.current) clearTimeout(trickClearTimer.current);
          trickClearTimer.current = setTimeout(() => {
            setTrick((cur) => (cur.length === NUM_PLAYERS ? [] : cur));
          }, TRICK_DISPLAY_MS);
        } else if (combined.length > NUM_PLAYERS) {
          // start of next trick — drop the prior completed trick
          combined.splice(0, combined.length - 1);
          if (trickClearTimer.current) {
            clearTimeout(trickClearTimer.current);
            trickClearTimer.current = null;
          }
        }
      }
      return combined;
    });
  }

  function play(idx: number) {
    if (!game || busy || gameOver) return;
    setBusy(true);
    setEvals([]);
    setAllowed([]);
    // Defer to allow UI to update before search runs
    setTimeout(() => {
      const [res, steps] = game.human_play(idx) as [number | null, JsRoundStep[]];
      processSteps(game, steps, res !== null);
      if (res !== null) {
        handleRoundEnded(game);
      } else {
        refreshFromGame(game);
        setBusy(false);
      }
    }, 0);
  }

  function renderOpponent(playerIdx: number, size: number, gridArea: string) {
    return (
      <div className={`player ${gridArea}`}>
        <div className="player-label">P{playerIdx + 1}</div>
        <div className="player-cards">
          {Array.from({ length: size }).map((_, i) => (
            <CardView key={i} suit="Hearts" rank="" faceDown />
          ))}
        </div>
      </div>
    );
  }

  const evalByIdx = new Map<number, MoveEval>();
  for (const e of evals) evalByIdx.set(e.hand_idx, e);

  return (
    <div>
      <h1>Watten</h1>
      <p>
        Trump: {trump ?? '-'} &nbsp;·&nbsp; Striker: {striker ?? '-'}
      </p>
      <p>
        Scores: Team 1 {scores[0]} — Team 2 {scores[1]} (to {winningPoints})
        &nbsp;·&nbsp; Round worth: {roundPoints}
        {busy ? ' · thinking…' : ''}
      </p>
      <div className="actions">
        <button onClick={raise} disabled={!game || busy || !!gameOver}>
          Raise (+1)
        </button>
        <button onClick={concede} disabled={!game || busy || !!gameOver}>
          Concede round
        </button>
      </div>
      {gameOver && (
        <p className="game-over">
          Game over. Team {gameOver.winner} wins {gameOver.final[0]}–{gameOver.final[1]}.
        </p>
      )}
      <div className="table">
        {renderOpponent(2, opponentHandSizes[2], 'p2')}
        {renderOpponent(1, opponentHandSizes[1], 'p3')}
        {renderOpponent(3, opponentHandSizes[3], 'p4')}
        <div className="center">
          {trick.map((t, i) => (
            <div key={i} className="trick-card">
              <CardView suit={t.card.suit} rank={t.card.rank} />
              <div className="trick-label">P{t.player + 1}</div>
            </div>
          ))}
        </div>
        <div className="player hand">
          {hand.map((c, i) => {
            const e = evalByIdx.get(i);
            const rate = e ? Math.round(e.rate * 100) : null;
            return (
              <div key={i} className="hand-slot">
                <CardView
                  suit={c.suit}
                  rank={c.rank}
                  selectable={allowed.includes(i) && !busy && !gameOver}
                  onClick={() => play(i)}
                />
                <div className="card-rate">
                  {rate !== null && allowed.includes(i) ? `${rate}%` : ''}
                </div>
              </div>
            );
          })}
        </div>
      </div>
      <div className="log">
        {log.slice(-12).map((l, i) => (
          <div key={i}>{l}</div>
        ))}
      </div>
    </div>
  );
};

const root = createRoot(document.getElementById('root')!);
root.render(<App />);

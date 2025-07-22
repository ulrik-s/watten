import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import init, { WasmGame } from '../../pkg/watten';
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

const App = () => {
  const PROGRESS_TOTAL = 120 ** 4;
  const [game, setGame] = useState<WasmGame | null>(null);
  const [hand, setHand] = useState<JsCard[]>([]);
  const [allowed, setAllowed] = useState<number[]>([]);
  const [log, setLog] = useState<string[]>([]);
  const [trick, setTrick] = useState<JsCard[]>([]);
  const [scores, setScores] = useState<[number, number]>([0, 0]);
  const [trump, setTrump] = useState<string | null>(null);
  const [striker, setStriker] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    init().then(() => {
      const g = new WasmGame(1);
      if ((g as any).set_workers) {
        (g as any).set_workers(navigator.hardwareConcurrency || 4);
      }
      if ((g as any).set_progress_callback) {
        (g as any).set_progress_callback((c: number) => {
          setProgress(c);
        });
      }
      g.start_round_interactive();
      setGame(g);
      setTrump(g.trump_suit());
      setStriker(g.striker_rank());
      const [res, steps] = g.advance_bots() as [number | null, JsRoundStep[]];
      handleSteps(steps);
      updateHand(g);
      setLoading(false);
    });
  }, []);

  function handleSteps(steps: JsRoundStep[]) {
    const lines = steps.map(
      (s) => `Player ${s.player + 1} plays ${s.played.rank} of ${s.played.suit}`
    );
    setLog((prev) => [...prev, ...lines]);
    setTrick((prev) => {
      const next = [...prev, ...steps.map((s) => s.played)];
      return next.slice(next.length - 4);
    });
  }

  function updateHand(g: WasmGame) {
    setHand(g.hand(0) as JsCard[]);
    setAllowed(g.human_allowed_indices() as number[]);
    setScores(g.scores() as [number, number]);
  }

  function play(idx: number) {
    if (!game) return;
    const [res, steps] = game.human_play(idx) as [number | null, JsRoundStep[]];
    handleSteps(steps);
    updateHand(game);
    if (res !== null) {
      if (game.scores()[0] < 13 && game.scores()[1] < 13) {
        game.start_round_interactive();
        setTrump(game.trump_suit());
        setStriker(game.striker_rank());
        const [_, st] = game.advance_bots() as [number | null, JsRoundStep[]];
        handleSteps(st);
        updateHand(game);
      } else {
        setLog((prev) => [...prev, 'Game Over']);
      }
    }
  }

  if (loading) {
    return (
      <div>
        <h1>Watten</h1>
        <p>Calculating...</p>
        <progress value={progress} max={PROGRESS_TOTAL}></progress>
      </div>
    );
  }

  return (
    <div>
      <h1>Watten</h1>
      <p>Trump: {trump}</p>
      <p>Striker: {striker}</p>
      <p>Scores: {scores[0]} - {scores[1]}</p>
      <div className="table">
        <div className="player p2">
          {Array.from({ length: 5 }).map((_, i) => (
            <CardView key={i} suit="Hearts" rank="" faceDown />
          ))}
        </div>
        <div className="player p3">
          {Array.from({ length: 5 }).map((_, i) => (
            <CardView key={i} suit="Hearts" rank="" faceDown />
          ))}
        </div>
        <div className="player p4">
          {Array.from({ length: 5 }).map((_, i) => (
            <CardView key={i} suit="Hearts" rank="" faceDown />
          ))}
        </div>
        <div className="center">
          {trick.map((c, i) => (
            <CardView key={i} suit={c.suit} rank={c.rank} />
          ))}
        </div>
        <div className="player hand">
          {hand.map((c, i) => (
            <CardView
              key={i}
              suit={c.suit}
              rank={c.rank}
              selectable={allowed.includes(i)}
              onClick={() => play(i)}
            />
          ))}
        </div>
      </div>
      <div>
        {log.map((l, i) => (
          <div key={i}>{l}</div>
        ))}
      </div>
    </div>
  );
};

const root = createRoot(document.getElementById('root')!);
root.render(<App />);


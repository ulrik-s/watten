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

interface TrickEntry {
  card: JsCard;
  player: number;
}

const NUM_PLAYERS = 4;
const CARDS_PER_HAND = 5;

// `?fast=1` query param trims the animation delays so E2E tests can drive
// the full round-by-round play-out in a reasonable time. Real users get
// the normal pacing.
const FAST_MODE =
  typeof window !== 'undefined' &&
  new URLSearchParams(window.location.search).get('fast') === '1';
const STEP_MS = FAST_MODE ? 10 : 320;
const TRICK_HOLD_MS = FAST_MODE ? 50 : 1900;
const ROUND_GAP_MS = FAST_MODE ? 50 : 1800;

function sleep(ms: number) {
  return new Promise<void>((r) => setTimeout(r, ms));
}

// Mirrors the Rust card_strength logic so we can mark the trick winner.
const RANK_VALUES: Record<string, number> = {
  Seven: 1,
  Eight: 2,
  Nine: 3,
  Ten: 4,
  Unter: 5,
  Ober: 6,
  King: 7,
  Ace: 8,
  Weli: 9,
};

function cardStrength(
  card: JsCard,
  leadSuit: string,
  rechte: JsCard,
  position: number
): number {
  let base: number;
  if (card.suit === rechte.suit && card.rank === rechte.rank) {
    base = 200;
  } else if (card.rank === 'Weli') {
    base = 180;
  } else if (card.rank === rechte.rank) {
    base = 190;
  } else if (card.suit === rechte.suit) {
    base = 100 + (RANK_VALUES[card.rank] ?? 0);
  } else if (card.suit === leadSuit) {
    base = 50 + (RANK_VALUES[card.rank] ?? 0);
  } else {
    base = RANK_VALUES[card.rank] ?? 0;
  }
  return base * 10 - position;
}

function trickWinnerIndex(trick: TrickEntry[], rechte: JsCard | null): number {
  if (!rechte || trick.length === 0) return 0;
  const leadSuit = trick[0].card.suit;
  let bestPos = 0;
  let bestScore = cardStrength(trick[0].card, leadSuit, rechte, 0);
  for (let pos = 1; pos < trick.length; pos++) {
    const s = cardStrength(trick[pos].card, leadSuit, rechte, pos);
    if (s > bestScore) {
      bestScore = s;
      bestPos = pos;
    }
  }
  return bestPos;
}

const App = () => {
  const [game, setGame] = useState<WasmGame | null>(null);
  // slots[i] is the card in original-deal position i, or null if the human
  // has already played it this round. Length is always CARDS_PER_HAND so the
  // layout never reflows.
  const [slots, setSlots] = useState<(JsCard | null)[]>(
    Array(CARDS_PER_HAND).fill(null)
  );
  const slotsRef = useRef<(JsCard | null)[]>(Array(CARDS_PER_HAND).fill(null));
  const [allowedSlots, setAllowedSlots] = useState<Set<number>>(new Set());
  const [evalBySlot, setEvalBySlot] = useState<Map<number, MoveEval>>(new Map());
  const [log, setLog] = useState<string[]>([]);
  const [trick, setTrick] = useState<TrickEntry[]>([]);
  const [opponentHandSizes, setOpponentHandSizes] = useState<number[]>([5, 5, 5, 5]);
  const [scores, setScores] = useState<[number, number]>([0, 0]);
  const [trump, setTrump] = useState<string | null>(null);
  const [striker, setStriker] = useState<string | null>(null);
  const [roundPoints, setRoundPoints] = useState(2);
  const [winningPoints, setWinningPoints] = useState(15);
  const [gameOver, setGameOver] = useState<null | { winner: 1 | 2; final: [number, number] }>(null);
  const [busy, setBusy] = useState(false);
  const [roundNumber, setRoundNumber] = useState(1);
  // Brief "Round N is starting" announcement that fades away after the deal.
  const [roundBanner, setRoundBanner] = useState<string | null>(null);
  // Set once concede/fold locks the round winner. The user keeps clicking
  // through their remaining cards, but the round outcome is fixed.
  const [decidedFor, setDecidedFor] = useState<number | null>(null);
  // Last team to have a raise accepted; the alternation rule blocks the
  // same team from raising again until the other team raises.
  const [lastRaiseBy, setLastRaiseBy] = useState<number | null>(null);
  // Position inside `trick` of the winning card, once a trick is full.
  // `null` while the trick is still being played out.
  const [trickWinnerPos, setTrickWinnerPos] = useState<number | null>(null);
  const [rechte, setRechte] = useState<JsCard | null>(null);

  // Authoritative trick state used by the animation loop.
  const trickRef = useRef<TrickEntry[]>([]);
  const logRef = useRef<HTMLDivElement>(null);

  // Auto-scroll the log to its bottom whenever a new entry lands so the
  // latest event is in view.
  useEffect(() => {
    if (logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [log]);

  useEffect(() => {
    init().then(() => {
      const g = new WasmGame(1);
      setWinningPoints((g as any).winning_points?.() ?? 15);
      g.start_round_interactive();
      setGame(g);
      setTrump(g.trump_suit());
      setStriker(g.striker_rank());
      setRechte(((g as any).rechte?.() ?? null) as JsCard | null);
      // Capture the original 5-card hand for the human.
      const orig = g.hand(0) as JsCard[];
      slotsRef.current = padSlots(orig);
      setSlots([...slotsRef.current]);
      // Bots advance to the human's first move; animate their plays in.
      const [, steps] = g.advance_bots() as [number | null, JsRoundStep[]];
      void processStepsAnimated(steps).then(() => refreshFromGame(g));
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function padSlots(hand: JsCard[]): (JsCard | null)[] {
    const out: (JsCard | null)[] = Array(CARDS_PER_HAND).fill(null);
    for (let i = 0; i < hand.length && i < CARDS_PER_HAND; i++) {
      out[i] = hand[i];
    }
    return out;
  }

  function slotToCurrentIdx(slotIdx: number, s: (JsCard | null)[]): number {
    let n = 0;
    for (let i = 0; i < slotIdx; i++) if (s[i]) n++;
    return n;
  }

  function refreshFromGame(g: WasmGame) {
    const currentHand = g.hand(0) as JsCard[];
    const currentAllowed = g.human_allowed_indices() as number[];
    const evs = g.human_move_evaluations() as MoveEval[];

    // Map each card in the current hand back to its slot via card identity.
    const allowedSet = new Set<number>();
    const evalMap = new Map<number, MoveEval>();
    currentHand.forEach((card, currentIdx) => {
      const slot = slotsRef.current.findIndex(
        (c) => c !== null && c.suit === card.suit && c.rank === card.rank
      );
      if (slot < 0) return;
      if (currentAllowed.includes(currentIdx)) allowedSet.add(slot);
      const e = evs.find((ev) => ev.hand_idx === currentIdx);
      if (e) evalMap.set(slot, e);
    });

    setAllowedSlots(allowedSet);
    setEvalBySlot(evalMap);
    setScores(g.scores() as [number, number]);
    setRoundPoints((g as any).round_points?.() ?? 2);
  }

  async function processStepsAnimated(steps: JsRoundStep[]) {
    for (const s of steps) {
      setLog((prev) => [
        ...prev,
        `Player ${s.player + 1} plays ${s.played.rank} of ${s.played.suit}`,
      ]);

      const wasFull = trickRef.current.length >= NUM_PLAYERS;
      const nextTrick = wasFull
        ? [{ card: s.played, player: s.player }]
        : [...trickRef.current, { card: s.played, player: s.player }];
      trickRef.current = nextTrick;
      setTrick(nextTrick);
      // The winner halo only appears once the trick is full; starting a new
      // trick clears it.
      setTrickWinnerPos(null);

      if (s.player !== 0) {
        setOpponentHandSizes((prev) => {
          const next = [...prev];
          next[s.player] = Math.max(0, next[s.player] - 1);
          return next;
        });
      } else {
        // Player 0 = human. Manual plays null the slot pre-emptively in
        // `play()`, but auto-play also produces steps for the human, so
        // empty their slot here when we still have one matching the card.
        const slotIdx = slotsRef.current.findIndex(
          (c) =>
            c !== null && c.suit === s.played.suit && c.rank === s.played.rank
        );
        if (slotIdx >= 0) {
          slotsRef.current = slotsRef.current.map((c, i) =>
            i === slotIdx ? null : c
          );
          setSlots([...slotsRef.current]);
        }
      }

      if (nextTrick.length === NUM_PLAYERS) {
        // Trick completed: highlight the winner and announce it, then hold.
        const winnerPos = trickWinnerIndex(nextTrick, rechte);
        const winnerPlayer = nextTrick[winnerPos].player;
        setTrickWinnerPos(winnerPos);
        setLog((prev) => [...prev, `Player ${winnerPlayer + 1} wins the trick`]);
        await sleep(TRICK_HOLD_MS);
      } else {
        await sleep(STEP_MS);
      }
    }
  }

  async function play(slotIdx: number) {
    if (!game || busy || gameOver) return;
    if (!slotsRef.current[slotIdx]) return;
    if (!allowedSlots.has(slotIdx)) return;
    setBusy(true);

    // Compute the current-hand index for wasm BEFORE nulling the slot.
    const currentIdx = slotToCurrentIdx(slotIdx, slotsRef.current);

    // Optimistic hand-slot removal: the player gets instant feedback that
    // their click registered.
    slotsRef.current = slotsRef.current.map((c, i) => (i === slotIdx ? null : c));
    setSlots([...slotsRef.current]);
    setAllowedSlots(new Set());
    setEvalBySlot(new Map());

    // Drive the actual play through wasm, then animate every returned step
    // (human + bots) uniformly so the trick-winner highlight always fires.
    // NOTE: serde-wasm-bindgen serializes Rust `None` as JS `undefined`, NOT
    // `null`, so use `typeof === 'number'` to detect a real round-end result.
    const [res, steps] = game.human_play(currentIdx) as [
      number | undefined,
      JsRoundStep[]
    ];
    await processStepsAnimated(steps);

    if (typeof res === 'number') {
      await handleRoundEnded(game);
    } else {
      refreshFromGame(game);
      setBusy(false);
    }
  }

  async function handleRoundEnded(g: WasmGame) {
    const s = g.scores() as [number, number];
    setScores(s);
    setLog((prev) => [
      ...prev,
      `Round ${roundNumber} ends. Score: Team 1 ${s[0]} — Team 2 ${s[1]}.`,
    ]);
    if (s[0] >= winningPoints || s[1] >= winningPoints) {
      const winner = s[0] >= winningPoints ? 1 : 2;
      setGameOver({ winner, final: s });
      setLog((prev) => [...prev, `Team ${winner} wins the game!`]);
      setBusy(false);
      return;
    }
    // Pause showing the trick winner / final state of the round, then
    // announce the next round.
    await sleep(TRICK_HOLD_MS);
    const nextRound = roundNumber + 1;
    setRoundBanner(`Round ${nextRound} — new deal`);
    // Clear the table for the new deal.
    trickRef.current = [];
    setTrick([]);
    setTrickWinnerPos(null);
    setDecidedFor(null);
    setLastRaiseBy(null);
    g.start_round_interactive();
    setTrump(g.trump_suit());
    setStriker(g.striker_rank());
    setRechte(((g as any).rechte?.() ?? null) as JsCard | null);
    setOpponentHandSizes([5, 5, 5, 5]);
    const orig = g.hand(0) as JsCard[];
    slotsRef.current = padSlots(orig);
    setSlots([...slotsRef.current]);
    setRoundNumber(nextRound);
    setLog((prev) => [...prev, `--- Round ${nextRound} deal ---`]);
    // Give the user a beat to register the new deal before bots start.
    await sleep(ROUND_GAP_MS);
    setRoundBanner(null);
    const [, st] = g.advance_bots() as [number | null, JsRoundStep[]];
    await processStepsAnimated(st);
    refreshFromGame(g);
    setBusy(false);
  }

  async function playOutRoundIfNeeded(g: WasmGame) {
    // After concede/fold the engine knows the winner; finish the round by
    // animating every remaining card before dealing a new one.
    const out = (g as any).auto_play_round?.() as
      | null
      | { ended: boolean; steps: JsRoundStep[] };
    if (!out) return;
    if (out.steps && out.steps.length > 0) {
      await processStepsAnimated(out.steps);
    }
    if (out.ended) {
      await handleRoundEnded(g);
    } else {
      refreshFromGame(g);
      setBusy(false);
    }
  }

  async function onRaise() {
    if (!game || busy || gameOver) return;
    if (decidedFor !== null) return;
    // Alternation rule: Team 1 cannot raise twice in a row.
    if (lastRaiseBy === 0) {
      setLog((prev) => [
        ...prev,
        `Team 1 cannot raise — Team 2 must raise first.`,
      ]);
      return;
    }
    setBusy(true);
    try {
      const before = (game as any).round_points?.() ?? roundPoints;
      const proposed = before + 1;
      const ok = (game as any).propose_raise?.(0);
      if (!ok) {
        setLog((prev) => [
          ...prev,
          `Team 1 cannot raise right now.`,
        ]);
        setBusy(false);
        return;
      }
      setLog((prev) => [
        ...prev,
        `Team 1 proposes to raise the round to ${proposed} — Team 2 is thinking…`,
      ]);
      await sleep(700);
      const outcome = ((game as any).auto_respond_raise?.() ?? null) as
        | null
        | { accepted: true; new_value: number; proposing_team?: number }
        | { accepted: false; winning_team: number; points: number; ended: boolean };
      if (!outcome) {
        setBusy(false);
        return;
      }
      if (outcome.accepted) {
        setRoundPoints(outcome.new_value);
        setLastRaiseBy(0);
        setLog((prev) => [
          ...prev,
          `Team 2 accepts. Round is now worth ${outcome.new_value}.`,
        ]);
        setBusy(false);
      } else {
        // Fold: the proposing team (Team 1) wins the round at the pre-raise
        // value. The hand is NOT auto-played — the user continues clicking
        // through their remaining cards; the score lands at finish_round.
        setDecidedFor(outcome.winning_team);
        setLog((prev) => [
          ...prev,
          `Team 2 folds. Team ${outcome.winning_team + 1} will take ${outcome.points} points when the round ends. Keep playing.`,
        ]);
        setBusy(false);
      }
    } catch (err) {
      setBusy(false);
      throw err;
    }
  }

  async function onConcede() {
    if (!game || busy || gameOver) return;
    if (decidedFor !== null) return;
    const ok = (game as any).concede_round?.(0);
    if (!ok) return;
    setDecidedFor(1);
    setLog((prev) => [
      ...prev,
      `Team 1 concedes. Team 2 will take ${roundPoints} points when the round ends. Keep playing.`,
    ]);
    // No auto-play and no setBusy — the user keeps clicking through their
    // remaining cards normally.
  }

  function renderOpponent(playerIdx: number, size: number, gridArea: string) {
    return (
      <div className={`player ${gridArea}`}>
        <div className="player-label">P{playerIdx + 1}</div>
        <div className="player-cards">
          {Array.from({ length: CARDS_PER_HAND }).map((_, i) => (
            <div key={i} className="opp-slot">
              {i < size ? <CardView suit="Hearts" rank="" faceDown /> : null}
            </div>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <h1>Watten</h1>
      <p className="info">
        Round <strong>{roundNumber}</strong>
        &nbsp;·&nbsp; Trump: <strong>{trump ?? '-'}</strong>
        &nbsp;·&nbsp; Striker: <strong>{striker ?? '-'}</strong>
      </p>
      <p className="info">
        Scores: Team 1 {scores[0]} — Team 2 {scores[1]} (to {winningPoints})
        &nbsp;·&nbsp; Round worth: {roundPoints}
        {busy ? ' · thinking…' : ''}
      </p>
      {roundBanner && (
        <p className="round-banner" data-testid="round-banner">
          {roundBanner}
        </p>
      )}
      <div className="actions">
        <button
          onClick={onRaise}
          disabled={
            !game || busy || !!gameOver || decidedFor !== null || lastRaiseBy === 0
          }
          title={
            lastRaiseBy === 0
              ? 'Team 2 must raise first (alternation rule)'
              : decidedFor !== null
              ? 'Round outcome is already decided'
              : ''
          }
        >
          Raise (+1)
        </button>
        <button
          onClick={onConcede}
          disabled={!game || busy || !!gameOver || decidedFor !== null}
          title={decidedFor !== null ? 'Round outcome is already decided' : ''}
        >
          Concede round
        </button>
      </div>
      {decidedFor !== null && (
        <p className="round-decided" data-testid="round-decided">
          Round outcome locked in: Team {decidedFor + 1} will take the round
          when all cards are played.
        </p>
      )}
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
          {Array.from({ length: NUM_PLAYERS }).map((_, i) => {
            const t = trick[i];
            const isWinner = trickWinnerPos === i;
            return (
              <div
                key={i}
                className={`trick-slot${isWinner ? ' winner' : ''}`}
              >
                {t ? (
                  <>
                    <CardView suit={t.card.suit} rank={t.card.rank} />
                    <div className="trick-label">
                      P{t.player + 1}
                      {isWinner ? ' ★' : ''}
                    </div>
                  </>
                ) : null}
              </div>
            );
          })}
        </div>
        <div className="trick-banner-wrap">
          {trickWinnerPos !== null && trick[trickWinnerPos] ? (
            <div className="trick-banner" data-testid="trick-winner">
              Player {trick[trickWinnerPos].player + 1} wins the trick
            </div>
          ) : null}
        </div>
        <div className="player hand">
          {slots.map((c, slotIdx) => {
            const e = c ? evalBySlot.get(slotIdx) : undefined;
            const rate = e ? Math.round(e.rate * 100) : null;
            const selectable = !!c && allowedSlots.has(slotIdx) && !busy && !gameOver;
            return (
              <div key={slotIdx} className="hand-slot">
                {c ? (
                  <CardView
                    suit={c.suit}
                    rank={c.rank}
                    selectable={selectable}
                    onClick={() => play(slotIdx)}
                  />
                ) : (
                  <div className="card placeholder" aria-hidden="true" />
                )}
                <div className="card-rate">
                  {rate !== null && allowedSlots.has(slotIdx) ? `${rate}%` : ''}
                </div>
              </div>
            );
          })}
        </div>
      </div>
      <div className="log" ref={logRef}>
        {log.map((l, i) => (
          <div key={`${i}-${l}`}>{l}</div>
        ))}
      </div>
    </div>
  );
};

const root = createRoot(document.getElementById('root')!);
root.render(<App />);

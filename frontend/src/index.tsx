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
  illegal: number;
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
// Hold the completed trick on screen long enough that a non-seer (who
// doesn't see trump/striker on the header) can reason about why a card
// won. 3.5 s is enough to read four cards and the winner banner.
const TRICK_HOLD_MS = FAST_MODE ? 50 : 3500;
const ROUND_GAP_MS = FAST_MODE ? 50 : 1800;

function sleep(ms: number) {
  return new Promise<void>((r) => setTimeout(r, ms));
}

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

const RANK_DISPLAY: Record<string, string> = {
  Seven: '7',
  Eight: '8',
  Nine: '9',
  Ten: '10',
  Unter: 'Unter',
  Ober: 'Ober',
  King: 'King',
  Ace: 'Ace',
  Weli: 'Weli',
};
const displayRank = (r: string): string => RANK_DISPLAY[r] ?? r;

// Per the user's spec:
//   round_score = (trump_suit ? 100 : 0) + (striker_rank ? 200 : 0)
//   trick_score:
//     - if card is striker rank AND an earlier striker was played → -400
//     - else if card.suit === lead_suit                            → rank_value
//     - else                                                       → 0
//   total = round_score + trick_score  (compare with strict >, earlier wins ties)
function roundScore(card: JsCard, rechte: JsCard): number {
  let s = 0;
  if (card.suit === rechte.suit) s += 100;
  if (card.rank === rechte.rank) s += 200;
  return s;
}

function trickScore(
  card: JsCard,
  position: number,
  trick: TrickEntry[],
  rechte: JsCard
): number {
  if (card.rank === rechte.rank) {
    for (let earlier = 0; earlier < position; earlier++) {
      if (trick[earlier].card.rank === rechte.rank) return -400;
    }
  }
  const leadSuit = trick[0].card.suit;
  const rv = RANK_VALUES[card.rank] ?? 0;
  return card.suit === leadSuit ? rv + 20 : rv;
}

function cardTotalScore(
  card: JsCard,
  position: number,
  trick: TrickEntry[],
  rechte: JsCard
): number {
  return roundScore(card, rechte) + trickScore(card, position, trick, rechte);
}

function trickWinnerIndex(trick: TrickEntry[], rechte: JsCard | null): number {
  if (!rechte || trick.length === 0) return 0;
  let bestPos = 0;
  let bestScore = cardTotalScore(trick[0].card, 0, trick, rechte);
  for (let pos = 1; pos < trick.length; pos++) {
    const s = cardTotalScore(trick[pos].card, pos, trick, rechte);
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
  const [tricksThisRound, setTricksThisRound] = useState<[number, number]>([0, 0]);
  const [showDebug, setShowDebug] = useState(false);
  const [useDatabaseEvaluator, setUseDatabaseEvaluator] = useState(false);
  const [evaluatorBusy, setEvaluatorBusy] = useState(false);
  // 0..1 progress for the database populate, or null when idle.
  const [dbProgress, setDbProgress] = useState<number | null>(null);
  const cancelDbPopulate = useRef(false);
  const [trump, setTrump] = useState<string | null>(null);
  const [striker, setStriker] = useState<string | null>(null);
  const [roundPoints, setRoundPoints] = useState(2);
  const [winningPoints, setWinningPoints] = useState(13);
  const [raiseLockoutScore, setRaiseLockoutScore] = useState(10);
  const [dealer, setDealer] = useState(0);
  const [humanIsSeer, setHumanIsSeer] = useState(true);
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
  // rechte mirrored in a ref so async closures (e.g. processStepsAnimated
  // started from useEffect on first load) read the *latest* value rather
  // than whatever was in state when the closure was captured. Without this,
  // the displayed trick winner falls back to the lead because the initial
  // render's `rechte` is still null when the first bots' steps animate.
  const rechteRef = useRef<JsCard | null>(null);
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
      setWinningPoints((g as any).winning_points?.() ?? 13);
      setRaiseLockoutScore((g as any).raise_lockout_score?.() ?? 10);
      g.start_round_interactive();
      setGame(g);
      setTrump(g.trump_suit());
      setStriker(g.striker_rank());
      const r = ((g as any).rechte?.() ?? null) as JsCard | null;
      rechteRef.current = r;
      setRechte(r);
      const d = ((g as any).dealer?.() ?? 0) as number;
      setDealer(d);
      setHumanIsSeer(
        ((g as any).is_seer?.(0) ?? (d === 0 || d === 3)) as boolean
      );
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

  function refreshTricksThisRound(g: WasmGame) {
    const t = ((g as any).tricks_won?.() ?? null) as number[] | null;
    if (t && t.length >= 2) {
      setTricksThisRound([t[0], t[1]]);
    }
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
    refreshTricksThisRound(g);
  }

  async function processStepsAnimated(steps: JsRoundStep[]) {
    for (const s of steps) {
      setLog((prev) => [
        ...prev,
        `Player ${s.player + 1} plays ${displayRank(s.played.rank)} of ${s.played.suit}`,
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
        const winnerPos = trickWinnerIndex(nextTrick, rechteRef.current);
        const winnerPlayer = nextTrick[winnerPos].player;
        setTrickWinnerPos(winnerPos);
        setLog((prev) => [...prev, `Player ${winnerPlayer + 1} wins the trick`]);
        // Update the team's trick-count for this round visibly the moment
        // the trick completes.
        setTricksThisRound((prev) => {
          const next = [...prev] as [number, number];
          next[winnerPlayer % 2] += 1;
          return next;
        });
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
    setTricksThisRound([0, 0]);
    g.start_round_interactive();
    setTrump(g.trump_suit());
    setStriker(g.striker_rank());
    const r = ((g as any).rechte?.() ?? null) as JsCard | null;
    rechteRef.current = r;
    setRechte(r);
    const d = ((g as any).dealer?.() ?? 0) as number;
    setDealer(d);
    setHumanIsSeer(
      ((g as any).is_seer?.(0) ?? (d === 0 || d === 3)) as boolean
    );
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

  async function onToggleEvaluator(useDb: boolean) {
    if (!game || evaluatorBusy) return;
    if (!useDb) {
      // Switching back to search is instant.
      (game as any).set_evaluator?.('search');
      setUseDatabaseEvaluator(false);
      setDbProgress(null);
      setLog((prev) => [...prev, 'Switched to fast search evaluator.']);
      refreshFromGame(game);
      return;
    }
    // Switching ON: chunked populate with a progress bar. We pump batches
    // of plays inside wasm and yield to the JS event loop between them so
    // the percentage updates and the page stays responsive.
    cancelDbPopulate.current = false;
    setEvaluatorBusy(true);
    setDbProgress(0);
    setLog((prev) => [
      ...prev,
      'Starting 120⁴ database populate…',
    ]);
    // Defer one tick so React paints the 0% bar before wasm starts.
    await sleep(0);
    const total = (game as any).database_populate_begin?.() as number;
    if (!total || total <= 0) {
      setEvaluatorBusy(false);
      setDbProgress(null);
      setLog((prev) => [...prev, 'Database populate could not start.']);
      return;
    }
    // Pick a batch size so we yield ~10 times per second on a typical run.
    // Smaller = smoother bar, larger = less overhead. 200k feels right.
    const BATCH = 200_000;
    let done = 0;
    while (done < total) {
      if (cancelDbPopulate.current) {
        // User toggled the checkbox back off while loading.
        (game as any).set_evaluator?.('search');
        setDbProgress(null);
        setUseDatabaseEvaluator(false);
        setEvaluatorBusy(false);
        setLog((prev) => [...prev, 'Database populate cancelled.']);
        return;
      }
      const out = (game as any).database_populate_step?.(BATCH) as
        | null
        | { done: number; total: number; complete: boolean };
      if (!out) break;
      done = out.done;
      setDbProgress(out.done / out.total);
      // Yield to JS so the progress bar can repaint.
      await sleep(0);
      if (out.complete) break;
    }
    setUseDatabaseEvaluator(true);
    setDbProgress(1);
    setLog((prev) => [
      ...prev,
      `120⁴ database populate complete (${total.toLocaleString()} games).`,
    ]);
    refreshFromGame(game);
    setEvaluatorBusy(false);
    // Drop the progress bar after a brief pause.
    setTimeout(() => setDbProgress(null), 800);
  }

  async function onRaise() {
    if (!game || busy || gameOver) return;
    if (decidedFor !== null) return;
    // Lockout: a team at >= RAISE_LOCKOUT_SCORE may not propose raises.
    if (scores[0] >= raiseLockoutScore) {
      setLog((prev) => [
        ...prev,
        `Team 1 cannot raise — at ${scores[0]} points, the raise lockout (${raiseLockoutScore}+) is in effect.`,
      ]);
      return;
    }
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
    const team = playerIdx % 2; // 0 = Team 1 (P1+P3), 1 = Team 2 (P2+P4)
    const relation = team === 0 ? 'teammate' : 'opponent';
    return (
      <div className={`player ${gridArea} team-${team + 1} ${relation}`}>
        <div className="player-label">
          P{playerIdx + 1}
          <span className="player-team-tag">{team === 0 ? 'T1' : 'T2'}</span>
        </div>
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
      <p className="info" data-testid="round-info">
        Round <strong>{roundNumber}</strong>
        &nbsp;·&nbsp; Dealer: <strong>P{dealer + 1}</strong>
        {(humanIsSeer || showDebug) ? (
          <>
            &nbsp;·&nbsp; Trump:{' '}
            <strong data-testid="trump-display">{trump ?? '-'}</strong>
            &nbsp;·&nbsp; Striker:{' '}
            <strong data-testid="striker-display">{striker ?? '-'}</strong>
            {!humanIsSeer && showDebug ? (
              <span className="debug-note"> (debug)</span>
            ) : null}
          </>
        ) : (
          <span className="hidden-trump-hint">
            &nbsp;·&nbsp; <em>Trump & Striker hidden — you're not a seer this round</em>
          </span>
        )}
      </p>
      <p className="info">
        Scores: Team 1 {scores[0]} — Team 2 {scores[1]} (to {winningPoints})
        &nbsp;·&nbsp; Round worth: {roundPoints}
        {busy ? ' · thinking…' : ''}
      </p>
      <p className="info" data-testid="tricks-this-round">
        Tricks this round: Team 1 <strong>{tricksThisRound[0]}</strong>
        &nbsp;·&nbsp; Team 2 <strong>{tricksThisRound[1]}</strong>
      </p>
      <label className="debug-toggle">
        <input
          type="checkbox"
          checked={showDebug}
          onChange={(e) => setShowDebug(e.target.checked)}
          data-testid="show-debug"
        />
        Show scores
      </label>
      &nbsp;&nbsp;
      <label className="debug-toggle">
        <input
          type="checkbox"
          checked={useDatabaseEvaluator || evaluatorBusy}
          disabled={!game || evaluatorBusy}
          onChange={(e) => onToggleEvaluator(e.target.checked)}
          data-testid="toggle-db-evaluator"
        />
        Use full 120<sup>4</sup> database (slow)
      </label>
      {dbProgress !== null && (
        <div className="db-progress" data-testid="db-progress">
          <div className="db-progress-bar">
            <div
              className="db-progress-fill"
              style={{ width: `${Math.round(dbProgress * 100)}%` }}
            />
          </div>
          <span className="db-progress-label">
            Loading 120⁴ database… <strong>{Math.round(dbProgress * 100)}%</strong>
          </span>
        </div>
      )}
      {roundBanner && (
        <p className="round-banner" data-testid="round-banner">
          {roundBanner}
        </p>
      )}
      <div className="actions">
        <button
          onClick={onRaise}
          disabled={
            !game ||
            busy ||
            !!gameOver ||
            decidedFor !== null ||
            lastRaiseBy === 0 ||
            scores[0] >= raiseLockoutScore
          }
          title={
            scores[0] >= raiseLockoutScore
              ? `Team 1 has reached ${raiseLockoutScore} points and may not propose raises any more`
              : lastRaiseBy === 0
              ? 'Team 2 must raise first (alternation rule)'
              : decidedFor !== null
              ? 'Round outcome is already decided'
              : ''
          }
          data-testid="raise-button"
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
          Round outcome locked in: Team {decidedFor + 1} will take{' '}
          <strong>{roundPoints}</strong> point{roundPoints === 1 ? '' : 's'} when
          all cards are played.
        </p>
      )}
      {gameOver && (
        <p className="game-over">
          Game over. Team {gameOver.winner} wins {gameOver.final[0]}–{gameOver.final[1]}.
        </p>
      )}
      <p className="info team-legend" data-testid="team-info">
        <span className="team-1-chip">Team 1</span>: You (P1) &amp; P3 (across) &nbsp;·&nbsp;
        <span className="team-2-chip">Team 2</span>: P2 &amp; P4 (opponents)
      </p>
      <div className="table">
        {/* Seating: teammates sit opposite each other.
            Bottom: P1 (you, idx 0, Team 1).
            Top:    P3 (idx 2, Team 1) — your teammate, directly across.
            Left:   P2 (idx 1, Team 2) — opponent.
            Right:  P4 (idx 3, Team 2) — opponent.
            With this layout, the dealer & forehand (any two consecutive
            players) are always one from each team. */}
        {renderOpponent(1, opponentHandSizes[1], 'p2')}
        {renderOpponent(2, opponentHandSizes[2], 'p3')}
        {renderOpponent(3, opponentHandSizes[3], 'p4')}
        <div className="center">
          {Array.from({ length: NUM_PLAYERS }).map((_, i) => {
            const t = trick[i];
            const isWinner = trickWinnerPos === i;
            const r = rechte;
            const rs = t && r ? roundScore(t.card, r) : null;
            const ts = t && r ? trickScore(t.card, i, trick, r) : null;
            return (
              <div
                key={i}
                className={`trick-slot${isWinner ? ' winner' : ''}`}
              >
                {t ? (
                  <>
                    <CardView suit={t.card.suit} rank={displayRank(t.card.rank)} />
                    <div className={`trick-label team-${(t.player % 2) + 1}`}>
                      P{t.player + 1}
                      <span className="trick-team-tag">
                        {t.player % 2 === 0 ? 'T1' : 'T2'}
                      </span>
                      {isWinner ? ' ★' : ''}
                    </div>
                    {showDebug && rs !== null && ts !== null && (
                      <div className="card-debug" data-testid="trick-debug">
                        R:{rs} T:{ts}
                        <br />={rs + ts}
                      </div>
                    )}
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
        <div className="player hand team-1">
          {slots.map((c, slotIdx) => {
            const e = c ? evalBySlot.get(slotIdx) : undefined;
            const rate = e ? Math.round(e.rate * 100) : null;
            const selectable = !!c && allowedSlots.has(slotIdx) && !busy && !gameOver;
            return (
              <div key={slotIdx} className="hand-slot">
                {c ? (
                  <CardView
                    suit={c.suit}
                    rank={displayRank(c.rank)}
                    selectable={selectable}
                    onClick={() => play(slotIdx)}
                  />
                ) : (
                  <div className="card placeholder" aria-hidden="true" />
                )}
                <div className="card-rate">
                  {rate !== null && allowedSlots.has(slotIdx) ? `${rate}%` : ''}
                </div>
                {showDebug && c && rechte && (() => {
                  const me = evalBySlot.get(slotIdx);
                  return (
                    <div className="card-debug" data-testid="hand-debug">
                      R:{roundScore(c, rechte)}
                      {me ? (
                        <>
                          <br />W:{me.wins} L:{me.total - me.wins}
                          {me.illegal > 0 ? <> I:{me.illegal}</> : null}
                        </>
                      ) : null}
                    </div>
                  );
                })()}
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

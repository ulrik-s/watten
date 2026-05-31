import type { WasmGame } from '../pkg/watten';
import type { JsCard } from './scoring';

/** One animated play in a trick, as serialized by the wasm layer. */
export interface JsRoundStep {
  player: number;
  hand: JsCard[];
  allowed: number[];
  played: JsCard;
}

/** Per-card win-rate evaluation for the player on the move. */
export interface MoveEval {
  hand_idx: number;
  wins: number;
  total: number;
  illegal: number;
  rate: number;
}

/** `[round_result_or_undefined, animation_steps]` returned by play methods. */
export type PlayResult = [number | undefined, JsRoundStep[]];

/** Outcome of an auto-resolved raise proposal. */
export type RaiseOutcome =
  | { accepted: true; new_value: number; proposing_team?: number }
  | { accepted: false; winning_team: number; points: number; ended: boolean };

/** Result of finishing a decided round via `auto_play_round`. */
export interface AutoPlayResult {
  ended: boolean;
  steps: JsRoundStep[];
}

/** Progress report from one chunk of the 120⁴ database populate. */
export interface PopulateStep {
  done: number;
  total: number;
  complete: boolean;
}

/**
 * The methods whose generated return type is `any` (serde-serialized
 * values). We strip them off the base type with `Omit` and restate them
 * below — extending the class directly would *merge* the `any` signatures
 * as overloads instead of replacing them, defeating the point.
 */
type SerdeReturningMethods =
  | 'rechte'
  | 'current_player'
  | 'hand'
  | 'human_allowed_indices'
  | 'human_move_evaluations'
  | 'move_evaluations_for'
  | 'advance_bots'
  | 'human_play'
  | 'human_play_no_advance'
  | 'advance_one_bot'
  | 'auto_play_round'
  | 'auto_respond_raise'
  | 'database_populate_step';

/**
 * The wasm-bindgen-generated bindings type every serde-serialized return as
 * `any`. This interface restates the real runtime shapes so the React app is
 * fully type-checked. `WasmGame` is structurally assignable to it, so a
 * plain annotation is enough:
 *
 * ```ts
 * const g: TypedWasmGame = new WasmGame(1);
 * ```
 */
export interface TypedWasmGame extends Omit<WasmGame, SerdeReturningMethods> {
  rechte(): JsCard | null;
  current_player(): number | null;
  hand(idx: number): JsCard[];
  human_allowed_indices(): number[];
  human_move_evaluations(): MoveEval[];
  move_evaluations_for(p: number): MoveEval[];
  advance_bots(): PlayResult;
  human_play(idx: number): PlayResult;
  human_play_no_advance(idx: number): PlayResult;
  advance_one_bot(): PlayResult | undefined;
  auto_play_round(): AutoPlayResult | null;
  auto_respond_raise(): RaiseOutcome | null;
  database_populate_step(batch: number): PopulateStep | null;
}

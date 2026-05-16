use serde::{Deserialize, Serialize};
use serde_wasm_bindgen as swb;
use wasm_bindgen::prelude::*;

use crate::game::{Evaluator, GameState};
use crate::{Rank, Suit};

#[wasm_bindgen]
pub struct WasmGame {
    inner: GameState,
}

#[derive(Serialize, Deserialize)]
pub struct JsCard {
    suit: Suit,
    rank: Rank,
}

#[derive(Serialize, Deserialize)]
pub struct JsRoundStep {
    player: usize,
    hand: Vec<JsCard>,
    allowed: Vec<usize>,
    played: JsCard,
}

#[wasm_bindgen]
impl WasmGame {
    #[wasm_bindgen(constructor)]
    pub fn new(humans: usize) -> WasmGame {
        WasmGame {
            inner: GameState::new(humans),
        }
    }

    pub fn start_round(&mut self) {
        self.inner.start_round();
    }

    pub fn start_round_interactive(&mut self) {
        self.inner.start_round_interactive();
    }

    /// Limit the permutation range used for database population. Providing a
    /// single permutation drastically speeds up simulations and is useful in
    /// tests.
    pub fn set_perm_range_single(&mut self, idx: usize) {
        self.inner.set_perm_range_single(idx);
    }

    /// Limit the permutation range used for database population to a custom
    /// list of permutation indices.
    pub fn set_perm_range(&mut self, indices: js_sys::Array) {
        let mut vec = Vec::new();
        for v in indices.iter() {
            if let Some(n) = v.as_f64() {
                vec.push(n as usize);
            }
        }
        self.inner.set_perm_range(vec);
    }

    /// Clear any permutation range so that all permutations are considered
    /// again.
    pub fn clear_perm_range(&mut self) {
        self.inner.clear_perm_range();
    }

    /// Set the number of worker threads used for database population.
    pub fn set_workers(&mut self, workers: usize) {
        self.inner.set_workers(workers);
    }

    /// Register a JavaScript callback for database population progress.
    pub fn set_progress_callback(&mut self, cb: js_sys::Function) {
        self.inner.set_progress_callback(Some(Box::new(move |v| {
            let _ = cb.call1(&JsValue::NULL, &JsValue::from(v));
        })));
    }

    pub fn play_round(&mut self) -> u8 {
        self.inner.play_round() as u8
    }

    /// Propose a raise on behalf of `team` (0 or 1). The round value does
    /// NOT change yet — the opposing team must respond. Returns true if
    /// the proposal was recorded.
    pub fn propose_raise(&mut self, team: usize) -> bool {
        self.inner.propose_raise(team).is_ok()
    }

    /// Which team has an outstanding raise proposal, if any.
    pub fn pending_raise(&self) -> Option<usize> {
        self.inner.pending_raise()
    }

    /// Respond to the pending raise on behalf of `team` (the team that
    /// did *not* propose). Returns a JSON-shaped object describing the
    /// outcome:
    ///   `{ accepted: true, new_value: N }`            on accept
    ///   `{ accepted: false, winning_team: T, points: N, ended: true }` on fold
    /// Returns `null` if there's no pending raise or the call is invalid.
    pub fn respond_to_raise(&mut self, team: usize, accept: bool) -> JsValue {
        match self.inner.respond_to_raise(team, accept) {
            Ok(crate::game::RaiseOutcome::Accepted {
                proposing_team,
                new_value,
            }) => {
                #[derive(Serialize)]
                struct Out {
                    accepted: bool,
                    proposing_team: usize,
                    new_value: usize,
                }
                swb::to_value(&Out {
                    accepted: true,
                    proposing_team,
                    new_value,
                })
                .unwrap_or(JsValue::NULL)
            }
            Ok(crate::game::RaiseOutcome::Folded {
                winning_team,
                points,
            }) => {
                #[derive(Serialize)]
                struct Out {
                    accepted: bool,
                    winning_team: usize,
                    points: usize,
                    ended: bool,
                }
                swb::to_value(&Out {
                    accepted: false,
                    winning_team,
                    points,
                    ended: true,
                })
                .unwrap_or(JsValue::NULL)
            }
            Err(_) => JsValue::NULL,
        }
    }

    /// Decide the response to a pending raise on behalf of the *opposing*
    /// team using the move evaluator as a heuristic. Returns the same
    /// shape as `respond_to_raise`.
    pub fn auto_respond_raise(&mut self) -> JsValue {
        match self.inner.auto_respond_raise() {
            Ok(crate::game::RaiseOutcome::Accepted {
                proposing_team,
                new_value,
            }) => {
                #[derive(Serialize)]
                struct Out {
                    accepted: bool,
                    proposing_team: usize,
                    new_value: usize,
                }
                swb::to_value(&Out {
                    accepted: true,
                    proposing_team,
                    new_value,
                })
                .unwrap_or(JsValue::NULL)
            }
            Ok(crate::game::RaiseOutcome::Folded {
                winning_team,
                points,
            }) => {
                #[derive(Serialize)]
                struct Out {
                    accepted: bool,
                    winning_team: usize,
                    points: usize,
                    ended: bool,
                }
                swb::to_value(&Out {
                    accepted: false,
                    winning_team,
                    points,
                    ended: true,
                })
                .unwrap_or(JsValue::NULL)
            }
            Err(_) => JsValue::NULL,
        }
    }

    /// Concede the current round on behalf of `team`. The round outcome is
    /// locked in but the cards still need to play out — call
    /// [`Self::auto_play_round`] right after to drive the remaining tricks.
    /// Returns true if the concede was accepted.
    pub fn concede_round(&mut self, team: usize) -> bool {
        self.inner.concede_round(team).is_ok()
    }

    /// Play out the rest of the round automatically (bot logic for every
    /// player including the human). Returns the same `JsRoundStep[]` shape
    /// as `play_round_logged` / `advance_bots`, plus a flag indicating
    /// whether the round actually ended.
    pub fn auto_play_round(&mut self) -> JsValue {
        let steps = self.inner.auto_play_round();
        let js_steps: Vec<JsRoundStep> = steps
            .into_iter()
            .map(|s| JsRoundStep {
                player: s.player,
                hand: s
                    .hand
                    .iter()
                    .map(|c| JsCard {
                        suit: c.suit,
                        rank: c.rank,
                    })
                    .collect(),
                allowed: s.allowed,
                played: JsCard {
                    suit: s.played.suit,
                    rank: s.played.rank,
                },
            })
            .collect();
        // round_ended is true iff playing_round flipped off (finish_round was
        // hit). The caller uses this to know when to deal a new round.
        let ended = !self.inner.playing_round;
        #[derive(serde::Serialize)]
        struct Out {
            ended: bool,
            steps: Vec<JsRoundStep>,
        }
        swb::to_value(&Out {
            ended,
            steps: js_steps,
        })
        .unwrap_or(JsValue::NULL)
    }

    /// True iff the round outcome has been locked in (concede/fold).
    pub fn round_decided(&self) -> Option<usize> {
        self.inner.round_decided()
    }

    pub fn round_points(&self) -> usize {
        self.inner.round_points
    }

    /// Target score to win the game.
    pub fn winning_points(&self) -> usize {
        crate::game::WINNING_POINTS
    }

    /// A team at or above this score may not propose raises any more.
    pub fn raise_lockout_score(&self) -> usize {
        crate::game::RAISE_LOCKOUT_SCORE
    }

    /// Index (0..4) of the dealer for the current round.
    pub fn dealer(&self) -> usize {
        self.inner.dealer
    }

    /// True iff `player_idx` is a "seeing" player this round
    /// (the dealer or the forehand). Seers see the cards that determine
    /// trump suit and striker rank — non-seers must deduce them from
    /// the play.
    pub fn is_seer(&self, player_idx: usize) -> bool {
        let dealer = self.inner.dealer;
        player_idx == dealer || player_idx == (dealer + 1) % 4
    }

    pub fn trump_suit(&self) -> Option<String> {
        self.inner.trump_suit().map(|s| s.to_string())
    }

    pub fn striker_rank(&self) -> Option<String> {
        self.inner.striker_rank().map(|r| r.to_string())
    }

    /// Returns the Rechte (the unique trump-suit + striker-rank card) as
    /// `{suit, rank}` using the same variant names as the cards in
    /// `hand(...)`. Returns null before a round has been started.
    pub fn rechte(&self) -> JsValue {
        match self.inner.rechte {
            Some(c) => swb::to_value(&JsCard {
                suit: c.suit,
                rank: c.rank,
            })
            .unwrap_or(JsValue::NULL),
            None => JsValue::NULL,
        }
    }

    pub fn scores(&self) -> Vec<usize> {
        self.inner.scores.to_vec()
    }

    /// Number of tricks each team has won SO FAR in the current round
    /// (resets at start_round_interactive). `[team1, team2]`.
    pub fn tricks_won(&self) -> Vec<usize> {
        self.inner.tricks_won_for_round().to_vec()
    }

    pub fn hand(&self, idx: usize) -> JsValue {
        let cards: Vec<JsCard> = self.inner.players[idx]
            .hand
            .iter()
            .map(|c| JsCard {
                suit: c.suit,
                rank: c.rank,
            })
            .collect();
        swb::to_value(&cards).unwrap()
    }

    pub fn play_round_logged(&mut self) -> JsValue {
        let (result, steps) = self.inner.play_round_logged();
        let js_steps: Vec<JsRoundStep> = steps
            .into_iter()
            .map(|s| JsRoundStep {
                player: s.player,
                hand: s
                    .hand
                    .iter()
                    .map(|c| JsCard {
                        suit: c.suit,
                        rank: c.rank,
                    })
                    .collect(),
                allowed: s.allowed,
                played: JsCard {
                    suit: s.played.suit,
                    rank: s.played.rank,
                },
            })
            .collect();
        swb::to_value(&(result as u8, js_steps)).unwrap()
    }

    pub fn advance_bots(&mut self) -> JsValue {
        let (res, steps) = self.inner.advance_bots();
        let js_steps: Vec<JsRoundStep> = steps
            .into_iter()
            .map(|s| JsRoundStep {
                player: s.player,
                hand: s
                    .hand
                    .iter()
                    .map(|c| JsCard {
                        suit: c.suit,
                        rank: c.rank,
                    })
                    .collect(),
                allowed: s.allowed,
                played: JsCard {
                    suit: s.played.suit,
                    rank: s.played.rank,
                },
            })
            .collect();
        swb::to_value(&(res.map(|r| r as u8), js_steps)).unwrap()
    }

    pub fn human_allowed_indices(&self) -> JsValue {
        swb::to_value(&self.inner.human_allowed_indices()).unwrap()
    }

    /// Returns `[{hand_idx, wins, total, illegal, rate}]` for the player
    /// whose turn it is. `total = wins + losses` (legal completions);
    /// `illegal` is non-zero only under the Database evaluator.
    pub fn human_move_evaluations(&self) -> JsValue {
        #[derive(Serialize)]
        struct JsEval {
            hand_idx: usize,
            wins: u32,
            total: u32,
            illegal: u32,
            rate: f64,
        }
        let evals: Vec<JsEval> = self
            .inner
            .human_move_evaluations()
            .into_iter()
            .map(|e| JsEval {
                hand_idx: e.hand_idx,
                wins: e.wins,
                total: e.total,
                illegal: e.illegal,
                rate: e.rate(),
            })
            .collect();
        swb::to_value(&evals).unwrap()
    }

    /// `"search"` (default, fast) or `"database"` (legacy brute-force 120^4).
    pub fn set_evaluator(&mut self, name: &str) {
        let kind = match name {
            "database" | "db" => Evaluator::Database,
            _ => Evaluator::Search,
        };
        self.inner.set_evaluator(kind);
    }

    /// Begin a chunked populate of the Database evaluator using the current
    /// round. Returns the total number of games that will be populated.
    /// Drive to completion with repeated calls to
    /// [`Self::database_populate_step`] interleaved with `setTimeout(0)`
    /// from JS so the UI can re-render in between batches.
    pub fn database_populate_begin(&mut self) -> usize {
        self.inner.begin_database_populate()
    }

    /// Process up to `batch` games. Returns `{ done, total, complete }`.
    pub fn database_populate_step(&mut self, batch: usize) -> JsValue {
        let (done, total) = self.inner.step_database_populate(batch);
        #[derive(Serialize)]
        struct Out {
            done: usize,
            total: usize,
            complete: bool,
        }
        swb::to_value(&Out {
            done,
            total,
            complete: done >= total && total > 0,
        })
        .unwrap_or(JsValue::NULL)
    }

    pub fn evaluator(&self) -> String {
        match self.inner.evaluator() {
            Evaluator::Search => "search".into(),
            Evaluator::Database => "database".into(),
        }
    }

    pub fn human_play(&mut self, idx: usize) -> JsValue {
        let (res, steps) = self.inner.human_play(idx);
        let js_steps: Vec<JsRoundStep> = steps
            .into_iter()
            .map(|s| JsRoundStep {
                player: s.player,
                hand: s
                    .hand
                    .iter()
                    .map(|c| JsCard {
                        suit: c.suit,
                        rank: c.rank,
                    })
                    .collect(),
                allowed: s.allowed,
                played: JsCard {
                    suit: s.played.suit,
                    rank: s.played.rank,
                },
            })
            .collect();
        swb::to_value(&(res.map(|r| r as u8), js_steps)).unwrap()
    }
}

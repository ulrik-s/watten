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

    /// Raise the current round's value by one point. team = 0 (Team 1) or
    /// 1 (Team 2). Returns true on success, false if the raise is illegal.
    pub fn raise_round(&mut self, team: usize) -> bool {
        self.inner.raise_round(team).is_ok()
    }

    /// Concede the current round on behalf of `team`. Returns true on success.
    pub fn concede_round(&mut self, team: usize) -> bool {
        self.inner.concede_round(team).is_ok()
    }

    pub fn round_points(&self) -> usize {
        self.inner.round_points
    }

    /// Target score to win the game.
    pub fn winning_points(&self) -> usize {
        crate::game::WINNING_POINTS
    }

    pub fn trump_suit(&self) -> Option<String> {
        self.inner.trump_suit().map(|s| s.to_string())
    }

    pub fn striker_rank(&self) -> Option<String> {
        self.inner.striker_rank().map(|r| r.to_string())
    }

    pub fn scores(&self) -> Vec<usize> {
        self.inner.scores.to_vec()
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

    /// Returns `[{hand_idx, wins, total, rate}]` for the player whose turn it
    /// is right now, restricted to legal moves.
    pub fn human_move_evaluations(&self) -> JsValue {
        #[derive(Serialize)]
        struct JsEval {
            hand_idx: usize,
            wins: u32,
            total: u32,
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

use serde::{Deserialize, Serialize};
use serde_wasm_bindgen as swb;
use wasm_bindgen::prelude::*;

use crate::game::{GameState, RoundStep};
use crate::{Card, GameResult, Rank, Suit};

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

    /// Clear any permutation range so that all permutations are considered
    /// again.
    pub fn clear_perm_range(&mut self) {
        self.inner.clear_perm_range();
    }

    pub fn play_round(&mut self) -> u8 {
        self.inner.play_round() as u8
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
                hand: s.hand.iter().map(|c| JsCard { suit: c.suit, rank: c.rank }).collect(),
                allowed: s.allowed,
                played: JsCard { suit: s.played.suit, rank: s.played.rank },
            })
            .collect();
        swb::to_value(&(res.map(|r| r as u8), js_steps)).unwrap()
    }

    pub fn human_allowed_indices(&self) -> JsValue {
        swb::to_value(&self.inner.human_allowed_indices()).unwrap()
    }

    pub fn human_play(&mut self, idx: usize) -> JsValue {
        let (res, steps) = self.inner.human_play(idx);
        let js_steps: Vec<JsRoundStep> = steps
            .into_iter()
            .map(|s| JsRoundStep {
                player: s.player,
                hand: s.hand.iter().map(|c| JsCard { suit: c.suit, rank: c.rank }).collect(),
                allowed: s.allowed,
                played: JsCard { suit: s.played.suit, rank: s.played.rank },
            })
            .collect();
        swb::to_value(&(res.map(|r| r as u8), js_steps)).unwrap()
    }
}

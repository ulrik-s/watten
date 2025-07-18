use serde::{Deserialize, Serialize};
use serde_wasm_bindgen as swb;
use wasm_bindgen::prelude::*;

use crate::game::GameState;
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
}

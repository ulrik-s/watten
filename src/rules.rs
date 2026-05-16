//! Pure scoring and trick-resolution rules for Watten.
//!
//! These functions are deliberately free of any game state: pass in cards
//! and the round's Rechte (trump suit + striker rank, encoded as a single
//! `Card`) and you get a comparable score back. `GameState`,
//! `SearchEvaluator`, `DatabaseEvaluator`, and the WASM bindings all funnel
//! their trick-winner decisions through `trick_winner_position` so the
//! ranking is consistent everywhere.
//!
//! ## Scoring model
//!
//!   card_score = round_score + trick_score
//!
//! ### round_score (fixed for the whole round)
//!   - trump suit:    +100
//!   - striker rank:  +200
//!   - Rechte (both): +300
//!
//! ### trick_score (depends on play order in the trick)
//!   - If a striker is played AFTER an earlier striker in the same trick,
//!     the later striker scores **-400** (the first striker dominates).
//!   - Otherwise, if the card matches the lead suit, score = rank + 20.
//!   - Otherwise, score = rank (cards still rank against each other within
//!     their own suit even when off-lead).
//!
//! Ties are broken by play order — the earliest-played card wins, which is
//! enforced by using strict `>` in `trick_winner_position`.

use crate::{Card, Rank};

/// Target score: the first team to reach this wins the game.
pub const WINNING_POINTS: usize = 13;

/// Once a team's score reaches this value, that team is no longer allowed
/// to *propose* a raise. (They can still answer raises from the other team
/// and play the round out.)
pub const RAISE_LOCKOUT_SCORE: usize = 10;

/// Base value of every round before any raises.
pub const ROUND_POINTS: usize = 2;

/// Number of tricks (= cards per hand) in one round.
pub const TRICKS_PER_ROUND: usize = 5;

/// Map a [`Rank`] to its numeric strength used inside [`trick_score`].
/// Higher = stronger; values are dense (1..=9) so they fit comfortably
/// inside the i16 we use for compound scores.
pub fn rank_value(r: Rank) -> u8 {
    match r {
        Rank::Seven => 1,
        Rank::Eight => 2,
        Rank::Nine => 3,
        Rank::Ten => 4,
        Rank::Unter => 5,
        Rank::Ober => 6,
        Rank::King => 7,
        Rank::Ace => 8,
        Rank::Weli => 9,
    }
}

/// Round-level score: +100 for trump suit, +200 for striker rank, +300 for
/// the Rechte itself. Independent of the trick.
pub fn round_score(card: &Card, rechte: Card) -> i16 {
    let mut s = 0;
    if card.suit == rechte.suit {
        s += 100;
    }
    if card.rank == rechte.rank {
        s += 200;
    }
    s
}

/// Trick-level score for a card at `position` (0..4) inside `trick`. See
/// the module docs for the model.
pub fn trick_score(card: &Card, position: usize, trick: &[Card], rechte: Card) -> i16 {
    if card.rank == rechte.rank {
        for earlier in 0..position {
            if trick[earlier].rank == rechte.rank {
                return -400;
            }
        }
    }
    let lead_suit = trick[0].suit;
    let rv = rank_value(card.rank) as i16;
    if card.suit == lead_suit {
        rv + 20
    } else {
        rv
    }
}

/// Total comparable score for a card in a trick — the sum of the round and
/// trick components.
pub fn card_score(card: &Card, position: usize, trick: &[Card], rechte: Card) -> i16 {
    round_score(card, rechte) + trick_score(card, position, trick, rechte)
}

/// Position in `trick` (0..len) of the card that wins. Ties resolve to the
/// earliest play because the comparison is strict `>`.
pub fn trick_winner_position(trick: &[Card], rechte: Card) -> usize {
    let mut best = 0;
    let mut best_score = card_score(&trick[0], 0, trick, rechte);
    for pos in 1..trick.len() {
        let s = card_score(&trick[pos], pos, trick, rechte);
        if s > best_score {
            best_score = s;
            best = pos;
        }
    }
    best
}

//! Move-evaluator interface and its two implementations.
//!
//! A [`MoveEvaluator`] takes the current round state and a list of legal
//! original-hand card indices, and returns wins/total counts per candidate
//! card. The two implementations agree on semantics — wins/total are counted
//! over legal completions of the round — so they can be swapped behind the
//! same `Box<dyn MoveEvaluator>`.
//!
//! - [`SearchEvaluator`]: memoized legal-completion enumeration. Fast,
//!   default.
//! - [`DatabaseEvaluator`]: pre-populates the full 120^4 result database (or
//!   a `perm_range`-restricted subset) and answers queries from it. Kept as
//!   a benchmarking / cross-check fallback.

use std::cell::RefCell;

use crate::database::{FlatGameDatabase, GameDatabase, InMemoryGameDatabase};
use crate::game::{play_hand, MoveEvaluation, TRICKS_PER_ROUND};
use crate::search::{
    evaluate_moves as search_evaluate_moves, SearchMemo, SearchPosition,
};
use crate::{all_hand_orders, perm_prefix_range, Card, GameResult};

/// All the per-round/per-state context an evaluator needs to score moves.
pub struct EvaluationContext<'a> {
    pub orig_hands: &'a [[Card; TRICKS_PER_ROUND]; 4],
    pub played: &'a [Vec<usize>; 4],
    pub dealer: usize,
    pub rechte: Card,
    pub player: usize,
    pub current_hand: &'a [Card],
    /// Original-hand indices that are legal for `player` to play next. Caller
    /// guarantees these all reference cards still in `current_hand`.
    pub allowed_orig_indices: &'a [usize],
    pub current_trick: &'a [(usize, Card)],
    pub tricks_won: [usize; 2],
}

pub trait MoveEvaluator {
    /// Called once at the start of every round, before any `evaluate_moves`
    /// call. Lets implementations pre-compute or reset per-round state.
    fn prepare_round(
        &mut self,
        orig_hands: &[[Card; TRICKS_PER_ROUND]; 4],
        dealer: usize,
        rechte: Card,
    );

    /// Evaluate every legal move available to `ctx.player`. Returned vector is
    /// in `ctx.allowed_orig_indices` order (one entry per supplied index).
    /// `hand_idx` is the index into `ctx.current_hand`.
    fn evaluate_moves(&self, ctx: &EvaluationContext<'_>) -> Vec<MoveEvaluation>;

    /// String tag for diagnostics / wasm.
    fn name(&self) -> &'static str;

    /// Begin a chunked preparation pass for evaluators with expensive
    /// per-round work (today: the 120^4 database). Returns the total number
    /// of "units" the caller will need to step through via
    /// [`Self::step_chunked_populate`]. Evaluators with cheap prep return
    /// 0 — the caller is then free to skip the chunked loop.
    fn begin_chunked_populate(
        &mut self,
        _orig_hands: &[[Card; TRICKS_PER_ROUND]; 4],
        _dealer: usize,
        _rechte: Card,
    ) -> usize {
        0
    }

    /// Process up to `batch` units of the chunked populate. Returns
    /// `(done_so_far, total)`. When `done_so_far >= total` the populate
    /// is finished and the evaluator is ready to answer queries.
    fn step_chunked_populate(&mut self, _batch: usize) -> (usize, usize) {
        (0, 0)
    }
}

// ---------------------------------------------------------------------------
// Helpers

fn remaining_mask_from_played(played: &[Vec<usize>; 4]) -> [u8; 4] {
    let mut mask = [0u8; 4];
    for p in 0..4 {
        let mut m = (1u8 << TRICKS_PER_ROUND) - 1;
        for &orig in &played[p] {
            m &= !(1u8 << orig);
        }
        mask[p] = m;
    }
    mask
}

fn current_hand_idx_for_orig(
    orig_hands: &[[Card; TRICKS_PER_ROUND]; 4],
    p_idx: usize,
    orig_idx: usize,
    current_hand: &[Card],
) -> Option<usize> {
    let card = orig_hands[p_idx][orig_idx];
    current_hand.iter().position(|c| *c == card)
}

// ---------------------------------------------------------------------------
// Search evaluator

pub struct SearchEvaluator {
    memo: RefCell<SearchMemo>,
}

impl SearchEvaluator {
    pub fn new() -> Self {
        Self {
            memo: RefCell::new(SearchMemo::new()),
        }
    }
}

impl Default for SearchEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveEvaluator for SearchEvaluator {
    fn prepare_round(
        &mut self,
        _orig_hands: &[[Card; TRICKS_PER_ROUND]; 4],
        _dealer: usize,
        _rechte: Card,
    ) {
        self.memo.borrow_mut().clear();
    }

    fn evaluate_moves(&self, ctx: &EvaluationContext<'_>) -> Vec<MoveEvaluation> {
        let remaining = remaining_mask_from_played(ctx.played);
        let lead = if ctx.current_trick.is_empty() {
            ctx.player as u8
        } else {
            ctx.current_trick[0].0 as u8
        };
        let pos = SearchPosition {
            orig_hands: ctx.orig_hands,
            remaining,
            lead,
            dealer: ctx.dealer as u8,
            rechte: ctx.rechte,
        };
        let trick: Vec<(u8, Card)> = ctx
            .current_trick
            .iter()
            .map(|(p, c)| (*p as u8, *c))
            .collect();
        let mut memo = self.memo.borrow_mut();
        let evals = search_evaluate_moves(
            &pos,
            ctx.player as u8,
            &trick,
            [ctx.tricks_won[0] as u8, ctx.tricks_won[1] as u8],
            ctx.allowed_orig_indices,
            &mut memo,
        );
        evals
            .into_iter()
            .filter_map(|e| {
                current_hand_idx_for_orig(
                    ctx.orig_hands,
                    ctx.player,
                    e.orig_idx,
                    ctx.current_hand,
                )
                .map(|hi| MoveEvaluation {
                    hand_idx: hi,
                    wins: e.wins,
                    total: e.total,
                    illegal: 0, // search only explores legal continuations
                })
            })
            .collect()
    }

    fn name(&self) -> &'static str {
        "search"
    }
}

// ---------------------------------------------------------------------------
// Database evaluator

pub struct DatabaseEvaluator {
    pub db: Box<dyn GameDatabase>,
    pub perm_range: Option<Vec<usize>>,
    pub workers: usize,
    /// In-progress chunked populate state. `Some` while
    /// `begin_chunked_populate` has been called and not yet driven to
    /// completion.
    populate: Option<PopulateState>,
}

struct PopulateState {
    orig_hands: [[Card; TRICKS_PER_ROUND]; 4],
    dealer: usize,
    rechte: Card,
    perms: Vec<[usize; TRICKS_PER_ROUND]>,
    indices: Vec<usize>,
    progress: usize,
    total: usize,
}

impl DatabaseEvaluator {
    pub fn new() -> Self {
        Self {
            db: Box::new(FlatGameDatabase::new()),
            perm_range: None,
            workers: num_cpus::get().max(1) * 2,
            populate: None,
        }
    }

    pub fn with_perm_range(mut self, range: Vec<usize>) -> Self {
        self.perm_range = Some(range);
        self
    }

    pub fn with_workers(mut self, workers: usize) -> Self {
        self.workers = workers.max(1);
        self
    }
}

impl Default for DatabaseEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveEvaluator for DatabaseEvaluator {
    fn prepare_round(
        &mut self,
        orig_hands: &[[Card; TRICKS_PER_ROUND]; 4],
        dealer: usize,
        rechte: Card,
    ) {
        self.db = Box::new(FlatGameDatabase::new());
        let perms = all_hand_orders();
        let indices: Vec<usize> = self
            .perm_range
            .clone()
            .unwrap_or_else(|| (0..perms.len()).collect());

        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::sync::mpsc::channel;
            use std::thread;

            let workers = self.workers.max(1);
            let total = indices.len().pow(4) as u64;
            let (tx, rx) = channel();
            for worker_id in 0..workers {
                let tx = tx.clone();
                let indices = indices.clone();
                let hands = *orig_hands;
                let dealer = dealer;
                let rechte = rechte;
                let perms = perms.clone();
                thread::spawn(move || {
                    let len = indices.len();
                    let total = len.pow(4);
                    for n in (worker_id..total).step_by(workers) {
                        let i1_idx = n / (len * len * len);
                        let i2_idx = (n / (len * len)) % len;
                        let i3_idx = (n / len) % len;
                        let i4_idx = n % len;
                        let i1 = indices[i1_idx];
                        let i2 = indices[i2_idx];
                        let i3 = indices[i3_idx];
                        let i4 = indices[i4_idx];
                        let result = play_hand(&hands, [i1, i2, i3, i4], dealer, rechte, &perms);
                        let _ = tx.send((i1, i2, i3, i4, result));
                    }
                });
            }
            drop(tx);
            let mut done = 0u64;
            for (i1, i2, i3, i4, res) in rx {
                self.db.set(i1, i2, i3, i4, res);
                done += 1;
                if done == total {
                    break;
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            for &i1 in &indices {
                for &i2 in &indices {
                    for &i3 in &indices {
                        for &i4 in &indices {
                            let result =
                                play_hand(orig_hands, [i1, i2, i3, i4], dealer, rechte, &perms);
                            self.db.set(i1, i2, i3, i4, result);
                        }
                    }
                }
            }
        }
    }

    fn evaluate_moves(&self, ctx: &EvaluationContext<'_>) -> Vec<MoveEvaluation> {
        let team = ctx.player % 2;
        let win_result = if team == 0 {
            GameResult::Team1Win as usize
        } else {
            GameResult::Team2Win as usize
        };
        let loss_result = if team == 0 {
            GameResult::Team2Win as usize
        } else {
            GameResult::Team1Win as usize
        };
        let mut out = Vec::with_capacity(ctx.allowed_orig_indices.len());
        for &orig in ctx.allowed_orig_indices {
            let mut lists: [Vec<usize>; 4] = std::array::from_fn(|_| Vec::new());
            for i in 0..4 {
                let mut prefix = ctx.played[i].clone();
                if i == ctx.player {
                    prefix.push(orig);
                }
                let (s, e) = perm_prefix_range(&prefix);
                if let Some(ref allowed) = self.perm_range {
                    lists[i] = allowed
                        .iter()
                        .cloned()
                        .filter(|&v| v >= s && v < e)
                        .collect();
                } else {
                    lists[i] = (s..e).collect();
                }
            }
            let counts = self
                .db
                .counts_in_lists(&lists[0], &lists[1], &lists[2], &lists[3]);
            let wins = counts[win_result];
            let losses = counts[loss_result];
            let illegal = counts[GameResult::RuleViolation as usize];
            let hi = match current_hand_idx_for_orig(
                ctx.orig_hands,
                ctx.player,
                orig,
                ctx.current_hand,
            ) {
                Some(i) => i,
                None => continue,
            };
            out.push(MoveEvaluation {
                hand_idx: hi,
                wins,
                total: wins + losses,
                illegal,
            });
        }
        out
    }

    fn name(&self) -> &'static str {
        "database"
    }

    fn begin_chunked_populate(
        &mut self,
        orig_hands: &[[Card; TRICKS_PER_ROUND]; 4],
        dealer: usize,
        rechte: Card,
    ) -> usize {
        self.db = Box::new(FlatGameDatabase::new());
        let perms = all_hand_orders();
        let indices: Vec<usize> = self
            .perm_range
            .clone()
            .unwrap_or_else(|| (0..perms.len()).collect());
        let len = indices.len();
        let total = len * len * len * len;
        self.populate = Some(PopulateState {
            orig_hands: *orig_hands,
            dealer,
            rechte,
            perms,
            indices,
            progress: 0,
            total,
        });
        total
    }

    fn step_chunked_populate(&mut self, batch: usize) -> (usize, usize) {
        // Pull the state out so we can mutate `self.db` alongside it.
        let mut state = match self.populate.take() {
            Some(s) => s,
            None => return (0, 0),
        };
        let end = state.total.min(state.progress.saturating_add(batch));
        let len = state.indices.len();
        for n in state.progress..end {
            // n = ((i1 * len + i2) * len + i3) * len + i4   (base-len digits)
            let i1_idx = n / (len * len * len);
            let i2_idx = (n / (len * len)) % len;
            let i3_idx = (n / len) % len;
            let i4_idx = n % len;
            let i1 = state.indices[i1_idx];
            let i2 = state.indices[i2_idx];
            let i3 = state.indices[i3_idx];
            let i4 = state.indices[i4_idx];
            let result = play_hand(
                &state.orig_hands,
                [i1, i2, i3, i4],
                state.dealer,
                state.rechte,
                &state.perms,
            );
            self.db.set(i1, i2, i3, i4, result);
        }
        state.progress = end;
        let progress = state.progress;
        let total = state.total;
        if progress < total {
            self.populate = Some(state);
        }
        (progress, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rank, Suit};

    fn sample_hands() -> [[Card; TRICKS_PER_ROUND]; 4] {
        use Rank::*;
        use Suit::*;
        [
            [
                Card::new(Hearts, Unter),
                Card::new(Bells, Ace),
                Card::new(Leaves, King),
                Card::new(Hearts, Ace),
                Card::new(Acorns, Ten),
            ],
            [
                Card::new(Hearts, Ten),
                Card::new(Bells, King),
                Card::new(Leaves, Ace),
                Card::new(Bells, Seven),
                Card::new(Acorns, Nine),
            ],
            [
                Card::new(Hearts, King),
                Card::new(Leaves, Ober),
                Card::new(Bells, Nine),
                Card::new(Hearts, Nine),
                Card::new(Acorns, Unter),
            ],
            [
                Card::new(Hearts, Ober),
                Card::new(Bells, Unter),
                Card::new(Leaves, Nine),
                Card::new(Acorns, Ace),
                Card::new(Bells, Ten),
            ],
        ]
    }

    fn make_ctx<'a>(
        hands: &'a [[Card; TRICKS_PER_ROUND]; 4],
        played: &'a [Vec<usize>; 4],
        allowed: &'a [usize],
        current_hand: &'a [Card],
        rechte: Card,
    ) -> EvaluationContext<'a> {
        EvaluationContext {
            orig_hands: hands,
            played,
            dealer: 0,
            rechte,
            player: 1,
            current_hand,
            allowed_orig_indices: allowed,
            current_trick: &[],
            tricks_won: [0, 0],
        }
    }

    #[test]
    fn search_evaluator_returns_one_eval_per_legal_card() {
        let hands = sample_hands();
        let played: [Vec<usize>; 4] = Default::default();
        let allowed = [0usize, 1, 2, 3, 4];
        let current_hand = hands[1].to_vec();
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let mut ev = SearchEvaluator::new();
        ev.prepare_round(&hands, 0, rechte);
        let ctx = make_ctx(&hands, &played, &allowed, &current_hand, rechte);
        let evs = ev.evaluate_moves(&ctx);
        assert_eq!(evs.len(), 5);
        assert!(evs.iter().any(|e| e.total > 0));
    }

    /// Sanity check: both evaluators share the same `MoveEvaluator` interface
    /// and return one [`MoveEvaluation`] per supplied legal index. Their
    /// counts differ in general (search counts legal orderings only; the DB
    /// counts the full 120^4 even when illegal), but their *interface* must
    /// be interchangeable.
    #[test]
    fn search_and_database_share_interface() {
        let hands = sample_hands();
        let played: [Vec<usize>; 4] = Default::default();
        let allowed = [0usize, 1, 2, 3, 4];
        let current_hand = hands[1].to_vec();
        let rechte = Card::new(Suit::Hearts, Rank::Unter);

        let mut search: Box<dyn MoveEvaluator> = Box::new(SearchEvaluator::new());
        let mut db: Box<dyn MoveEvaluator> =
            Box::new(DatabaseEvaluator::new().with_perm_range(vec![0, 1]));
        for ev in [&mut search, &mut db] {
            ev.prepare_round(&hands, 0, rechte);
            let ctx = make_ctx(&hands, &played, &allowed, &current_hand, rechte);
            let r = ev.evaluate_moves(&ctx);
            assert_eq!(r.len(), 5, "{}: wrong length", ev.name());
            assert!(r.iter().any(|m| m.total > 0), "{}: zero counts", ev.name());
        }
    }
}

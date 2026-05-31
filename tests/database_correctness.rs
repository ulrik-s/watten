//! Integration tests pinning down exactly how the 120⁴ database
//! evaluator is supposed to behave: every entry in the array is one
//! played game, indexed by `(p1_perm, p2_perm, p3_perm, p4_perm)` where
//! each `pX_perm` is an index into `all_hand_orders()`. When a player
//! plays a card we narrow the candidate completions to those that
//! *start with* the cards already on the table and tally win / loss /
//! illegal results out of that subset.
//!
//! Two invariants the rest of the engine depends on:
//!
//!  1. `perm_prefix_range(prefix)` returns the range of `all_hand_orders()`
//!     indices whose permutations start with `prefix` — so the evaluator
//!     can `(s..e).collect()` and query the database directly.
//!  2. The database evaluator's win/loss/illegal counts for a candidate
//!     card match what you get by hand-rolling the loop the user
//!     described: iterate every entry consistent with the played
//!     prefix and bucket the result column.
//!
//! Both invariants are exercised below.

use watten::evaluator::{DatabaseEvaluator, EvaluationContext, MoveEvaluator};
use watten::game::TRICKS_PER_ROUND;
use watten::{all_hand_orders, perm_prefix_range, Card, GameResult, Rank, Suit};

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

/// **Invariant 1**: every index in `perm_prefix_range([k])` must address
/// a permutation in `all_hand_orders()` whose first element is `k`, and
/// every such permutation must appear in that range. If this fails, the
/// database evaluator is querying arbitrary slots when the user thinks
/// it's filtering by "player k's first card is at orig position p".
#[test]
fn perm_prefix_range_indexes_into_all_hand_orders_correctly() {
    let perms = all_hand_orders();
    for first in 0..5 {
        let (s, e) = perm_prefix_range(&[first]);
        // Length of the range is always 4! = 24.
        assert_eq!(e - s, 24, "range size for prefix [{first}]");
        // Every index inside the range must point to a permutation
        // whose first element is `first`.
        for (offset, perm) in perms[s..e].iter().enumerate() {
            let i = s + offset;
            assert_eq!(
                perm[0], first,
                "perms[{i}] = {perm:?} should start with {first}; \
                 perm_prefix_range([{first}]) returned [{s}, {e})"
            );
        }
        // Conversely, every permutation that DOES start with `first`
        // must be inside the range.
        for (i, p) in perms.iter().enumerate() {
            if p[0] == first {
                assert!(
                    i >= s && i < e,
                    "perms[{i}] = {p:?} starts with {first} but lies outside \
                     perm_prefix_range([{first}]) = [{s}, {e})"
                );
            }
        }
    }
}

/// **Invariant 1b**: two-element prefixes work the same way.
#[test]
fn perm_prefix_range_two_element_prefix_indexes_correctly() {
    let perms = all_hand_orders();
    // Try a few representative prefixes covering different shapes.
    for &(a, b) in &[(0usize, 1usize), (2, 0), (4, 3), (1, 4)] {
        let (s, e) = perm_prefix_range(&[a, b]);
        assert_eq!(e - s, 6, "range size for prefix [{a}, {b}]");
        for perm in &perms[s..e] {
            assert_eq!(perm[0], a);
            assert_eq!(perm[1], b);
        }
        for (i, p) in perms.iter().enumerate() {
            if p[0] == a && p[1] == b {
                assert!(
                    i >= s && i < e,
                    "perms[{i}] = {p:?} starts with [{a}, {b}] but is outside \
                     perm_prefix_range result"
                );
            }
        }
    }
}

/// **Invariant 2** — the user's mental model: the database evaluator's
/// `(wins, total, illegal)` for a candidate card equals what you get by
/// iterating every entry in the database whose first-card prefix
/// matches the played state and bucketing the stored result.
///
/// We run this against a tiny `perm_range` so the populate finishes in
/// well under a second, and we exhaustively verify the counts for every
/// legal candidate of player 0 (leading the first trick).
#[test]
fn database_evaluator_counts_match_a_brute_force_count() {
    let hands = sample_hands();
    let dealer = 0;
    // Rechte for this deal: trump suit comes from dealer's first card
    // (Hearts), striker rank from the next player's first card (Ten).
    let rechte = Card::new(Suit::Hearts, Rank::Ten);
    // Restrict the populate to a small subset of permutations so each
    // player has at most a handful of orderings — keeps the test fast
    // while still exercising the full machinery.
    let restricted_perms = vec![0usize, 7, 19, 73];
    let perms = all_hand_orders();

    let mut ev = DatabaseEvaluator::new()
        .with_perm_range(restricted_perms.clone())
        .with_workers(1);
    ev.prepare_round(&hands, dealer, rechte);

    // Pretend nothing's been played yet; player 0 is leading the first
    // trick. `current_hand` matches `orig_hands[0]` 1:1.
    let played: [Vec<usize>; 4] = std::array::from_fn(|_| Vec::new());
    let current_hand: Vec<Card> = hands[0].to_vec();
    let allowed_orig_indices: Vec<usize> = (0..5).collect();
    let ctx = EvaluationContext {
        orig_hands: &hands,
        played: &played,
        dealer,
        rechte,
        player: 0,
        current_hand: &current_hand,
        allowed_orig_indices: &allowed_orig_indices,
        current_trick: &[],
        tricks_won: [0, 0],
    };
    let evals = ev.evaluate_moves(&ctx);
    assert_eq!(evals.len(), 5, "one eval per candidate card");

    // For every candidate card, compute the user-described brute force:
    // loop over every (i1, i2, i3, i4) in the populate's perm_range
    // such that p0's permutation has the candidate at its head; play
    // out that game; bucket the result.
    for &orig in &allowed_orig_indices {
        let mut wins = 0u32;
        let mut losses = 0u32;
        let mut illegal = 0u32;
        for &i1 in &restricted_perms {
            // Only count games where p0's permutation starts with the
            // candidate card — that's the meaning of "if p0 plays
            // card `orig` now".
            if perms[i1][0] != orig {
                continue;
            }
            for &i2 in &restricted_perms {
                for &i3 in &restricted_perms {
                    for &i4 in &restricted_perms {
                        let r = watten::game::play_hand(
                            &hands,
                            [i1, i2, i3, i4],
                            dealer,
                            rechte,
                            &perms,
                        );
                        match r {
                            GameResult::Team1Win => wins += 1,
                            GameResult::Team2Win => losses += 1,
                            GameResult::RuleViolation => illegal += 1,
                            GameResult::NotPlayed => {}
                        }
                    }
                }
            }
        }
        // hand_idx for player 0 with empty played == orig_idx (their
        // current hand still equals the deal).
        let e = evals
            .iter()
            .find(|e| e.hand_idx == orig)
            .unwrap_or_else(|| panic!("no eval for orig idx {orig}"));
        assert_eq!(
            e.wins, wins,
            "wins mismatch for candidate orig={}: evaluator={}, brute-force={}",
            orig, e.wins, wins
        );
        assert_eq!(
            e.total,
            wins + losses,
            "total (wins+losses) mismatch for candidate orig={orig}"
        );
        assert_eq!(
            e.illegal, illegal,
            "illegal mismatch for candidate orig={orig}"
        );
    }
}

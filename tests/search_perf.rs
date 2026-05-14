use watten::search::{count_completions, evaluate_moves, SearchMemo, SearchPosition};
use watten::{Card, Rank, Suit};

fn sample_hands() -> [[Card; 5]; 4] {
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

#[test]
fn search_full_round_perf() {
    let hands = sample_hands();
    let rechte = Card::new(Suit::Hearts, Rank::Unter);
    let pos = SearchPosition {
        orig_hands: &hands,
        remaining: [0b11111; 4],
        lead: 1,
        dealer: 0,
        rechte,
    };
    let mut memo = SearchMemo::new();
    let start = std::time::Instant::now();
    let counts = count_completions(&pos, &mut memo);
    let elapsed = start.elapsed();
    let total: u64 = counts.iter().map(|&c| c as u64).sum();
    println!(
        "search full round elapsed={:?} total_orderings={} memo_entries={}",
        elapsed,
        total,
        // SearchMemo's `table` is private; we only have `clear`. Use the public
        // observation that we just ran a fresh search.
        0
    );
    assert!(total > 0);
}

#[test]
fn evaluate_moves_perf_from_start() {
    let hands = sample_hands();
    let rechte = Card::new(Suit::Hearts, Rank::Unter);
    let pos = SearchPosition {
        orig_hands: &hands,
        remaining: [0b11111; 4],
        lead: 1,
        dealer: 0,
        rechte,
    };
    let mut memo = SearchMemo::new();
    let start = std::time::Instant::now();
    let moves = evaluate_moves(&pos, 1, &[], [0, 0], &[0, 1, 2, 3, 4], &mut memo);
    let elapsed = start.elapsed();
    println!("evaluate_moves elapsed={:?}", elapsed);
    for m in &moves {
        println!(
            "  card[{}] wins={} total={} rate={:.3}",
            m.orig_idx,
            m.wins,
            m.total,
            m.rate()
        );
    }
    assert_eq!(moves.len(), 5);
}

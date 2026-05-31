use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Suit {
    Hearts,
    Bells,
    Leaves,
    Acorns,
}

impl std::fmt::Display for Suit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Suit::Hearts => "Hearts",
            Suit::Bells => "Bells",
            Suit::Leaves => "Leaves",
            Suit::Acorns => "Acorns",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rank {
    Seven,
    Eight,
    Nine,
    Ten,
    Unter,
    Ober,
    King,
    Ace,
    /// Special card, 6 of Bells
    Weli,
}

impl std::fmt::Display for Rank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "10",
            Rank::Unter => "Unter",
            Rank::Ober => "Ober",
            Rank::King => "King",
            Rank::Ace => "Ace",
            Rank::Weli => "Weli",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

impl Card {
    pub fn new(suit: Suit, rank: Rank) -> Self {
        Self { suit, rank }
    }
}

impl std::fmt::Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} of {}", self.rank, self.suit)
    }
}

/// Return a deck containing all 33 Watten cards
pub fn deck() -> Vec<Card> {
    use Rank::*;
    use Suit::*;
    let mut cards = Vec::new();
    let ranks = [Seven, Eight, Nine, Ten, Unter, Ober, King, Ace];
    for &suit in &[Hearts, Bells, Leaves, Acorns] {
        for &rank in &ranks {
            cards.push(Card::new(suit, rank));
        }
    }
    // Add Weli (6 of Bells)
    cards.push(Card::new(Suit::Bells, Weli));
    cards
}

/// Shuffle a deck of cards in place
pub fn shuffle(deck: &mut [Card]) {
    use rand::seq::SliceRandom;
    let mut rng = rand::thread_rng();
    deck.shuffle(&mut rng);
}

/// Number of ways a hand of five cards can be ordered
pub const HAND_PERMUTATIONS: usize = 120; // 5!

/// Compute the lexicographic index range of all permutations starting with the
/// given prefix. The returned range is `[start, end)`.
pub fn perm_prefix_range(prefix: &[usize]) -> (usize, usize) {
    assert!(prefix.len() <= 5, "prefix too long");
    let mut index = 0;
    let mut used = [false; 5];
    for (i, &v) in prefix.iter().enumerate() {
        let smaller = used[..v].iter().filter(|&&u| !u).count();
        used[v] = true;
        index += smaller * factorial(4 - i);
    }
    let remaining = 5 - prefix.len();
    let len = factorial(remaining);
    (index, index + len)
}

/// Generate all permutations of indices `[0,1,2,3,4]` **in
/// lexicographic order**.
///
/// CRITICAL invariant for the 120⁴ database: `all_hand_orders()[i]` is
/// the permutation whose [`perm_index`] is `i`, and consequently
/// `perm_prefix_range(prefix)` returns `[s, e)` such that exactly
/// `all_hand_orders()[s..e]` are the permutations starting with
/// `prefix`. The database evaluator relies on this to map "the cards
/// already played by player p" into a contiguous slice of permutation
/// indices that select rows in the populated array.
///
/// Heap's algorithm — which the previous implementation used — scatters
/// permutations starting with `k` across the 120 outputs, which makes
/// `perm_prefix_range`'s contiguous range refer to the wrong subset of
/// database rows and corrupts every win/loss tally.
pub fn all_hand_orders() -> Vec<[usize; 5]> {
    let mut result = Vec::with_capacity(HAND_PERMUTATIONS);
    let mut arr: [usize; 5] = [0, 1, 2, 3, 4];
    result.push(arr);
    while next_lex_permutation(&mut arr) {
        result.push(arr);
    }
    debug_assert_eq!(result.len(), HAND_PERMUTATIONS);
    result
}

/// Mutate `arr` into the lexicographically-next permutation of its
/// elements. Returns `false` (and leaves `arr` reversed) once the
/// strictly-descending permutation is hit. Standard algorithm — same
/// idea as `std::next_permutation` in C++.
fn next_lex_permutation(arr: &mut [usize; 5]) -> bool {
    // Find largest `i` such that arr[i-1] < arr[i].
    let n = arr.len();
    let mut i = n - 1;
    while i > 0 && arr[i - 1] >= arr[i] {
        i -= 1;
    }
    if i == 0 {
        return false;
    }
    // Find largest `j` such that arr[j] > arr[i-1].
    let mut j = n - 1;
    while arr[j] <= arr[i - 1] {
        j -= 1;
    }
    arr.swap(i - 1, j);
    arr[i..].reverse();
    true
}

/// Compute the lexicographic index of a permutation of `[0,1,2,3,4]`
pub fn perm_index(perm: &[usize; 5]) -> usize {
    let mut index = 0;
    let mut used = [false; 5];
    for (i, &p) in perm.iter().enumerate() {
        let smaller = used[..p].iter().filter(|&&u| !u).count();
        used[p] = true;
        index += smaller * factorial(4 - i);
    }
    index
}

const fn factorial(n: usize) -> usize {
    match n {
        0 | 1 => 1,
        _ => n * factorial(n - 1),
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    NotPlayed = 0,
    Team1Win = 1,
    Team2Win = 2,
    RuleViolation = 3,
}

pub mod database;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deck_has_33_cards() {
        let d = deck();
        assert_eq!(d.len(), 33);
    }

    #[test]
    fn hand_order_permutations() {
        let perms = all_hand_orders();
        assert_eq!(perms.len(), HAND_PERMUTATIONS);
        // first permutation should be identity
        assert_eq!(perms[0], [0, 1, 2, 3, 4]);
        // ensure indexes are unique
        let mut seen = std::collections::HashSet::new();
        for p in &perms {
            let idx = perm_index(p);
            assert!(seen.insert(idx), "duplicate index {}", idx);
        }
    }

    #[test]
    fn prefix_ranges() {
        let (s, e) = perm_prefix_range(&[0]);
        assert_eq!(e - s, factorial(4));
        assert_eq!(s, perm_index(&[0, 1, 2, 3, 4]));

        let (s2, e2) = perm_prefix_range(&[1, 0]);
        assert_eq!(e2 - s2, factorial(3));
        assert_eq!(s2, perm_index(&[1, 0, 2, 3, 4]));
    }

    #[test]
    fn db_counts_over_range() {
        use crate::database::{GameDatabase, InMemoryGameDatabase};
        let mut db = InMemoryGameDatabase::new();
        db.set(0, 0, 0, 0, GameResult::Team1Win);
        let counts = db.counts_in_ranges(0..1, 0..1, 0..1, 0..1);
        assert_eq!(counts[GameResult::Team1Win as usize], 1);
        assert_eq!(counts[GameResult::NotPlayed as usize], 0);
    }
}

pub mod evaluator;
pub mod game;
pub mod player;
pub mod rules;
pub mod search;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

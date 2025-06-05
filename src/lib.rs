#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    for i in 0..prefix.len() {
        let v = prefix[i];
        let mut smaller = 0;
        for j in 0..v {
            if !used[j] {
                smaller += 1;
            }
        }
        used[v] = true;
        index += smaller * factorial(4 - i);
    }
    let remaining = 5 - prefix.len();
    let len = factorial(remaining);
    (index, index + len)
}

/// Generate all permutations of indices `[0,1,2,3,4]`
pub fn all_hand_orders() -> Vec<[usize; 5]> {
    fn permute(result: &mut Vec<[usize; 5]>, arr: &mut [usize], n: usize) {
        if n == 1 {
            result.push([arr[0], arr[1], arr[2], arr[3], arr[4]]);
        } else {
            for i in 0..n {
                permute(result, arr, n - 1);
                if n % 2 == 0 {
                    arr.swap(i, n - 1);
                } else {
                    arr.swap(0, n - 1);
                }
            }
        }
    }
    let mut data = [0, 1, 2, 3, 4];
    let mut result = Vec::with_capacity(HAND_PERMUTATIONS);
    permute(&mut result, &mut data, 5);
    result
}

/// Compute the lexicographic index of a permutation of `[0,1,2,3,4]`
pub fn perm_index(perm: &[usize; 5]) -> usize {
    let mut index = 0;
    let mut used = [false; 5];
    for i in 0..5 {
        let mut smaller = 0;
        for j in 0..perm[i] {
            if !used[j] {
                smaller += 1;
            }
        }
        used[perm[i]] = true;
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

use std::collections::HashMap;

/// Database holding evaluated games keyed by permutation index
pub struct GameDatabase {
    results: HashMap<u32, GameResult>,
}

impl GameDatabase {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
        }
    }

    pub fn make_index(p1: usize, p2: usize, p3: usize, p4: usize) -> u32 {
        (((p1 * HAND_PERMUTATIONS + p2) * HAND_PERMUTATIONS + p3) * HAND_PERMUTATIONS + p4) as u32
    }

    pub fn set(&mut self, p1: usize, p2: usize, p3: usize, p4: usize, result: GameResult) {
        let idx = Self::make_index(p1, p2, p3, p4);
        self.results.insert(idx, result);
    }

    pub fn get(&self, p1: usize, p2: usize, p3: usize, p4: usize) -> GameResult {
        let idx = Self::make_index(p1, p2, p3, p4);
        *self.results.get(&idx).unwrap_or(&GameResult::NotPlayed)
    }

    /// Count game results over all index combinations within the provided ranges.
    pub fn counts_in_ranges(
        &self,
        p1: std::ops::Range<usize>,
        p2: std::ops::Range<usize>,
        p3: std::ops::Range<usize>,
        p4: std::ops::Range<usize>,
    ) -> [u32; 4] {
        let mut counts = [0u32; 4];
        for i1 in p1.clone() {
            for i2 in p2.clone() {
                for i3 in p3.clone() {
                    for i4 in p4.clone() {
                        let r = self.get(i1, i2, i3, i4) as usize;
                        counts[r] += 1;
                    }
                }
            }
        }
        counts
    }
}

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
        let mut db = GameDatabase::new();
        db.set(0, 0, 0, 0, GameResult::Team1Win);
        let counts = db.counts_in_ranges(0..1, 0..1, 0..1, 0..1);
        assert_eq!(counts[GameResult::Team1Win as usize], 1);
        assert_eq!(counts[GameResult::NotPlayed as usize], 0);
    }
}

pub mod player;
pub mod game;

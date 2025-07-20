use std::collections::HashMap;
use std::ops::Range;

use crate::{GameResult, HAND_PERMUTATIONS};

/// API for accessing the game database.
pub trait GameDatabase {
    fn set(&mut self, p1: usize, p2: usize, p3: usize, p4: usize, result: GameResult);
    fn get(&self, p1: usize, p2: usize, p3: usize, p4: usize) -> GameResult;
    fn counts_in_ranges(
        &self,
        p1: Range<usize>,
        p2: Range<usize>,
        p3: Range<usize>,
        p4: Range<usize>,
    ) -> [u32; 4];

    /// Count results over explicit lists of permutation indices. This allows
    /// callers to restrict iteration to a subset of permutations rather than a
    /// contiguous range.
    fn counts_in_lists(
        &self,
        p1: &[usize],
        p2: &[usize],
        p3: &[usize],
        p4: &[usize],
    ) -> [u32; 4];
}

/// In-memory implementation of [`GameDatabase`].
pub struct InMemoryGameDatabase {
    results: HashMap<u32, GameResult>,
}

impl InMemoryGameDatabase {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
        }
    }

    fn make_index(p1: usize, p2: usize, p3: usize, p4: usize) -> u32 {
        (((p1 * HAND_PERMUTATIONS + p2) * HAND_PERMUTATIONS + p3) * HAND_PERMUTATIONS + p4) as u32
    }
}

impl GameDatabase for InMemoryGameDatabase {
    fn set(&mut self, p1: usize, p2: usize, p3: usize, p4: usize, result: GameResult) {
        let idx = Self::make_index(p1, p2, p3, p4);
        self.results.insert(idx, result);
    }

    fn get(&self, p1: usize, p2: usize, p3: usize, p4: usize) -> GameResult {
        let idx = Self::make_index(p1, p2, p3, p4);
        *self.results.get(&idx).unwrap_or(&GameResult::NotPlayed)
    }

    fn counts_in_ranges(
        &self,
        p1: Range<usize>,
        p2: Range<usize>,
        p3: Range<usize>,
        p4: Range<usize>,
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

    fn counts_in_lists(
        &self,
        p1: &[usize],
        p2: &[usize],
        p3: &[usize],
        p4: &[usize],
    ) -> [u32; 4] {
        let mut counts = [0u32; 4];
        for &i1 in p1 {
            for &i2 in p2 {
                for &i3 in p3 {
                    for &i4 in p4 {
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
    fn counts_over_range() {
        let mut db = InMemoryGameDatabase::new();
        db.set(0, 0, 0, 0, GameResult::Team1Win);
        let counts = db.counts_in_ranges(0..1, 0..1, 0..1, 0..1);
        assert_eq!(counts[GameResult::Team1Win as usize], 1);
        assert_eq!(counts[GameResult::NotPlayed as usize], 0);
    }

    #[test]
    fn counts_over_lists() {
        let mut db = InMemoryGameDatabase::new();
        db.set(0, 0, 0, 0, GameResult::Team1Win);
        let counts = db.counts_in_lists(&[0], &[0], &[0], &[0]);
        assert_eq!(counts[GameResult::Team1Win as usize], 1);
        assert_eq!(counts[GameResult::NotPlayed as usize], 0);
    }
}

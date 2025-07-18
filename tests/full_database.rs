use watten::game::GameState;
use watten::{GameResult, HAND_PERMUTATIONS};

#[test]
#[ignore]
fn full_database_population_after_clear() {
    let mut g = GameState::new(0);
    // Restrict to a single permutation first
    g.set_perm_range_single(0);
    g.start_round();
    // Only permutation 0 should exist
    assert_ne!(g.db.get(0, 0, 0, 0), GameResult::NotPlayed);
    assert_eq!(g.db.get(1, 0, 0, 0), GameResult::NotPlayed);

    // Enable full range and populate again
    g.clear_perm_range();
    g.start_round();

    let hi = HAND_PERMUTATIONS - 1;
    assert_ne!(g.db.get(hi, hi, hi, hi), GameResult::NotPlayed);
    assert_ne!(g.db.get(1, 0, 0, 0), GameResult::NotPlayed);
}

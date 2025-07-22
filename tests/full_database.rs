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

#[test]
fn estimate_full_database_population_time() {
    let mut g = GameState::new(0);
    // Use a small subset of permutations to keep the test fast
    let subset = vec![0, 1, 2];
    g.set_perm_range(subset.clone());
    g.set_workers(1);
    let start = std::time::Instant::now();
    g.start_round();
    let elapsed = start.elapsed();

    let subset_total = (subset.len() as u64).pow(4);
    let full_total = (HAND_PERMUTATIONS as u64).pow(4);
    let est_full = elapsed.as_secs_f64() * (full_total as f64 / subset_total as f64);

    println!(
        "Elapsed {:?} for {} plays -> estimated full database {:.2} seconds",
        elapsed, subset_total, est_full
    );

    assert_ne!(g.db.get(0, 0, 0, 0), GameResult::NotPlayed);
    assert!(est_full > 0.0);
}

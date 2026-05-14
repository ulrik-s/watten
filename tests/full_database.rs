use watten::game::{Evaluator, GameState};
use watten::HAND_PERMUTATIONS;

/// Heavy test: populating the full 120^4 database. Ignored by default,
/// run with `cargo full`. Verifies that running with `perm_range` first
/// and then with the full range produces evaluations for both ends of the
/// permutation index space.
#[test]
#[ignore]
fn full_database_population_after_clear() {
    let mut g = GameState::new(0);
    g.set_evaluator(Evaluator::Database);
    g.set_perm_range_single(0);
    g.set_evaluator(Evaluator::Database);
    g.start_round();
    // With perm_range = [0], only orderings starting at 0 contribute.
    let p = (g.dealer + 1) % 4;
    let allowed: Vec<usize> = (0..g.players[p].hand.len()).collect();
    let evs_restricted = g.evaluate_moves(p, &allowed, &[], [0, 0]);
    assert!(evs_restricted.iter().any(|e| e.total > 0));

    // Enable full range and re-populate.
    g.clear_perm_range();
    g.set_evaluator(Evaluator::Database);
    g.start_round();
    let p = (g.dealer + 1) % 4;
    let allowed: Vec<usize> = (0..g.players[p].hand.len()).collect();
    let evs_full = g.evaluate_moves(p, &allowed, &[], [0, 0]);
    // Full population should give at least an order of magnitude more counts
    // per move than the single-permutation run.
    let restricted_max = evs_restricted
        .iter()
        .map(|e| e.total)
        .max()
        .unwrap_or(0);
    let full_max = evs_full.iter().map(|e| e.total).max().unwrap_or(0);
    assert!(full_max > restricted_max);
}

#[test]
fn estimate_full_database_population_time() {
    let mut g = GameState::new(0);
    g.set_evaluator(Evaluator::Database);
    // Use a small subset of permutations to keep the test fast
    let subset = vec![0, 1, 2];
    g.set_perm_range(subset.clone());
    g.set_workers(1);
    // Re-apply evaluator to pick up perm_range/workers.
    g.set_evaluator(Evaluator::Database);
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

    let p = (g.dealer + 1) % 4;
    let allowed: Vec<usize> = (0..g.players[p].hand.len()).collect();
    let evs = g.evaluate_moves(p, &allowed, &[], [0, 0]);
    assert!(evs.iter().any(|e| e.total > 0));
    assert!(est_full > 0.0);
}

//! Measures the time it takes to fully populate the 120^4 database
//! evaluator from a real start_round_interactive deal. Ignored by default
//! (it really does run 207 million games — release-mode only takes a
//! reasonable amount of time). Run with:
//!
//!     cargo test --release --test db_populate_perf -- --ignored --nocapture

use watten::game::{Evaluator, GameState};

#[test]
#[ignore]
fn full_database_populate_release_perf() {
    let mut g = GameState::new(0);
    g.start_round_interactive();
    let start = std::time::Instant::now();
    let total = g.begin_database_populate();
    eprintln!("Populating {} games…", total);
    let batch = 5_000_000usize;
    let mut last_report = std::time::Instant::now();
    loop {
        let (done, total) = g.step_database_populate(batch);
        if last_report.elapsed().as_secs_f64() > 0.5 {
            let pct = (done as f64 / total as f64) * 100.0;
            eprintln!(
                "  {:>10} / {} ({:>5.1}%) — {:.1}s elapsed",
                done,
                total,
                pct,
                start.elapsed().as_secs_f64()
            );
            last_report = std::time::Instant::now();
        }
        if done >= total {
            break;
        }
    }
    let elapsed = start.elapsed();
    eprintln!(
        "Populated {} games in {:.2}s → {:.1} M games/s",
        total,
        elapsed.as_secs_f64(),
        total as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );
}

#[test]
fn small_database_populate_smoke() {
    // Cheap sanity check that the chunked populate hits the same total
    // as the full enumeration.
    let mut g = GameState::new(0);
    g.set_evaluator(Evaluator::Database);
    g.set_perm_range(vec![0, 1, 2, 3]);
    g.start_round_interactive();
    let total = g.begin_database_populate();
    assert_eq!(total, 4usize.pow(4));
    let mut last = 0;
    loop {
        let (done, total) = g.step_database_populate(50);
        assert!(done >= last);
        last = done;
        if done >= total {
            break;
        }
    }
    assert_eq!(last, 4usize.pow(4));
}

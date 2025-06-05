# Watten

Basic structures for a Watten card game trainer written in Rust.

The crate provides:

- A 33 card deck used in the game
- Utilities for enumerating all 120 possible orders in which a 5‑card hand can be played
- A database API that maps ordered plays of four players to a result (team 1/2 win, not played or rule violation)
- Functions for computing permutation ranges so partially played games can be matched

Run tests with `cargo test`.

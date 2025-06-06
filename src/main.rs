use std::io;
use watten::game::{GameState, WINNING_POINTS};
use watten::GameResult;

fn main() {
    println!("Play with a human player? [y/N]");
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let humans = if input.trim().eq_ignore_ascii_case("y") {
        1
    } else {
        0
    };

    let mut game = GameState::new(humans);
    while game.scores[0] < WINNING_POINTS && game.scores[1] < WINNING_POINTS {
        println!("\nStarting round. Dealer is player {}\n", game.dealer + 1);
        let result = game.play_round();
        match result {
            GameResult::Team1Win => println!("Team 1 wins the round"),
            GameResult::Team2Win => println!("Team 2 wins the round"),
            GameResult::RuleViolation => println!("A rule was violated"),
            GameResult::NotPlayed => println!("Round could not be played"),
        }
        println!(
            "Team 1: {} points, Team 2: {} points",
            game.scores[0], game.scores[1]
        );
    }
    if game.scores[0] >= WINNING_POINTS {
        println!("Team 1 wins the game!");
    } else {
        println!("Team 2 wins the game!");
    }
}

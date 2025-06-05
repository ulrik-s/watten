use rand::seq::SliceRandom;
use std::io::{self, Write};

use crate::{Card, Suit};

#[derive(Clone)]
pub struct Player {
    pub hand: Vec<Card>,
    pub human: bool,
}

impl Player {
    pub fn new(human: bool) -> Self {
        Self {
            hand: Vec::new(),
            human,
        }
    }

    pub fn play_card(&mut self, lead: Option<Suit>) -> Card {
        // determine playable card indices
        let playable: Vec<usize> = if let Some(s) = lead {
            let inds: Vec<usize> = self
                .hand
                .iter()
                .enumerate()
                .filter(|(_, c)| c.suit == s)
                .map(|(i, _)| i)
                .collect();
            if inds.is_empty() {
                (0..self.hand.len()).collect()
            } else {
                inds
            }
        } else {
            (0..self.hand.len()).collect()
        };

        if self.human {
            loop {
                println!("Your hand:");
                for (i, c) in self.hand.iter().enumerate() {
                    println!("  {}: {}", i + 1, c);
                }
                print!("Select card to play: ");
                io::stdout().flush().unwrap();
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                if let Ok(idx) = input.trim().parse::<usize>() {
                    if idx >= 1 && idx <= self.hand.len() && playable.contains(&(idx - 1)) {
                        return self.hand.remove(idx - 1);
                    }
                }
                println!("Invalid choice, try again.");
            }
        } else {
            let idx = *playable.choose(&mut rand::thread_rng()).unwrap();
            self.hand.remove(idx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rank, Suit};

    #[test]
    fn follow_suit_if_possible() {
        let mut p = Player::new(false);
        p.hand = vec![
            Card::new(Suit::Hearts, Rank::Seven),
            Card::new(Suit::Bells, Rank::Eight),
            Card::new(Suit::Leaves, Rank::Nine),
        ];
        let card = p.play_card(Some(Suit::Hearts));
        assert_eq!(card.suit, Suit::Hearts);
        assert_eq!(p.hand.len(), 2);
    }

    #[test]
    fn play_any_if_no_follow() {
        let mut p = Player::new(false);
        p.hand = vec![
            Card::new(Suit::Bells, Rank::Eight),
            Card::new(Suit::Leaves, Rank::Nine),
        ];
        let card = p.play_card(Some(Suit::Hearts));
        assert!(card.suit == Suit::Bells || card.suit == Suit::Leaves);
        assert_eq!(p.hand.len(), 1);
    }
}


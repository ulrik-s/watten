use rand::seq::SliceRandom;
use std::io::{self, Write};


fn format_hand(hand: &[Card], allowed: &[usize], rates: Option<&[f64]>) -> String {
    let mut out = String::new();
    for (i, c) in hand.iter().enumerate() {
        let mark = if allowed.contains(&i) {
            ""
        } else {
            " (not allowed)"
        };
        if let Some(r) = rates {
            out.push_str(&format!(
                "  {}: {} {:.1}%{}\n",
                i + 1,
                c,
                r[i] * 100.0,
                mark
            ));
        } else {
            out.push_str(&format!("  {}: {}{}\n", i + 1, c, mark));
        }
    }
    out
}

use crate::Card;

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

    pub fn play_card(&mut self, allowed: &[usize], rates: Option<&[f64]>) -> Card {
        if self.human {
            loop {
                println!("Your hand:");
                print!("{}", format_hand(&self.hand, allowed, rates));
                print!("Select card to play: ");
                io::stdout().flush().unwrap();
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                if let Ok(idx) = input.trim().parse::<usize>() {
                    if idx >= 1 && idx <= self.hand.len() && allowed.contains(&(idx - 1)) {
                        return self.hand.remove(idx - 1);
                    }
                }
                println!("Invalid choice, try again.");
            }
        } else {
            let idx = *allowed.choose(&mut rand::thread_rng()).unwrap();
            self.hand.remove(idx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rank, Suit};

    #[test]
    fn play_card_from_allowed_indices() {
        let mut p = Player::new(false);
        p.hand = vec![
            Card::new(Suit::Hearts, Rank::Seven),
            Card::new(Suit::Bells, Rank::Eight),
            Card::new(Suit::Leaves, Rank::Nine),
        ];
        let card = p.play_card(&[1], None);
        assert_eq!(card.suit, Suit::Bells);
        assert_eq!(p.hand.len(), 2);
    }

    #[test]
    fn play_random_from_multiple_allowed() {
        let mut p = Player::new(false);
        p.hand = vec![
            Card::new(Suit::Bells, Rank::Eight),
            Card::new(Suit::Leaves, Rank::Nine),
        ];
        let card = p.play_card(&[0, 1], None);
        assert!(card.suit == Suit::Bells || card.suit == Suit::Leaves);
        assert_eq!(p.hand.len(), 1);
    }

    #[test]
    fn format_hand_shows_percentages() {
        let p = Player {
            hand: vec![Card::new(Suit::Hearts, Rank::Seven)],
            human: true,
        };
        let text = format_hand(&p.hand, &[0], Some(&[0.42]));
        assert!(text.contains("42.0%"));
        assert!(text.contains("7 of Hearts"));
    }
}

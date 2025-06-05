use crate::{deck, shuffle, Card, Rank, Suit};
use crate::player::Player;

pub const WINNING_POINTS: usize = 13;
pub const PART_POINTS: usize = 2;
pub const PART_TRICKS: usize = 5;

pub fn rank_value(r: Rank) -> u8 {
    match r {
        Rank::Seven => 1,
        Rank::Eight => 2,
        Rank::Nine => 3,
        Rank::Ten => 4,
        Rank::Unter => 5,
        Rank::Ober => 6,
        Rank::King => 7,
        Rank::Ace => 8,
        Rank::Weli => 9,
    }
}

pub fn card_strength(card: &Card, lead: Suit, rechte: Card, position: usize) -> i16 {
    let trump_suit = rechte.suit;
    let striker_rank = rechte.rank;
    let base: i16 = if *card == rechte {
        200
    } else if card.rank == Rank::Weli {
        180
    } else if card.rank == striker_rank {
        // striker beats any trump except the rechte
        190
    } else if card.suit == trump_suit {
        100 + rank_value(card.rank) as i16
    } else if card.suit == lead {
        50 + rank_value(card.rank) as i16
    } else {
        rank_value(card.rank) as i16
    };
    base * 10 - position as i16
}

pub struct GameState {
    pub players: [Player; 4],
    pub dealer: usize,
    pub rechte: Option<Card>,
    pub scores: [usize; 2],
    pub part_points: usize,
}

impl GameState {
    pub fn new(human_players: usize) -> Self {
        let mut players = [
            Player::new(false),
            Player::new(false),
            Player::new(false),
            Player::new(false),
        ];
        for i in 0..human_players.min(4) {
            players[i].human = true;
        }
        Self {
            players,
            dealer: 0,
            rechte: None,
            scores: [0, 0],
            part_points: PART_POINTS,
        }
    }

    fn start_part(&mut self) {
        let mut cards = deck();
        shuffle(&mut cards);
        for p in self.players.iter_mut() {
            p.hand.clear();
        }
        for i in 0..PART_TRICKS {
            for j in 0..4 {
                let idx = (self.dealer + 1 + j) % 4;
                self.players[idx].hand.push(cards[i * 4 + j]);
            }
        }
        let dealer_card = self.players[self.dealer].hand[0];
        let next_idx = (self.dealer + 1) % 4;
        let next_card = self.players[next_idx].hand[0];
        self.rechte = Some(Card::new(dealer_card.suit, next_card.rank));
        println!("Trump suit is {}", dealer_card.suit);
        println!("Striker rank is {}", next_card.rank);
        println!("Rechte is {}", self.rechte.unwrap());
    }

    pub fn trump_suit(&self) -> Option<Suit> {
        self.rechte.map(|c| c.suit)
    }

    pub fn striker_rank(&self) -> Option<Rank> {
        self.rechte.map(|c| c.rank)
    }

    #[allow(unused_assignments)]
    pub fn play_part(&mut self) -> usize {
        self.start_part();
        let mut tricks = [0usize; 2];
        let mut lead = (self.dealer + 1) % 4;
        for _ in 0..PART_TRICKS {
            let mut played: Vec<(usize, Card)> = Vec::new();
            let lead_card = {
                let card = self.players[lead].play_card(None);
                println!("Player {} plays {}", lead + 1, card);
                played.push((lead, card));
                card
            };
            let lead_suit = lead_card.suit;
            for offset in 1..4 {
                let p_idx = (lead + offset) % 4;
                let card = self.players[p_idx].play_card(Some(lead_suit));
                println!("Player {} plays {}", p_idx + 1, card);
                played.push((p_idx, card));
            }
            let rechte = self.rechte.unwrap();
            let mut best = (played[0].0, played[0].1, 0usize);
            let mut best_score = card_strength(&best.1, lead_suit, rechte, 0);
            for (pos, &(idx, ref card)) in played.iter().enumerate().skip(1) {
                let val = card_strength(card, lead_suit, rechte, pos);
                if val > best_score {
                    best = (idx, *card, pos);
                    best_score = val;
                }
            }
            let (winner_idx, _, _) = best;
            println!("Player {} wins the trick\n", winner_idx + 1);
            tricks[winner_idx % 2] += 1;
            lead = winner_idx;
        }
        self.dealer = (self.dealer + 1) % 4;
        let winner = if tricks[0] > tricks[1] { 0 } else { 1 };
        self.scores[winner] += self.part_points;
        winner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn striker_beats_trump() {
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let striker = Card::new(Suit::Bells, Rank::Unter);
        let trump_card = Card::new(Suit::Hearts, Rank::Ace);
        let lead = Suit::Hearts;
        assert!(
            card_strength(&striker, lead, rechte, 0)
                > card_strength(&trump_card, lead, rechte, 0)
        );
    }

    #[test]
    fn rechte_beats_striker() {
        let rechte = Card::new(Suit::Leaves, Rank::Ober);
        let striker = Card::new(Suit::Hearts, Rank::Ober);
        let lead = Suit::Hearts;
        assert!(
            card_strength(&rechte, lead, rechte, 0)
                > card_strength(&striker, lead, rechte, 0)
        );
    }

    #[test]
    fn first_striker_played_beats_striker() {
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let lead = Suit::Bells;
        let first = Card::new(Suit::Bells, Rank::Unter);
        let second = Card::new(Suit::Leaves, Rank::Unter);
        let mut best = (0usize, first);
        let best_val = card_strength(&best.1, lead, rechte, 0);
        let candidate = (1usize, second);
        let val = card_strength(&candidate.1, lead, rechte, 1);
        if val > best_val {
            best = candidate;
        }
        assert_eq!(best.0, 0);
    }

    #[test]
    fn new_game_has_zero_scores() {
        let g = GameState::new(0);
        assert_eq!(g.scores, [0, 0]);
        assert_eq!(g.part_points, PART_POINTS);
    }

    #[test]
    fn play_card_whole_trick_first_striker_wins() {
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let mut players = [
            Player::new(false),
            Player::new(false),
            Player::new(false),
            Player::new(false),
        ];
        players[0].hand.push(Card::new(Suit::Bells, Rank::Unter));
        players[1].hand.push(Card::new(Suit::Leaves, Rank::Unter));
        players[2].hand.push(Card::new(Suit::Hearts, Rank::Ace));
        players[3].hand.push(Card::new(Suit::Acorns, Rank::Ace));

        let lead_card = players[0].play_card(None);
        let lead_suit = lead_card.suit;
        let mut cards = vec![lead_card];
        for i in 1..4 {
            cards.push(players[i].play_card(Some(lead_suit)));
        }

        let mut best_idx = 0usize;
        let mut best_val = card_strength(&cards[0], lead_suit, rechte, 0);
        for (pos, card) in cards.iter().enumerate().skip(1) {
            let val = card_strength(card, lead_suit, rechte, pos);
            if val > best_val {
                best_idx = pos;
                best_val = val;
            }
        }

        assert_eq!(best_idx, 0);
        for p in &players {
            assert!(p.hand.is_empty());
        }
    }

    #[test]
    fn play_card_whole_trick_rechte_wins_even_last() {
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let mut players = [
            Player::new(false),
            Player::new(false),
            Player::new(false),
            Player::new(false),
        ];
        players[0].hand.push(Card::new(Suit::Hearts, Rank::Ace));
        players[1].hand.push(Card::new(Suit::Bells, Rank::Unter));
        players[2].hand.push(Card::new(Suit::Hearts, Rank::Nine));
        players[3].hand.push(rechte); // hearts unter

        let lead_card = players[0].play_card(None);
        let lead_suit = lead_card.suit;
        let mut cards = vec![lead_card];
        for i in 1..4 {
            cards.push(players[i].play_card(Some(lead_suit)));
        }

        let mut best_idx = 0usize;
        let mut best_val = card_strength(&cards[0], lead_suit, rechte, 0);
        for (pos, card) in cards.iter().enumerate().skip(1) {
            let val = card_strength(card, lead_suit, rechte, pos);
            if val > best_val {
                best_idx = pos;
                best_val = val;
            }
        }

        assert_eq!(best_idx, 3);
        for p in &players {
            assert!(p.hand.is_empty());
        }
    }
}


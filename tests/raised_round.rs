use watten::game::{card_strength, GameState};
use watten::{Card, Rank, Suit};

fn manual_round(hands: &mut [Vec<Card>; 4], dealer: usize, rechte: Card) -> (Vec<usize>, [usize; 2]) {
    let mut lead = (dealer + 1) % 4;
    let mut winners = Vec::new();
    let mut tricks = [0usize; 2];
    for _ in 0..watten::game::TRICKS_PER_ROUND {
        let lead_card = hands[lead].remove(0);
        let lead_suit = lead_card.suit;
        let mut played = vec![(lead, lead_card)];
        for off in 1..4 {
            let idx = (lead + off) % 4;
            let card = hands[idx].remove(0);
            played.push((idx, card));
        }
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
        winners.push(winner_idx);
        tricks[winner_idx % 2] += 1;
        lead = winner_idx;
    }
    (winners, tricks)
}

#[test]
fn raising_points_and_full_round() {
    use Rank::*;
    use Suit::*;
    let rechte = Card::new(Hearts, Unter);
    let original_hands = [
        vec![Card::new(Hearts, Unter), Card::new(Bells, Ace), Card::new(Leaves, King), Card::new(Hearts, Ace), Card::new(Acorns, Ten)],
        vec![Card::new(Hearts, Ten), Card::new(Bells, King), Card::new(Leaves, Ace), Card::new(Bells, Seven), Card::new(Acorns, Nine)],
        vec![Card::new(Hearts, King), Card::new(Leaves, Ober), Card::new(Bells, Nine), Card::new(Hearts, Nine), Card::new(Acorns, Unter)],
        vec![Card::new(Hearts, Ober), Card::new(Bells, Unter), Card::new(Leaves, Nine), Card::new(Acorns, Ace), Card::new(Bells, Ten)],
    ];
    let mut g = GameState::new(0);
    g.dealer = 0;
    g.rechte = Some(rechte);
    for i in 0..4 {
        g.players[i].hand = original_hands[i].clone();
    }
    assert_eq!(g.round_points, watten::game::ROUND_POINTS);
    assert!(g.raise_round(0).is_ok());
    assert!(g.raise_round(1).is_ok());
    assert_eq!(g.round_points, watten::game::ROUND_POINTS + 2);

    let mut hands = original_hands.clone();
    let (winners, tricks) = manual_round(&mut hands, g.dealer, rechte);
    assert_eq!(winners.len(), watten::game::TRICKS_PER_ROUND);
    let result = if tricks[0] > tricks[1] { watten::GameResult::Team1Win } else { watten::GameResult::Team2Win };
    match result {
        watten::GameResult::Team1Win => g.scores[0] += g.round_points,
        watten::GameResult::Team2Win => g.scores[1] += g.round_points,
        _ => {}
    }
    assert_eq!(result, watten::GameResult::Team1Win);
    assert_eq!(g.scores, [watten::game::ROUND_POINTS + 2, 0]);
}

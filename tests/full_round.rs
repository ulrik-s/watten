use watten::game::{card_strength, GameState};
use watten::{Card, Rank, Suit};

#[test]
fn seeing_players_follow_trump_full_trick() {
    use Rank::*;
    use Suit::*;

    let mut g = GameState::new(0);
    g.dealer = 0;
    g.rechte = Some(Card::new(Hearts, Unter));

    // Dealer is player 0, so players 0 and 1 are the seeing players.
    // Player 1 leads the trick as the player after the dealer.
    g.players[0].hand = vec![Card::new(Hearts, Ace), Card::new(Bells, Ace)];
    g.players[1].hand = vec![Card::new(Hearts, Ten)];
    g.players[2].hand = vec![Card::new(Bells, Ober), Card::new(Hearts, Seven)];
    g.players[3].hand = vec![Card::new(Leaves, Unter), Card::new(Bells, Ten)];

    // Player 1 leads with trump
    let lead_card = g.players[1].hand[0];
    let allowed_lead = g.allowed_indices(1, lead_card);
    assert_eq!(allowed_lead, vec![0]);
    let lead_card = g.players[1].hand.remove(allowed_lead[0]);
    let mut played = vec![(1usize, lead_card)];

    let allowed2 = g.allowed_indices(2, lead_card);
    assert_eq!(allowed2, vec![0, 1]); // not seeing, no restriction
    let card2 = g.players[2].hand.remove(allowed2[0]);
    played.push((2usize, card2));

    let allowed3 = g.allowed_indices(3, lead_card);
    assert_eq!(allowed3, vec![0, 1]);
    let card3 = g.players[3].hand.remove(allowed3[0]);
    played.push((3usize, card3));

    let allowed0 = g.allowed_indices(0, lead_card);
    assert_eq!(allowed0, vec![0]); // dealer must play trump or striker
    let card0 = g.players[0].hand.remove(allowed0[0]);
    played.push((0usize, card0));

    let rechte = g.rechte.unwrap();
    let mut best = (played[0].0, played[0].1, 0usize);
    let mut best_score = card_strength(&best.1, lead_card.suit, rechte, 0);
    for (pos, &(idx, ref card)) in played.iter().enumerate().skip(1) {
        let val = card_strength(card, lead_card.suit, rechte, pos);
        if val > best_score {
            best = (idx, *card, pos);
            best_score = val;
        }
    }
    assert_eq!(best.0, 3); // striker should win
}

fn manual_round(
    hands: &mut [Vec<Card>; 4],
    dealer: usize,
    rechte: Card,
) -> (Vec<usize>, [usize; 2]) {
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
fn full_round_five_tricks_winners() {
    use Rank::*;
    use Suit::*;
    let rechte = Card::new(Hearts, Unter);
    let mut hands = [
        vec![
            Card::new(Hearts, Unter),
            Card::new(Bells, Ace),
            Card::new(Leaves, King),
            Card::new(Hearts, Ace),
            Card::new(Acorns, Ten),
        ],
        vec![
            Card::new(Hearts, Ten),
            Card::new(Bells, King),
            Card::new(Leaves, Ace),
            Card::new(Bells, Seven),
            Card::new(Acorns, Nine),
        ],
        vec![
            Card::new(Hearts, King),
            Card::new(Leaves, Ober),
            Card::new(Bells, Nine),
            Card::new(Hearts, Nine),
            Card::new(Acorns, Unter),
        ],
        vec![
            Card::new(Hearts, Ober),
            Card::new(Bells, Unter),
            Card::new(Leaves, Nine),
            Card::new(Acorns, Ace),
            Card::new(Bells, Ten),
        ],
    ];
    let (winners, _tricks) = manual_round(&mut hands, 0, rechte);
    assert_eq!(winners, vec![0, 3, 1, 0, 2]);
}

#[test]
fn counting_points_after_rounds() {
    use Rank::*;
    use Suit::*;
    let rechte = Card::new(Hearts, Unter);
    let original_hands = [
        vec![
            Card::new(Hearts, Unter),
            Card::new(Bells, Ace),
            Card::new(Leaves, King),
            Card::new(Hearts, Ace),
            Card::new(Acorns, Ten),
        ],
        vec![
            Card::new(Hearts, Ten),
            Card::new(Bells, King),
            Card::new(Leaves, Ace),
            Card::new(Bells, Seven),
            Card::new(Acorns, Nine),
        ],
        vec![
            Card::new(Hearts, King),
            Card::new(Leaves, Ober),
            Card::new(Bells, Nine),
            Card::new(Hearts, Nine),
            Card::new(Acorns, Unter),
        ],
        vec![
            Card::new(Hearts, Ober),
            Card::new(Bells, Unter),
            Card::new(Leaves, Nine),
            Card::new(Acorns, Ace),
            Card::new(Bells, Ten),
        ],
    ];
    let mut g = GameState::new(0);
    g.dealer = 0;
    g.rechte = Some(rechte);
    for i in 0..4 {
        g.players[i].hand = original_hands[i].clone();
    }
    let (_winners, tricks) = manual_round(&mut original_hands.clone(), g.dealer, rechte);
    let result = if tricks[0] > tricks[1] {
        watten::GameResult::Team1Win
    } else {
        watten::GameResult::Team2Win
    };
    match result {
        watten::GameResult::Team1Win => g.scores[0] += g.round_points,
        watten::GameResult::Team2Win => g.scores[1] += g.round_points,
        _ => {}
    }
    assert_eq!(result, watten::GameResult::Team1Win);
    assert_eq!(g.scores, [watten::game::ROUND_POINTS, 0]);

    // play the same round again to verify accumulation
    let mut hands2 = original_hands.clone();
    let (_winners2, tricks2) = manual_round(&mut hands2, g.dealer, rechte);
    let result2 = if tricks2[0] > tricks2[1] {
        watten::GameResult::Team1Win
    } else {
        watten::GameResult::Team2Win
    };
    match result2 {
        watten::GameResult::Team1Win => g.scores[0] += g.round_points,
        watten::GameResult::Team2Win => g.scores[1] += g.round_points,
        _ => {}
    }
    assert_eq!(g.scores, [watten::game::ROUND_POINTS * 2, 0]);
}

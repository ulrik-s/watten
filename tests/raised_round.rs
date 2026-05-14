use watten::game::{card_strength, GameState};
use watten::{Card, Rank, Suit};

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
fn raising_points_and_full_round() {
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
    assert_eq!(g.round_points, watten::game::ROUND_POINTS);
    g.playing_round = true; // synthetic setup; bypass start_round
    assert!(g.propose_raise(0).is_ok());
    assert!(g.respond_to_raise(1, true).is_ok());
    assert!(g.propose_raise(1).is_ok());
    assert!(g.respond_to_raise(0, true).is_ok());
    assert_eq!(g.round_points, watten::game::ROUND_POINTS + 2);

    let mut hands = original_hands.clone();
    let (winners, tricks) = manual_round(&mut hands, g.dealer, rechte);
    assert_eq!(winners.len(), watten::game::TRICKS_PER_ROUND);
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
    assert_eq!(g.scores, [watten::game::ROUND_POINTS + 2, 0]);
}

#[test]
fn concede_locks_in_winner_and_round_plays_to_completion() {
    // Drive a full interactive round: start, concede right away, then have
    // the engine auto-play every remaining card. Verify that no new deal
    // happens until every hand is empty, and that the conceded team
    // receives exactly round_points at finish_round time.
    let mut g = watten::game::GameState::new(0);
    g.start_round_interactive();
    let dealer_at_start = g.dealer;
    // Every player has 5 cards immediately after a deal.
    for p in 0..4 {
        assert_eq!(g.players[p].hand.len(), 5);
    }
    // Team 1 concedes right away.
    let _ = g.concede_round(0).unwrap();
    assert_eq!(g.round_decided(), Some(1));
    // Scores must not have moved yet, and the round is still in progress.
    assert_eq!(g.scores, [0, 0]);
    assert!(g.playing_round);
    // Cards have not been re-dealt: every player still has the same 5 cards.
    for p in 0..4 {
        assert_eq!(g.players[p].hand.len(), 5);
    }
    // Play out the round automatically (bot picks for every player).
    let steps = g.auto_play_round();
    // 20 plays total (5 tricks × 4 players).
    assert_eq!(steps.len(), 20);
    // Round has ended and the dealer rotated.
    assert!(!g.playing_round);
    assert_eq!(g.dealer, (dealer_at_start + 1) % 4);
    // Team 2 got the conceded points (round_points stayed at the default
    // because nobody raised).
    assert_eq!(g.scores, [0, watten::game::ROUND_POINTS]);
    // round_decided cleared on finish.
    assert_eq!(g.round_decided(), None);
}

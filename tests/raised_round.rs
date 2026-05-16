use watten::game::{trick_winner_position, GameState};
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
        let mut played = vec![(lead, lead_card)];
        for off in 1..4 {
            let idx = (lead + off) % 4;
            let card = hands[idx].remove(0);
            played.push((idx, card));
        }
        let trick_cards: Vec<Card> = played.iter().map(|(_, c)| *c).collect();
        let winner_pos = trick_winner_position(&trick_cards, rechte);
        let winner_idx = played[winner_pos].0;
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

/// Reproduces the user-reported bug: after Team 1 raises to 3 and Team 2
/// accepts, Team 1 should receive 3 points if they win the round (not the
/// pre-raise 2). Verified end-to-end with a real start_round_interactive
/// deal driven through auto_play_round.
#[test]
fn accepted_raise_pays_out_at_increased_value() {
    let mut g = watten::game::GameState::new(0); // no humans — all bots
    g.start_round_interactive();
    let dealer = g.dealer;
    let scores_before = g.scores;

    // Team 1 (idx 0) proposes a raise; Team 2 (idx 1) accepts.
    assert_eq!(g.round_points, watten::game::ROUND_POINTS, "starts at default");
    assert!(g.propose_raise(0).is_ok());
    let outcome = g
        .respond_to_raise(1, true)
        .expect("Team 2 should be able to accept");
    use watten::game::RaiseOutcome;
    match outcome {
        RaiseOutcome::Accepted { new_value, .. } => {
            assert_eq!(new_value, watten::game::ROUND_POINTS + 1);
            assert_eq!(g.round_points, watten::game::ROUND_POINTS + 1);
        }
        _ => panic!("expected Accepted, got {:?}", outcome),
    }

    // Play out every remaining card and let the engine call finish_round.
    let steps = g.auto_play_round();
    assert_eq!(steps.len(), 20, "a round is always 5 tricks × 4 plays");
    assert!(!g.playing_round, "round should have ended");
    assert_eq!(g.dealer, (dealer + 1) % 4, "dealer rotates after a round");

    // Exactly one team gained points, and the gain equals the *raised*
    // value (ROUND_POINTS + 1), not the pre-raise value.
    let gained_team_1 = g.scores[0] - scores_before[0];
    let gained_team_2 = g.scores[1] - scores_before[1];
    assert_eq!(gained_team_1 + gained_team_2, watten::game::ROUND_POINTS + 1,
        "the round must pay out at the raised value (ROUND_POINTS+1 = 3), \
         got team 1 gained {} and team 2 gained {}",
        gained_team_1, gained_team_2);
}

/// Two accepted raises (one by each team) should pay out at ROUND_POINTS+2.
#[test]
fn two_accepted_raises_pay_out_at_the_compounded_value() {
    let mut g = watten::game::GameState::new(0);
    g.start_round_interactive();
    let scores_before = g.scores;

    assert!(g.propose_raise(0).is_ok());
    assert!(g.respond_to_raise(1, true).is_ok());
    assert_eq!(g.round_points, watten::game::ROUND_POINTS + 1);
    assert!(g.propose_raise(1).is_ok());
    assert!(g.respond_to_raise(0, true).is_ok());
    assert_eq!(g.round_points, watten::game::ROUND_POINTS + 2);

    let _ = g.auto_play_round();
    let gained = (g.scores[0] - scores_before[0]) + (g.scores[1] - scores_before[1]);
    assert_eq!(gained, watten::game::ROUND_POINTS + 2);
}

#[test]
fn human_must_play_five_cards_to_finish_a_round() {
    // Reproduces the user's report: after one human click, only the 5
    // tricks should not auto-complete. The human must press five times.
    let mut g = watten::game::GameState::new(1); // 1 human (player 0)
    g.start_round_interactive();

    // Bots advance up to the human's turn — the human's hand must remain at 5.
    let (res, _) = g.advance_bots();
    assert_eq!(res, None, "bots should pause at the human's turn");
    assert_eq!(g.players[0].hand.len(), 5);

    // Now simulate the human playing one card. The human plays current-hand
    // index 0; after that, bots advance again. The round must not be over
    // yet — only 1 trick has been completed.
    let (res1, _) = g.human_play(0);
    assert_eq!(
        res1, None,
        "round must not end after just one human play (got result)"
    );
    assert_eq!(
        g.players[0].hand.len(),
        4,
        "human should now have 4 cards left"
    );

    // Repeat: play 4 more cards. The round should end on (or before) the
    // 5th human play, never earlier.
    for n in 0..4 {
        let (res, _) = g.human_play(0);
        if n < 3 {
            assert_eq!(
                res, None,
                "round ended early after human's {}-th additional play",
                n + 2
            );
        } else {
            // Final play: the round must now have ended.
            assert!(res.is_some(), "round should end on the 5th human play");
        }
    }
    assert!(!g.playing_round);
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

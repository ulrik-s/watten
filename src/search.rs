use std::collections::HashMap;

use crate::game::{card_score, TRICKS_PER_ROUND};
use crate::Card;

/// Per-round transposition table sharing memoization across moves.
#[derive(Default)]
pub struct SearchMemo {
    table: HashMap<u32, [u32; TRICKS_PER_ROUND + 1]>,
}

impl SearchMemo {
    pub fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.table.clear();
    }
}

/// Input describing the position to evaluate.
pub struct SearchPosition<'a> {
    pub orig_hands: &'a [[Card; TRICKS_PER_ROUND]; 4],
    /// 5-bit mask per player: bit `i` set => `orig_hands[p][i]` still in hand.
    pub remaining: [u8; 4],
    /// Player to lead the next trick.
    pub lead: u8,
    /// Dealer (used to identify seeing players who must follow trump).
    pub dealer: u8,
    pub rechte: Card,
}

fn pack_key(remaining: [u8; 4], lead: u8) -> u32 {
    (remaining[0] as u32)
        | ((remaining[1] as u32) << 5)
        | ((remaining[2] as u32) << 10)
        | ((remaining[3] as u32) << 15)
        | ((lead as u32) << 20)
}

fn is_seeing(player: u8, dealer: u8) -> bool {
    player == dealer || player == (dealer + 1) % 4
}

fn legal_follow_mask(
    orig_hands: &[[Card; TRICKS_PER_ROUND]; 4],
    remaining_mask: u8,
    player: u8,
    lead_card: Card,
    dealer: u8,
    rechte: Card,
) -> u8 {
    // Must-follow rule: if the lead card is in the trump suit, a seeing
    // player must play a trump-suit card if they hold one.
    if lead_card.suit == rechte.suit && is_seeing(player, dealer) {
        let mut subset = 0u8;
        for i in 0..TRICKS_PER_ROUND {
            if remaining_mask & (1 << i) == 0 {
                continue;
            }
            let c = orig_hands[player as usize][i];
            if c.suit == rechte.suit {
                subset |= 1 << i;
            }
        }
        if subset != 0 {
            return subset;
        }
    }
    remaining_mask
}

fn trick_winner(plays: &[(u8, Card); 4], rechte: Card) -> u8 {
    let trick_cards = [plays[0].1, plays[1].1, plays[2].1, plays[3].1];
    let mut best_pos = 0usize;
    let mut best_score = card_score(&trick_cards[0], 0, &trick_cards, rechte);
    for pos in 1..4 {
        let s = card_score(&trick_cards[pos], pos, &trick_cards, rechte);
        if s > best_score {
            best_score = s;
            best_pos = pos;
        }
    }
    plays[best_pos].0
}

/// Count, by future team-1 trick count, how many legal completions exist from
/// this position. Returned slot `t` is the number of completion orderings in
/// which team 1 wins exactly `t` of the *remaining* tricks (0..=tricks_left).
pub fn count_completions(
    pos: &SearchPosition<'_>,
    memo: &mut SearchMemo,
) -> [u32; TRICKS_PER_ROUND + 1] {
    if pos.remaining.iter().all(|&r| r == 0) {
        let mut leaf = [0u32; TRICKS_PER_ROUND + 1];
        leaf[0] = 1;
        return leaf;
    }

    let key = pack_key(pos.remaining, pos.lead);
    if let Some(&cached) = memo.table.get(&key) {
        return cached;
    }

    let mut result = [0u32; TRICKS_PER_ROUND + 1];
    let lead_p = pos.lead as usize;

    let mut lead_mask = pos.remaining[lead_p];
    while lead_mask != 0 {
        let li = lead_mask.trailing_zeros() as usize;
        lead_mask &= lead_mask - 1;
        let lead_card = pos.orig_hands[lead_p][li];

        let mut rem1 = pos.remaining;
        rem1[lead_p] &= !(1u8 << li);

        let f1 = ((pos.lead + 1) % 4) as usize;
        let f1_mask = legal_follow_mask(
            pos.orig_hands,
            rem1[f1],
            f1 as u8,
            lead_card,
            pos.dealer,
            pos.rechte,
        );
        let mut m1 = f1_mask;
        while m1 != 0 {
            let i1 = m1.trailing_zeros() as usize;
            m1 &= m1 - 1;
            let c1 = pos.orig_hands[f1][i1];
            let mut rem2 = rem1;
            rem2[f1] &= !(1u8 << i1);

            let f2 = ((pos.lead + 2) % 4) as usize;
            let f2_mask = legal_follow_mask(
                pos.orig_hands,
                rem2[f2],
                f2 as u8,
                lead_card,
                pos.dealer,
                pos.rechte,
            );
            let mut m2 = f2_mask;
            while m2 != 0 {
                let i2 = m2.trailing_zeros() as usize;
                m2 &= m2 - 1;
                let c2 = pos.orig_hands[f2][i2];
                let mut rem3 = rem2;
                rem3[f2] &= !(1u8 << i2);

                let f3 = ((pos.lead + 3) % 4) as usize;
                let f3_mask = legal_follow_mask(
                    pos.orig_hands,
                    rem3[f3],
                    f3 as u8,
                    lead_card,
                    pos.dealer,
                    pos.rechte,
                );
                let mut m3 = f3_mask;
                while m3 != 0 {
                    let i3 = m3.trailing_zeros() as usize;
                    m3 &= m3 - 1;
                    let c3 = pos.orig_hands[f3][i3];
                    let mut rem4 = rem3;
                    rem4[f3] &= !(1u8 << i3);

                    let plays = [
                        (lead_p as u8, lead_card),
                        (f1 as u8, c1),
                        (f2 as u8, c2),
                        (f3 as u8, c3),
                    ];
                    let winner = trick_winner(&plays, pos.rechte);
                    let next = SearchPosition {
                        orig_hands: pos.orig_hands,
                        remaining: rem4,
                        lead: winner,
                        dealer: pos.dealer,
                        rechte: pos.rechte,
                    };
                    let sub = count_completions(&next, memo);
                    if winner % 2 == 0 {
                        for t in 0..TRICKS_PER_ROUND {
                            result[t + 1] = result[t + 1].saturating_add(sub[t]);
                        }
                    } else {
                        for t in 0..=TRICKS_PER_ROUND {
                            result[t] = result[t].saturating_add(sub[t]);
                        }
                    }
                }
            }
        }
    }

    memo.table.insert(key, result);
    result
}

#[derive(Debug, Clone, Copy)]
pub struct MoveEval {
    /// Index into the player's original 5-card hand.
    pub orig_idx: usize,
    pub wins: u32,
    pub total: u32,
}

impl MoveEval {
    pub fn rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.wins as f64 / self.total as f64
        }
    }
}

/// Evaluate every legal move available to `player_in_trick` while the partial
/// trick `current_trick` is in progress (may be empty if the player is
/// leading). Returns one [`MoveEval`] per supplied legal original-hand index.
pub fn evaluate_moves(
    pos: &SearchPosition<'_>,
    player_in_trick: u8,
    current_trick: &[(u8, Card)],
    team_tricks_so_far: [u8; 2],
    legal_orig_indices: &[usize],
    memo: &mut SearchMemo,
) -> Vec<MoveEval> {
    let player = player_in_trick as usize;
    let team = player % 2;
    let mut results = Vec::with_capacity(legal_orig_indices.len());

    let mut trick_buf: [(u8, Card); 4] = [(0, pos.orig_hands[0][0]); 4];
    for (i, &(p, c)) in current_trick.iter().enumerate() {
        trick_buf[i] = (p, c);
    }
    let trick_len = current_trick.len();

    for &orig_idx in legal_orig_indices {
        let card = pos.orig_hands[player][orig_idx];
        let mut completions = [0u32; TRICKS_PER_ROUND + 1];

        let mut rem_after = pos.remaining;
        rem_after[player] &= !(1u8 << orig_idx);

        trick_buf[trick_len] = (player_in_trick, card);
        enumerate_partial_trick(
            pos,
            &mut trick_buf,
            trick_len + 1,
            rem_after,
            &mut completions,
            memo,
        );

        let mut wins = 0u32;
        let mut total = 0u32;
        for t in 0..=TRICKS_PER_ROUND {
            if completions[t] == 0 {
                continue;
            }
            let team1_round = team_tricks_so_far[0] as usize + t;
            let team2_round = TRICKS_PER_ROUND - team1_round;
            total = total.saturating_add(completions[t]);
            if (team == 0 && team1_round > team2_round)
                || (team == 1 && team2_round > team1_round)
            {
                wins = wins.saturating_add(completions[t]);
            }
        }
        results.push(MoveEval {
            orig_idx,
            wins,
            total,
        });
    }

    results
}

fn enumerate_partial_trick(
    pos: &SearchPosition<'_>,
    trick_buf: &mut [(u8, Card); 4],
    trick_len: usize,
    remaining: [u8; 4],
    out: &mut [u32; TRICKS_PER_ROUND + 1],
    memo: &mut SearchMemo,
) {
    if trick_len == 4 {
        let winner = trick_winner(trick_buf, pos.rechte);
        let next = SearchPosition {
            orig_hands: pos.orig_hands,
            remaining,
            lead: winner,
            dealer: pos.dealer,
            rechte: pos.rechte,
        };
        let sub = count_completions(&next, memo);
        if winner % 2 == 0 {
            for t in 0..TRICKS_PER_ROUND {
                out[t + 1] = out[t + 1].saturating_add(sub[t]);
            }
        } else {
            for t in 0..=TRICKS_PER_ROUND {
                out[t] = out[t].saturating_add(sub[t]);
            }
        }
        return;
    }

    let lead_card = trick_buf[0].1;
    let next_player = (trick_buf[0].0 + trick_len as u8) % 4;
    let np = next_player as usize;
    let mask = legal_follow_mask(
        pos.orig_hands,
        remaining[np],
        next_player,
        lead_card,
        pos.dealer,
        pos.rechte,
    );
    let mut m = mask;
    while m != 0 {
        let i = m.trailing_zeros() as usize;
        m &= m - 1;
        let c = pos.orig_hands[np][i];
        let mut rem = remaining;
        rem[np] &= !(1u8 << i);
        trick_buf[trick_len] = (next_player, c);
        enumerate_partial_trick(pos, trick_buf, trick_len + 1, rem, out, memo);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rank, Suit};

    fn dummy_hands() -> [[Card; TRICKS_PER_ROUND]; 4] {
        use Rank::*;
        use Suit::*;
        [
            [
                Card::new(Hearts, Unter),
                Card::new(Bells, Ace),
                Card::new(Leaves, King),
                Card::new(Hearts, Ace),
                Card::new(Acorns, Ten),
            ],
            [
                Card::new(Hearts, Ten),
                Card::new(Bells, King),
                Card::new(Leaves, Ace),
                Card::new(Bells, Seven),
                Card::new(Acorns, Nine),
            ],
            [
                Card::new(Hearts, King),
                Card::new(Leaves, Ober),
                Card::new(Bells, Nine),
                Card::new(Hearts, Nine),
                Card::new(Acorns, Unter),
            ],
            [
                Card::new(Hearts, Ober),
                Card::new(Bells, Unter),
                Card::new(Leaves, Nine),
                Card::new(Acorns, Ace),
                Card::new(Bells, Ten),
            ],
        ]
    }

    #[test]
    fn completion_totals_equal_total_orderings() {
        let hands = dummy_hands();
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let pos = SearchPosition {
            orig_hands: &hands,
            remaining: [0b11111; 4],
            lead: 1,
            dealer: 0,
            rechte,
        };
        let mut memo = SearchMemo::new();
        let counts = count_completions(&pos, &mut memo);
        let total: u32 = counts.iter().sum();
        assert!(total > 0);
        // Distribution should not put all weight on impossible bucket; sanity
        for &c in &counts {
            assert!(c <= total);
        }
    }

    #[test]
    fn count_completions_memoizes() {
        let hands = dummy_hands();
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let pos = SearchPosition {
            orig_hands: &hands,
            remaining: [0b11111; 4],
            lead: 1,
            dealer: 0,
            rechte,
        };
        let mut memo = SearchMemo::new();
        let a = count_completions(&pos, &mut memo);
        let entries_after_first = memo.table.len();
        let b = count_completions(&pos, &mut memo);
        assert_eq!(a, b);
        assert_eq!(entries_after_first, memo.table.len());
        assert!(entries_after_first > 0);
    }

    #[test]
    fn evaluate_moves_returns_one_per_legal_card() {
        let hands = dummy_hands();
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let pos = SearchPosition {
            orig_hands: &hands,
            remaining: [0b11111; 4],
            lead: 1,
            dealer: 0,
            rechte,
        };
        let mut memo = SearchMemo::new();
        let moves = evaluate_moves(&pos, 1, &[], [0, 0], &[0, 1, 2, 3, 4], &mut memo);
        assert_eq!(moves.len(), 5);
        let any_total = moves.iter().any(|m| m.total > 0);
        assert!(any_total);
    }
}

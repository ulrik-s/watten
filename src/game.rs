use crate::database::{GameDatabase, InMemoryGameDatabase};
use crate::player::Player;
use crate::{all_hand_orders, deck, perm_prefix_range, shuffle, Card, GameResult, Rank, Suit};
use num_cpus;
use serde::Serialize;

pub const WINNING_POINTS: usize = 13;
pub const ROUND_POINTS: usize = 2;
pub const TRICKS_PER_ROUND: usize = 5;

const DUMMY_CARD: Card = Card {
    suit: Suit::Hearts,
    rank: Rank::Seven,
};

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

fn simulate_game(
    hands: &[[Card; TRICKS_PER_ROUND]; 4],
    perms: [[usize; TRICKS_PER_ROUND]; 4],
    dealer: usize,
    rechte: Card,
) -> GameResult {
    let mut pos = [0usize; 4];
    let mut lead = (dealer + 1) % 4;
    let mut tricks = [0usize; 2];
    for _ in 0..TRICKS_PER_ROUND {
        let lead_card = hands[lead][perms[lead][pos[lead]]];
        pos[lead] += 1;
        let lead_suit = lead_card.suit;
        let mut played = vec![(lead, lead_card)];
        for off in 1..4 {
            let idx = (lead + off) % 4;
            let card = hands[idx][perms[idx][pos[idx]]];
            pos[idx] += 1;
            played.push((idx, card));
        }
        let mut best = (played[0].0, played[0].1, 0usize);
        let mut best_score = card_strength(&best.1, lead_suit, rechte, 0);
        for (p, &(idx, c)) in played.iter().enumerate().skip(1) {
            let val = card_strength(&c, lead_suit, rechte, p);
            if val > best_score {
                best = (idx, c, p);
                best_score = val;
            }
        }
        let winner_idx = best.0;
        tricks[winner_idx % 2] += 1;
        lead = winner_idx;
    }
    if tricks[0] > tricks[1] {
        GameResult::Team1Win
    } else {
        GameResult::Team2Win
    }
}

/// Play a round using specific hand permutation ids for each player.
pub fn play_hand(
    hands: &[[Card; TRICKS_PER_ROUND]; 4],
    hand_ids: [usize; 4],
    dealer: usize,
    rechte: Card,
    perms: &[[usize; 5]],
) -> GameResult {
    let orders = [
        perms[hand_ids[0]],
        perms[hand_ids[1]],
        perms[hand_ids[2]],
        perms[hand_ids[3]],
    ];
    simulate_game(hands, orders, dealer, rechte)
}

#[derive(Clone, Serialize)]
pub struct RoundStep {
    pub player: usize,
    pub hand: Vec<Card>,
    pub allowed: Vec<usize>,
    pub played: Card,
}

pub struct GameState {
    pub players: [Player; 4],
    pub dealer: usize,
    pub rechte: Option<Card>,
    pub scores: [usize; 2],
    pub round_points: usize,
    /// Which team last raised the round points.
    last_raiser: Option<usize>,
    pub db: Box<dyn GameDatabase>,
    orig_hands: [[Card; TRICKS_PER_ROUND]; 4],
    played: [Vec<usize>; 4],
    /// Optional subset of permutation indices for populating the database
    perm_range: Option<Vec<usize>>,
    /// Number of worker threads used to populate the database
    workers: usize,
    /// Callback for progress updates during database population
    progress_cb: Option<Box<dyn Fn(u64)>>,
    // interactive round state
    playing_round: bool,
    trick_lead: usize,
    trick_pos: usize,
    tricks_won: [usize; 2],
    current_trick: Vec<(usize, Card)>,
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
            round_points: ROUND_POINTS,
            last_raiser: None,
            db: Box::new(InMemoryGameDatabase::new()),
            orig_hands: [[DUMMY_CARD; TRICKS_PER_ROUND]; 4],
            played: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            perm_range: None,
            workers: num_cpus::get() * 2,
            progress_cb: None,
            playing_round: false,
            trick_lead: 0,
            trick_pos: 0,
            tricks_won: [0, 0],
            current_trick: Vec::new(),
        }
    }

    /// Limit the permutation range used when populating the database. Providing
    /// a single permutation can dramatically speed up tests.
    pub fn set_perm_range_single(&mut self, idx: usize) {
        self.perm_range = Some(vec![idx]);
    }

    /// Clear any permutation restriction so that all permutations are used
    /// again when populating the database.
    pub fn clear_perm_range(&mut self) {
        self.perm_range = None;
    }

    /// Set the number of worker threads used when populating the database.
    pub fn set_workers(&mut self, workers: usize) {
        self.workers = workers.max(1);
    }

    /// Provide a callback that receives progress updates while the database is
    /// being populated.
    pub fn set_progress_callback(&mut self, cb: Option<Box<dyn Fn(u64)>>) {
        self.progress_cb = cb;
    }

    pub fn start_round(&mut self) {
        self.round_points = ROUND_POINTS;
        self.last_raiser = None;
        let mut cards = deck();
        shuffle(&mut cards);
        for p in self.players.iter_mut() {
            p.hand.clear();
        }
        for i in 0..4 {
            self.played[i].clear();
        }
        for i in 0..TRICKS_PER_ROUND {
            for j in 0..4 {
                let idx = (self.dealer + 1 + j) % 4;
                let c = cards[i * 4 + j];
                self.players[idx].hand.push(c);
                self.orig_hands[idx][i] = c;
            }
        }
        let dealer_card = self.players[self.dealer].hand[0];
        let next_idx = (self.dealer + 1) % 4;
        let next_card = self.players[next_idx].hand[0];
        self.rechte = Some(Card::new(dealer_card.suit, next_card.rank));
        println!("Trump suit is {}", dealer_card.suit);
        println!("Striker rank is {}", next_card.rank);
        println!("Rechte is {}", self.rechte.unwrap());
        self.populate_database();
    }

    pub fn start_round_interactive(&mut self) {
        self.start_round();
        self.playing_round = true;
        self.trick_lead = (self.dealer + 1) % 4;
        self.trick_pos = 0;
        self.tricks_won = [0, 0];
        self.current_trick.clear();
    }

    fn populate_database(&mut self) {
        self.db = Box::new(InMemoryGameDatabase::new());
        let perms = all_hand_orders();
        let rechte = self.rechte.unwrap();
        let indices: Vec<usize> = if let Some(ref r) = self.perm_range {
            r.clone()
        } else {
            (0..perms.len()).collect()
        };

        let workers = self.workers.max(1);
        let total = (indices.len() as u64).pow(4);
        println!(
            "Populating database with {} permutations using {} workers ({} total plays)",
            indices.len(),
            workers,
            total
        );
        let mut cb_opt = self.progress_cb.take();

        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::sync::mpsc::channel;
            use std::thread;

            let (tx, rx) = channel();
            for worker_id in 0..workers {
                println!("Starting worker {}", worker_id);
                let tx = tx.clone();
                let indices = indices.clone();
                let hands = self.orig_hands;
                let dealer = self.dealer;
                let rechte = rechte;
                let perms = perms.clone();
                thread::spawn(move || {
                    let len = indices.len();
                    let total = len.pow(4);
                    for n in (worker_id..total).step_by(workers) {
                        let i1_idx = n / (len * len * len);
                        let i2_idx = (n / (len * len)) % len;
                        let i3_idx = (n / len) % len;
                        let i4_idx = n % len;
                        let i1 = indices[i1_idx];
                        let i2 = indices[i2_idx];
                        let i3 = indices[i3_idx];
                        let i4 = indices[i4_idx];
                        let result = play_hand(&hands, [i1, i2, i3, i4], dealer, rechte, &perms);
                        tx.send((i1, i2, i3, i4, result)).unwrap();
                    }
                });
            }
            drop(tx);
            let mut done = 0u64;
            for (i1, i2, i3, i4, res) in rx {
                self.db.set(i1, i2, i3, i4, res);
                done += 1;
                if let Some(ref cb) = cb_opt {
                    cb(done);
                }
                if done % 1_000_000 == 0 || done == total {
                    println!("Progress: {} / {}", done, total);
                }
                if done == total {
                    break;
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let mut done = 0u64;
            for &i1 in &indices {
                for &i2 in &indices {
                    for &i3 in &indices {
                        for &i4 in &indices {
                            let result = play_hand(
                                &self.orig_hands,
                                [i1, i2, i3, i4],
                                self.dealer,
                                rechte,
                                &perms,
                            );
                            self.db.set(i1, i2, i3, i4, result);
                            done += 1;
                            if let Some(ref cb) = cb_opt {
                                cb(done);
                            }
                            if done % 1_000_000 == 0 || done == total {
                                println!("Progress: {} / {}", done, total);
                            }
                        }
                    }
                }
            }
        }

        self.progress_cb = cb_opt;
    }

    fn find_orig_index(&self, p_idx: usize, card: Card) -> usize {
        for (i, c) in self.orig_hands[p_idx].iter().enumerate() {
            if *c == card && !self.played[p_idx].contains(&i) {
                return i;
            }
        }
        panic!("card not found");
    }

    fn is_seeing_player(&self, idx: usize) -> bool {
        idx == self.dealer || idx == (self.dealer + 1) % 4
    }

    /// Return the indices that the given player is allowed to play
    /// when `lead_card` was led. This enforces the rule that seeing
    /// players must play trump or striker when a trick is started
    /// with a trump card.
    pub fn allowed_indices(&self, p_idx: usize, lead_card: Card) -> Vec<usize> {
        let mut allowed: Vec<usize> = (0..self.players[p_idx].hand.len()).collect();
        if let Some(rechte) = self.rechte {
            if lead_card.suit == rechte.suit && self.is_seeing_player(p_idx) {
                let subset: Vec<usize> = self.players[p_idx]
                    .hand
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.suit == rechte.suit || c.rank == rechte.rank)
                    .map(|(i, _)| i)
                    .collect();
                if !subset.is_empty() {
                    allowed = subset;
                }
            }
        }
        allowed
    }

    pub fn best_card_index(&self, p_idx: usize, allowed: &[usize]) -> usize {
        let player = &self.players[p_idx];
        let playable: Vec<usize> = allowed.to_vec();

        let mut best_idx = playable[0];
        let mut best_rate = -1.0f64;
        for &idx in &playable {
            let card = player.hand[idx];
            let orig = self.find_orig_index(p_idx, card);
            let mut lists: [Vec<usize>; 4] = std::array::from_fn(|_| Vec::new());
            for i in 0..4 {
                let mut prefix = self.played[i].clone();
                if i == p_idx {
                    prefix.push(orig);
                }
                let (s, e) = perm_prefix_range(&prefix);
                if let Some(ref allowed_indices) = self.perm_range {
                    lists[i] = allowed_indices
                        .iter()
                        .cloned()
                        .filter(|&v| v >= s && v < e)
                        .collect();
                } else {
                    lists[i] = (s..e).collect();
                }
            }
            let counts = self
                .db
                .counts_in_lists(&lists[0], &lists[1], &lists[2], &lists[3]);
            let team = p_idx % 2;
            let wins = counts[if team == 0 {
                GameResult::Team1Win as usize
            } else {
                GameResult::Team2Win as usize
            }];
            let losses = counts[if team == 0 {
                GameResult::Team2Win as usize
            } else {
                GameResult::Team1Win as usize
            }];
            let total = wins + losses;
            let rate = if total == 0 {
                0.0
            } else {
                wins as f64 / total as f64
            };
            if rate > best_rate {
                best_rate = rate;
                best_idx = idx;
            }
        }
        best_idx
    }

    fn card_win_rate(&self, p_idx: usize, hand_idx: usize) -> f64 {
        let player = &self.players[p_idx];
        let card = player.hand[hand_idx];
        let orig = self.find_orig_index(p_idx, card);
        let mut lists: [Vec<usize>; 4] = std::array::from_fn(|_| Vec::new());
        for i in 0..4 {
            let mut prefix = self.played[i].clone();
            if i == p_idx {
                prefix.push(orig);
            }
            let (s, e) = perm_prefix_range(&prefix);
            if let Some(ref allowed_indices) = self.perm_range {
                lists[i] = allowed_indices
                    .iter()
                    .cloned()
                    .filter(|&v| v >= s && v < e)
                    .collect();
            } else {
                lists[i] = (s..e).collect();
            }
        }
        let counts = self
            .db
            .counts_in_lists(&lists[0], &lists[1], &lists[2], &lists[3]);
        let team = p_idx % 2;
        let wins = counts[if team == 0 {
            GameResult::Team1Win as usize
        } else {
            GameResult::Team2Win as usize
        }];
        let losses = counts[if team == 0 {
            GameResult::Team2Win as usize
        } else {
            GameResult::Team1Win as usize
        }];
        let total = wins + losses;
        if total == 0 {
            0.0
        } else {
            wins as f64 / total as f64
        }
    }

    fn win_rates_for_player(&self, p_idx: usize) -> Vec<f64> {
        (0..self.players[p_idx].hand.len())
            .map(|i| self.card_win_rate(p_idx, i))
            .collect()
    }

    pub fn trump_suit(&self) -> Option<Suit> {
        self.rechte.map(|c| c.suit)
    }

    pub fn striker_rank(&self) -> Option<Rank> {
        self.rechte.map(|c| c.rank)
    }

    /// Increase the value of the current round by one point on behalf of the
    /// given team (`0` for team 1, `1` for team 2`). The same team may not
    /// raise twice in a row. Returns `Ok(())` if the raise was accepted.
    pub fn raise_round(&mut self, team: usize) -> Result<(), &'static str> {
        if team > 1 {
            return Err("invalid team");
        }
        if self.last_raiser == Some(team) {
            return Err("team already raised");
        }
        self.round_points += 1;
        self.last_raiser = Some(team);
        Ok(())
    }

    #[allow(unused_assignments)]
    pub fn play_round(&mut self) -> GameResult {
        self.start_round();
        let mut tricks = [0usize; 2];
        let mut lead = (self.dealer + 1) % 4;
        for _ in 0..TRICKS_PER_ROUND {
            let mut played: Vec<(usize, Card)> = Vec::new();
            let lead_card = {
                let allowed: Vec<usize> = (0..self.players[lead].hand.len()).collect();
                let card = if self.players[lead].human {
                    let rates = self.win_rates_for_player(lead);
                    self.players[lead].play_card(&allowed, Some(&rates))
                } else {
                    let idx = self.best_card_index(lead, &allowed);
                    self.players[lead].hand.remove(idx)
                };
                let orig = self.find_orig_index(lead, card);
                self.played[lead].push(orig);
                println!("Player {} plays {}", lead + 1, card);
                played.push((lead, card));
                card
            };
            let lead_suit = lead_card.suit;
            for offset in 1..4 {
                let p_idx = (lead + offset) % 4;
                let allowed = self.allowed_indices(p_idx, lead_card);
                let card = if self.players[p_idx].human {
                    let rates = self.win_rates_for_player(p_idx);
                    self.players[p_idx].play_card(&allowed, Some(&rates))
                } else {
                    let idx = self.best_card_index(p_idx, &allowed);
                    self.players[p_idx].hand.remove(idx)
                };
                let orig = self.find_orig_index(p_idx, card);
                self.played[p_idx].push(orig);
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
        let result = if tricks[0] > tricks[1] {
            GameResult::Team1Win
        } else {
            GameResult::Team2Win
        };
        match result {
            GameResult::Team1Win => self.scores[0] += self.round_points,
            GameResult::Team2Win => self.scores[1] += self.round_points,
            _ => {}
        }
        result
    }

    #[allow(unused_assignments)]
    pub fn play_round_logged(&mut self) -> (GameResult, Vec<RoundStep>) {
        self.start_round();
        let mut log = Vec::new();
        let mut tricks = [0usize; 2];
        let mut lead = (self.dealer + 1) % 4;
        for _ in 0..TRICKS_PER_ROUND {
            let mut played: Vec<(usize, Card)> = Vec::new();
            let lead_allowed: Vec<usize> = (0..self.players[lead].hand.len()).collect();
            let lead_hand = self.players[lead].hand.clone();
            let lead_card = {
                let card = if self.players[lead].human {
                    let rates = self.win_rates_for_player(lead);
                    self.players[lead].play_card(&lead_allowed, Some(&rates))
                } else {
                    let idx = self.best_card_index(lead, &lead_allowed);
                    self.players[lead].hand.remove(idx)
                };
                let orig = self.find_orig_index(lead, card);
                self.played[lead].push(orig);
                log.push(RoundStep {
                    player: lead,
                    hand: lead_hand,
                    allowed: lead_allowed.clone(),
                    played: card,
                });
                played.push((lead, card));
                card
            };
            let lead_suit = lead_card.suit;
            for offset in 1..4 {
                let p_idx = (lead + offset) % 4;
                let allowed = self.allowed_indices(p_idx, lead_card);
                let hand_before = self.players[p_idx].hand.clone();
                let card = if self.players[p_idx].human {
                    let rates = self.win_rates_for_player(p_idx);
                    self.players[p_idx].play_card(&allowed, Some(&rates))
                } else {
                    let idx = self.best_card_index(p_idx, &allowed);
                    self.players[p_idx].hand.remove(idx)
                };
                let orig = self.find_orig_index(p_idx, card);
                self.played[p_idx].push(orig);
                log.push(RoundStep {
                    player: p_idx,
                    hand: hand_before,
                    allowed: allowed.clone(),
                    played: card,
                });
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
            tricks[winner_idx % 2] += 1;
            lead = winner_idx;
        }
        self.dealer = (self.dealer + 1) % 4;
        let result = if tricks[0] > tricks[1] {
            GameResult::Team1Win
        } else {
            GameResult::Team2Win
        };
        match result {
            GameResult::Team1Win => self.scores[0] += self.round_points,
            GameResult::Team2Win => self.scores[1] += self.round_points,
            _ => {}
        }
        (result, log)
    }

    fn current_player(&self) -> usize {
        (self.trick_lead + self.trick_pos) % 4
    }

    fn current_allowed(&self) -> Vec<usize> {
        let p = self.current_player();
        if self.trick_pos == 0 {
            (0..self.players[p].hand.len()).collect()
        } else {
            let lead_card = self.current_trick[0].1;
            self.allowed_indices(p, lead_card)
        }
    }

    fn play_internal(&mut self, p_idx: usize, idx: usize, record: &mut Vec<RoundStep>) {
        let hand_before = self.players[p_idx].hand.clone();
        let allowed = if self.trick_pos == 0 {
            (0..hand_before.len()).collect()
        } else {
            let lead_card = self.current_trick[0].1;
            self.allowed_indices(p_idx, lead_card)
        };
        let card = self.players[p_idx].hand.remove(idx);
        let orig = self.find_orig_index(p_idx, card);
        self.played[p_idx].push(orig);
        record.push(RoundStep {
            player: p_idx,
            hand: hand_before,
            allowed: allowed.clone(),
            played: card,
        });
        self.current_trick.push((p_idx, card));
        self.trick_pos += 1;
        if self.trick_pos == 4 {
            self.finish_trick(record);
        }
    }

    fn finish_trick(&mut self, record: &mut Vec<RoundStep>) {
        let rechte = self.rechte.unwrap();
        let lead_suit = self.current_trick[0].1.suit;
        let mut best = (self.current_trick[0].0, self.current_trick[0].1, 0usize);
        let mut best_score = card_strength(&best.1, lead_suit, rechte, 0);
        for (pos, &(idx, ref card)) in self.current_trick.iter().enumerate().skip(1) {
            let val = card_strength(card, lead_suit, rechte, pos);
            if val > best_score {
                best = (idx, *card, pos);
                best_score = val;
            }
        }
        let winner_idx = best.0;
        self.tricks_won[winner_idx % 2] += 1;
        self.trick_lead = winner_idx;
        self.trick_pos = 0;
        self.current_trick.clear();
        if self.tricks_won[0] + self.tricks_won[1] == TRICKS_PER_ROUND {
            self.finish_round();
        }
    }

    fn finish_round(&mut self) {
        self.playing_round = false;
        self.dealer = (self.dealer + 1) % 4;
        let result = if self.tricks_won[0] > self.tricks_won[1] {
            GameResult::Team1Win
        } else {
            GameResult::Team2Win
        };
        match result {
            GameResult::Team1Win => self.scores[0] += self.round_points,
            GameResult::Team2Win => self.scores[1] += self.round_points,
            _ => {}
        }
    }

    fn advance_bots_internal(&mut self, record: &mut Vec<RoundStep>) -> Option<GameResult> {
        while self.playing_round {
            let p = self.current_player();
            if self.players[p].human {
                return None;
            }
            let allowed = self.current_allowed();
            let idx = self.best_card_index(p, &allowed);
            self.play_internal(p, idx, record);
            if !self.playing_round {
                return Some(if self.tricks_won[0] > self.tricks_won[1] {
                    GameResult::Team1Win
                } else {
                    GameResult::Team2Win
                });
            }
        }
        Some(if self.tricks_won[0] > self.tricks_won[1] {
            GameResult::Team1Win
        } else {
            GameResult::Team2Win
        })
    }

    pub fn advance_bots(&mut self) -> (Option<GameResult>, Vec<RoundStep>) {
        let mut log = Vec::new();
        let result = self.advance_bots_internal(&mut log);
        (result, log)
    }

    pub fn human_allowed_indices(&self) -> Vec<usize> {
        self.current_allowed()
    }

    pub fn human_play(&mut self, idx: usize) -> (Option<GameResult>, Vec<RoundStep>) {
        let mut log = Vec::new();
        let p = self.current_player();
        self.play_internal(p, idx, &mut log);
        if self.playing_round {
            if let Some(res) = self.advance_bots_internal(&mut log) {
                return (Some(res), log);
            }
        } else {
            return (
                Some(if self.tricks_won[0] > self.tricks_won[1] {
                    GameResult::Team1Win
                } else {
                    GameResult::Team2Win
                }),
                log,
            );
        }
        (None, log)
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
            card_strength(&striker, lead, rechte, 0) > card_strength(&trump_card, lead, rechte, 0)
        );
    }

    #[test]
    fn rechte_beats_striker() {
        let rechte = Card::new(Suit::Leaves, Rank::Ober);
        let striker = Card::new(Suit::Hearts, Rank::Ober);
        let lead = Suit::Hearts;
        assert!(card_strength(&rechte, lead, rechte, 0) > card_strength(&striker, lead, rechte, 0));
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
        assert_eq!(g.round_points, ROUND_POINTS);
    }

    #[test]
    fn teams_can_raise_alternating() {
        let mut g = GameState::new(0);
        assert_eq!(g.round_points, ROUND_POINTS);
        assert!(g.raise_round(0).is_ok());
        assert_eq!(g.round_points, ROUND_POINTS + 1);
        assert!(g.raise_round(0).is_err());
        assert!(g.raise_round(1).is_ok());
        assert_eq!(g.round_points, ROUND_POINTS + 2);
        assert!(g.raise_round(1).is_err());
        assert!(g.raise_round(0).is_ok());
        assert_eq!(g.round_points, ROUND_POINTS + 3);
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

        let lead_card = players[0].play_card(&[0], None);
        let lead_suit = lead_card.suit;
        let mut cards = vec![lead_card];
        for i in 1..4 {
            let allowed: Vec<usize> = (0..players[i].hand.len()).collect();
            cards.push(players[i].play_card(&allowed, None));
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

        let lead_card = players[0].play_card(&[0], None);
        let lead_suit = lead_card.suit;
        let mut cards = vec![lead_card];
        for i in 1..4 {
            let allowed: Vec<usize> = (0..players[i].hand.len()).collect();
            cards.push(players[i].play_card(&allowed, None));
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

    #[test]
    fn play_hand_by_ids_matches_simulation() {
        let mut deck = deck();
        let mut hands = [[DUMMY_CARD; TRICKS_PER_ROUND]; 4];
        for i in 0..TRICKS_PER_ROUND {
            for j in 0..4 {
                hands[j][i] = deck[i * 4 + j];
            }
        }
        let rechte = Card::new(hands[0][0].suit, hands[1][0].rank);
        let ids = [0usize; 4];
        let expect = simulate_game(&hands, [[0, 1, 2, 3, 4]; 4], 0, rechte);
        let perms = all_hand_orders();
        let result = play_hand(&hands, ids, 0, rechte, &perms);
        assert_eq!(expect, result);
    }

    #[test]
    fn allowed_indices_for_seeing_players() {
        use Rank::*;
        use Suit::*;
        let mut g = GameState::new(0);
        g.dealer = 0;
        g.rechte = Some(Card::new(Hearts, Unter));

        g.players[0].hand = vec![Card::new(Hearts, Ace), Card::new(Bells, Ober)];
        g.players[1].hand = vec![Card::new(Leaves, Unter), Card::new(Acorns, Seven)];
        g.players[2].hand = vec![Card::new(Hearts, Ten)];
        g.players[3].hand = vec![Card::new(Bells, Ace)];

        let lead = Card::new(Hearts, Ten);
        let a0 = g.allowed_indices(0, lead.clone());
        assert_eq!(a0, vec![0]);
        let a1 = g.allowed_indices(1, lead);
        assert_eq!(a1, vec![0]);
    }

    #[test]
    fn start_round_populates_database() {
        let mut g = GameState::new(0);
        g.perm_range = Some(vec![0]);
        g.start_round();
        // database should contain at least the entry for all-zero permutations
        assert!(g.db.get(0, 0, 0, 0) != GameResult::NotPlayed);
    }

    #[test]
    fn best_card_uses_database() {
        use std::cell::RefCell;
        use std::rc::Rc;

        struct DummyDB {
            calls: Rc<RefCell<u32>>,
        }

        impl GameDatabase for DummyDB {
            fn set(&mut self, _p1: usize, _p2: usize, _p3: usize, _p4: usize, _r: GameResult) {}
            fn get(&self, _p1: usize, _p2: usize, _p3: usize, _p4: usize) -> GameResult {
                GameResult::NotPlayed
            }
            fn counts_in_ranges(
                &self,
                _p1: std::ops::Range<usize>,
                _p2: std::ops::Range<usize>,
                _p3: std::ops::Range<usize>,
                _p4: std::ops::Range<usize>,
            ) -> [u32; 4] {
                let mut c = self.calls.borrow_mut();
                let result = if *c == 0 {
                    [0, 10, 0, 0]
                } else {
                    [0, 0, 10, 0]
                };
                *c += 1;
                result
            }

            fn counts_in_lists(
                &self,
                _p1: &[usize],
                _p2: &[usize],
                _p3: &[usize],
                _p4: &[usize],
            ) -> [u32; 4] {
                self.counts_in_ranges(0..0, 0..0, 0..0, 0..0)
            }
        }

        let counter = Rc::new(RefCell::new(0));
        let mut g = GameState::new(0);
        g.db = Box::new(DummyDB {
            calls: counter.clone(),
        });

        g.players[0].hand = vec![
            Card::new(Suit::Hearts, Rank::Seven),
            Card::new(Suit::Bells, Rank::Ace),
        ];
        g.orig_hands[0][0] = g.players[0].hand[0];
        g.orig_hands[0][1] = g.players[0].hand[1];

        let idx = g.best_card_index(0, &[0, 1]);
        assert_eq!(idx, 0);
        assert!(*counter.borrow() >= 2);
    }

    #[test]
    fn card_win_rate_uses_database() {
        use std::cell::RefCell;
        use std::rc::Rc;

        struct DummyDB {
            calls: Rc<RefCell<u32>>,
        }

        impl GameDatabase for DummyDB {
            fn set(&mut self, _p1: usize, _p2: usize, _p3: usize, _p4: usize, _r: GameResult) {}
            fn get(&self, _p1: usize, _p2: usize, _p3: usize, _p4: usize) -> GameResult {
                GameResult::NotPlayed
            }
            fn counts_in_ranges(
                &self,
                _p1: std::ops::Range<usize>,
                _p2: std::ops::Range<usize>,
                _p3: std::ops::Range<usize>,
                _p4: std::ops::Range<usize>,
            ) -> [u32; 4] {
                *self.calls.borrow_mut() += 1;
                [0, 5, 10, 0]
            }

            fn counts_in_lists(
                &self,
                _p1: &[usize],
                _p2: &[usize],
                _p3: &[usize],
                _p4: &[usize],
            ) -> [u32; 4] {
                self.counts_in_ranges(0..0, 0..0, 0..0, 0..0)
            }
        }

        let counter = Rc::new(RefCell::new(0));
        let mut g = GameState::new(0);
        g.db = Box::new(DummyDB {
            calls: counter.clone(),
        });

        g.players[0].hand = vec![Card::new(Suit::Hearts, Rank::Seven)];
        g.orig_hands[0][0] = g.players[0].hand[0];

        let rate = g.card_win_rate(0, 0);
        assert!(*counter.borrow() > 0);
        assert!((rate - 0.3333).abs() < 1e-4);
    }
}

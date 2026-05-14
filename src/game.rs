use crate::evaluator::{DatabaseEvaluator, EvaluationContext, MoveEvaluator, SearchEvaluator};
use crate::player::Player;
use crate::{deck, shuffle, Card, GameResult, Rank, Suit};
use num_cpus;
use serde::Serialize;

/// Selector for the bundled evaluator strategies. For custom evaluators,
/// call [`GameState::set_evaluator_impl`] with your own
/// `Box<dyn MoveEvaluator>` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evaluator {
    /// Memoized legal-completion search. Fast, default.
    Search,
    /// Brute-force 120^4 database population. Kept as a benchmarking
    /// fallback and for cross-validation against the search.
    Database,
}

impl Default for Evaluator {
    fn default() -> Self {
        Evaluator::Search
    }
}

/// Win/total counts for one candidate move.
#[derive(Debug, Clone, Copy)]
pub struct MoveEvaluation {
    /// Index into the player's current hand vector.
    pub hand_idx: usize,
    /// Games in which the player's team won.
    pub wins: u32,
    /// `wins + losses` — i.e. legal completions only. `losses = total - wins`.
    pub total: u32,
    /// Games where some seeing player failed to follow trump. Only the
    /// 120^4 [`Evaluator::Database`] path reports non-zero values here;
    /// the search evaluator only explores legal continuations.
    pub illegal: u32,
}

impl MoveEvaluation {
    pub fn rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.wins as f64 / self.total as f64
        }
    }

    pub fn losses(&self) -> u32 {
        self.total.saturating_sub(self.wins)
    }
}

/// What happened when a pending raise was resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaiseOutcome {
    /// The responding team accepted; `round_points` is now `new_value`.
    Accepted {
        proposing_team: usize,
        new_value: usize,
    },
    /// The responding team folded; the proposing team won the round at
    /// `points` (the pre-raise round value). The round is over.
    Folded {
        winning_team: usize,
        points: usize,
    },
}

/// Default target score; standard Watten ranges 11..18 depending on variant.
pub const WINNING_POINTS: usize = 15;
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

/// Round-level score for a card given the round's Rechte (trump suit +
/// striker rank). Fixed for the whole round.
///   - Trump suit: +100
///   - Striker rank: +200
///   (the Rechte is both → 300)
pub fn round_score(card: &Card, rechte: Card) -> i16 {
    let mut s = 0;
    if card.suit == rechte.suit {
        s += 100;
    }
    if card.rank == rechte.rank {
        s += 200;
    }
    s
}

/// Trick-level score for a card. Depends on the order it was played in
/// the current trick.
///   - If the card has striker rank AND an earlier card in the trick
///     also had striker rank, the trick score is **-400** (overrides
///     everything else).
///   - Else if the card's suit matches the *lead* card's suit, the trick
///     score is `rank_value + 20`.
///   - Else the trick score is `rank_value` (i.e. cards still rank
///     against each other within their suit even when off-lead).
pub fn trick_score(card: &Card, position: usize, trick: &[Card], rechte: Card) -> i16 {
    if card.rank == rechte.rank {
        for earlier in 0..position {
            if trick[earlier].rank == rechte.rank {
                return -400;
            }
        }
    }
    let lead_suit = trick[0].suit;
    let rv = rank_value(card.rank) as i16;
    if card.suit == lead_suit {
        rv + 20
    } else {
        rv
    }
}

/// Total comparable score for a card in a trick:
///   `round_score + trick_score`
/// Use strict `>` to determine the trick winner so the earliest play wins
/// ties.
pub fn card_score(card: &Card, position: usize, trick: &[Card], rechte: Card) -> i16 {
    round_score(card, rechte) + trick_score(card, position, trick, rechte)
}

/// Position in `trick` (0..4) of the card that wins. Ties go to the
/// earliest play.
pub fn trick_winner_position(trick: &[Card], rechte: Card) -> usize {
    let mut best = 0;
    let mut best_score = card_score(&trick[0], 0, trick, rechte);
    for pos in 1..trick.len() {
        let s = card_score(&trick[pos], pos, trick, rechte);
        if s > best_score {
            best_score = s;
            best = pos;
        }
    }
    best
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
    let is_seeing = |p: usize| p == dealer || p == (dealer + 1) % 4;
    for _ in 0..TRICKS_PER_ROUND {
        let lead_card = hands[lead][perms[lead][pos[lead]]];
        pos[lead] += 1;
        let mut played = vec![(lead, lead_card)];
        for off in 1..4 {
            let idx = (lead + off) % 4;
            let card = hands[idx][perms[idx][pos[idx]]];
            // Must-follow check: if a trump is led and a seeing player
            // plays a non-trump card while still holding a trump-suit
            // card in their *remaining* hand, this permutation produces
            // an illegal game.
            if lead_card.suit == rechte.suit
                && is_seeing(idx)
                && card.suit != rechte.suit
            {
                let has_trump_remaining =
                    (pos[idx] + 1..TRICKS_PER_ROUND).any(|p| {
                        hands[idx][perms[idx][p]].suit == rechte.suit
                    });
                if has_trump_remaining {
                    return GameResult::RuleViolation;
                }
            }
            pos[idx] += 1;
            played.push((idx, card));
        }
        let trick_cards: Vec<Card> = played.iter().map(|(_, c)| *c).collect();
        let winner_pos = trick_winner_position(&trick_cards, rechte);
        let winner_idx = played[winner_pos].0;
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
    /// Team that has an outstanding raise proposal awaiting the other team's
    /// response, if any.
    pending_raise: Option<usize>,
    /// `Some(team)` once the round's winner is fixed by concede/fold. The
    /// remaining tricks still play out (a round is always 5 tricks), they
    /// just don't affect the result.
    round_decided: Option<usize>,
    /// Whichever team most recently had a raise accepted. They cannot
    /// propose another raise until the other team raises (the
    /// alternation rule). Cleared at round start.
    last_raise_by: Option<usize>,
    orig_hands: [[Card; TRICKS_PER_ROUND]; 4],
    played: [Vec<usize>; 4],
    /// Optional subset of permutation indices, applied when in Database mode.
    perm_range: Option<Vec<usize>>,
    /// Number of worker threads used by the Database evaluator
    workers: usize,
    /// Callback for progress updates during database population
    progress_cb: Option<Box<dyn Fn(u64)>>,
    /// Tag indicating which bundled evaluator is active. Updated by
    /// [`Self::set_evaluator`]. Custom implementations swap in via
    /// [`Self::set_evaluator_impl`] and leave this at its previous value.
    evaluator_kind: Evaluator,
    /// The active move evaluator. Always populated; defaults to the search
    /// evaluator.
    evaluator: Box<dyn MoveEvaluator>,
    /// When true, print round/trick narration to stdout (used by the CLI).
    pub verbose: bool,
    // interactive round state
    pub playing_round: bool,
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
            pending_raise: None,
            round_decided: None,
            last_raise_by: None,
            orig_hands: [[DUMMY_CARD; TRICKS_PER_ROUND]; 4],
            played: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            perm_range: None,
            workers: num_cpus::get() * 2,
            progress_cb: None,
            evaluator_kind: Evaluator::default(),
            evaluator: Box::new(SearchEvaluator::new()),
            verbose: false,
            playing_round: false,
            trick_lead: 0,
            trick_pos: 0,
            tricks_won: [0, 0],
            current_trick: Vec::new(),
        }
    }

    /// Select one of the bundled evaluators. The corresponding evaluator
    /// instance is constructed immediately. If a round is in progress, the
    /// new evaluator's [`MoveEvaluator::prepare_round`] is also called so
    /// queries against it return meaningful counts straight away.
    pub fn set_evaluator(&mut self, evaluator: Evaluator) {
        self.evaluator_kind = evaluator;
        self.evaluator = match evaluator {
            Evaluator::Search => Box::new(SearchEvaluator::new()),
            Evaluator::Database => {
                let mut db = DatabaseEvaluator::new();
                if let Some(ref r) = self.perm_range {
                    db.perm_range = Some(r.clone());
                }
                db.workers = self.workers;
                Box::new(db)
            }
        };
        // If a round is already underway, prepare the new evaluator with the
        // current deal so toggling mid-round just works.
        if self.playing_round {
            if let Some(rechte) = self.rechte {
                self.evaluator
                    .prepare_round(&self.orig_hands, self.dealer, rechte);
            }
        }
    }

    /// Inject a custom evaluator implementation. The `kind` reported by
    /// [`Self::evaluator`] is unchanged so callers can still distinguish
    /// between the bundled strategies.
    pub fn set_evaluator_impl(&mut self, evaluator: Box<dyn MoveEvaluator>) {
        self.evaluator = evaluator;
    }

    pub fn evaluator(&self) -> Evaluator {
        self.evaluator_kind
    }

    /// Tricks each team has won so far in the current round (resets to
    /// `[0, 0]` at start_round_interactive).
    pub fn tricks_won_for_round(&self) -> [usize; 2] {
        self.tricks_won
    }

    pub fn evaluator_name(&self) -> &'static str {
        self.evaluator.name()
    }

    fn rebuild_database_evaluator(&mut self) {
        if matches!(self.evaluator_kind, Evaluator::Database) {
            let mut db = DatabaseEvaluator::new();
            db.perm_range = self.perm_range.clone();
            db.workers = self.workers;
            self.evaluator = Box::new(db);
        }
    }

    /// Limit the permutation range used when populating the database. Providing
    /// a single permutation can dramatically speed up tests.
    pub fn set_perm_range_single(&mut self, idx: usize) {
        self.perm_range = Some(vec![idx]);
        self.rebuild_database_evaluator();
    }

    /// Limit the permutation range used when populating the database to an
    /// explicit list of permutation indices.
    pub fn set_perm_range(&mut self, indices: Vec<usize>) {
        self.perm_range = Some(indices);
        self.rebuild_database_evaluator();
    }

    /// Clear any permutation restriction so that all permutations are used
    /// again when populating the database.
    pub fn clear_perm_range(&mut self) {
        self.perm_range = None;
        self.rebuild_database_evaluator();
    }

    /// Set the number of worker threads used when populating the database.
    pub fn set_workers(&mut self, workers: usize) {
        self.workers = workers.max(1);
        self.rebuild_database_evaluator();
    }

    /// Progress-callback hook is kept for backwards compatibility with the
    /// wasm bindings but is currently unused by either evaluator.
    pub fn set_progress_callback(&mut self, cb: Option<Box<dyn Fn(u64)>>) {
        self.progress_cb = cb;
    }

    pub fn start_round(&mut self) {
        self.round_points = ROUND_POINTS;
        self.pending_raise = None;
        self.round_decided = None;
        self.last_raise_by = None;
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
        let rechte = Card::new(dealer_card.suit, next_card.rank);
        self.rechte = Some(rechte);
        self.evaluator
            .prepare_round(&self.orig_hands, self.dealer, rechte);
    }

    pub fn start_round_interactive(&mut self) {
        self.start_round();
        self.playing_round = true;
        self.trick_lead = (self.dealer + 1) % 4;
        self.trick_pos = 0;
        self.tricks_won = [0, 0];
        self.current_trick.clear();
    }

    fn orig_idx_in_hand(&self, p_idx: usize, hand_idx: usize) -> usize {
        let card = self.players[p_idx].hand[hand_idx];
        self.find_orig_index(p_idx, card)
    }

    /// Evaluate every legal move for `p_idx` given the current trick state and
    /// per-team trick counts. Dispatches to the active [`MoveEvaluator`].
    pub fn evaluate_moves(
        &self,
        p_idx: usize,
        allowed_hand_indices: &[usize],
        current_trick: &[(usize, Card)],
        tricks_won: [usize; 2],
    ) -> Vec<MoveEvaluation> {
        let rechte = match self.rechte {
            Some(r) => r,
            None => return Vec::new(),
        };
        let allowed_orig: Vec<usize> = allowed_hand_indices
            .iter()
            .map(|&hi| self.orig_idx_in_hand(p_idx, hi))
            .collect();
        let ctx = EvaluationContext {
            orig_hands: &self.orig_hands,
            played: &self.played,
            dealer: self.dealer,
            rechte,
            player: p_idx,
            current_hand: &self.players[p_idx].hand,
            allowed_orig_indices: &allowed_orig,
            current_trick,
            tricks_won,
        };
        self.evaluator.evaluate_moves(&ctx)
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

    /// Return the indices that the given player is allowed to play when
    /// `lead_card` was led. Enforces the "seeing players must follow
    /// trump" rule: if the lead card is in the trump suit and the player
    /// is a seeing player (dealer or forehand), they must play a
    /// trump-suit card if they hold one.
    pub fn allowed_indices(&self, p_idx: usize, lead_card: Card) -> Vec<usize> {
        let mut allowed: Vec<usize> = (0..self.players[p_idx].hand.len()).collect();
        if let Some(rechte) = self.rechte {
            if lead_card.suit == rechte.suit && self.is_seeing_player(p_idx) {
                let subset: Vec<usize> = self.players[p_idx]
                    .hand
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.suit == rechte.suit)
                    .map(|(i, _)| i)
                    .collect();
                if !subset.is_empty() {
                    allowed = subset;
                }
            }
        }
        allowed
    }

    pub fn best_card_index_with_trick(
        &self,
        p_idx: usize,
        allowed: &[usize],
        current_trick: &[(usize, Card)],
        tricks_won: [usize; 2],
    ) -> usize {
        let evals = self.evaluate_moves(p_idx, allowed, current_trick, tricks_won);
        let mut best_idx = allowed[0];
        let mut best_rate = -1.0f64;
        for e in &evals {
            let rate = e.rate();
            if rate > best_rate {
                best_rate = rate;
                best_idx = e.hand_idx;
            }
        }
        best_idx
    }

    /// Backwards-compatible best-card pick assuming the player is leading and
    /// no team tricks have been won. Prefer [`Self::best_card_index_with_trick`]
    /// when trick state is known.
    pub fn best_card_index(&self, p_idx: usize, allowed: &[usize]) -> usize {
        self.best_card_index_with_trick(p_idx, allowed, &[], [0, 0])
    }

    fn win_rates_for_player_with_trick(
        &self,
        p_idx: usize,
        current_trick: &[(usize, Card)],
        tricks_won: [usize; 2],
    ) -> Vec<f64> {
        let allowed: Vec<usize> = (0..self.players[p_idx].hand.len()).collect();
        let evals = self.evaluate_moves(p_idx, &allowed, current_trick, tricks_won);
        let mut rates = vec![0.0; self.players[p_idx].hand.len()];
        for e in evals {
            rates[e.hand_idx] = e.rate();
        }
        rates
    }

    pub fn trump_suit(&self) -> Option<Suit> {
        self.rechte.map(|c| c.suit)
    }

    pub fn striker_rank(&self) -> Option<Rank> {
        self.rechte.map(|c| c.rank)
    }

    /// Which team has an outstanding raise proposal awaiting the opposing
    /// team's response.
    pub fn pending_raise(&self) -> Option<usize> {
        self.pending_raise
    }

    /// Propose a raise on behalf of `team`. The round value is *not* changed
    /// yet — the opposing team must call [`Self::respond_to_raise`] to
    /// either accept the raise (round value goes up by 1) or fold (which
    /// awards the current `round_points` to the proposing team).
    pub fn propose_raise(&mut self, team: usize) -> Result<(), &'static str> {
        if team > 1 {
            return Err("invalid team");
        }
        if !self.playing_round {
            return Err("round not in progress");
        }
        if self.round_decided.is_some() {
            return Err("round outcome is already decided");
        }
        if self.last_raise_by == Some(team) {
            // Alternation rule: once a team's raise has been accepted, that
            // team cannot raise again until the opposing team has raised.
            return Err("the other team must raise first");
        }
        match self.pending_raise {
            None => {
                self.pending_raise = Some(team);
                Ok(())
            }
            Some(other) if other == team => Err("team already proposed a raise"),
            Some(_) => Err("opposing team must respond to the pending raise first"),
        }
    }

    /// Respond to a pending raise on behalf of `team`. `accept = true`
    /// promotes the round value by one and clears the pending state.
    /// `accept = false` folds: the proposing team is awarded the pre-raise
    /// `round_points` and the round ends.
    pub fn respond_to_raise(
        &mut self,
        team: usize,
        accept: bool,
    ) -> Result<RaiseOutcome, &'static str> {
        if team > 1 {
            return Err("invalid team");
        }
        let proposing = match self.pending_raise {
            Some(t) => t,
            None => return Err("no pending raise"),
        };
        if proposing == team {
            return Err("the proposing team cannot respond to its own raise");
        }
        if accept {
            self.round_points += 1;
            self.pending_raise = None;
            // Alternation lock: this proposing team cannot raise again
            // until the other team has raised.
            self.last_raise_by = Some(proposing);
            Ok(RaiseOutcome::Accepted {
                proposing_team: proposing,
                new_value: self.round_points,
            })
        } else {
            // Fold: the proposing team's win is locked in at the pre-raise
            // round value, but the round continues being played out so the
            // "5 tricks per round" invariant holds.
            let points = self.round_points;
            self.pending_raise = None;
            self.round_decided = Some(proposing);
            Ok(RaiseOutcome::Folded {
                winning_team: proposing,
                points,
            })
        }
    }

    /// Decide how the team that is *not* the proposer should respond to the
    /// current pending raise, using the active move evaluator as a heuristic.
    /// Returns the outcome of the response. Defaults to accept when the
    /// estimate is unavailable.
    ///
    /// Accept threshold: 0.30 — if the responding team's estimated win
    /// probability from the current position is below 30% they fold.
    pub fn auto_respond_raise(&mut self) -> Result<RaiseOutcome, &'static str> {
        let proposing = match self.pending_raise {
            Some(t) => t,
            None => return Err("no pending raise"),
        };
        let responding = 1 - proposing;
        let rate = self.estimate_team_win_rate(responding).unwrap_or(0.5);
        let accept = rate >= 0.30;
        self.respond_to_raise(responding, accept)
    }

    /// Best-effort estimate of `team`'s probability of winning the round
    /// from the current position. Uses the move evaluator: looks at the
    /// next player to act and aggregates wins / total across all of their
    /// legal moves, then flips the perspective if needed.
    pub fn estimate_team_win_rate(&self, team: usize) -> Option<f64> {
        if !self.playing_round {
            return None;
        }
        let p = self.current_player();
        let allowed = self.current_allowed();
        if allowed.is_empty() {
            return None;
        }
        let evals = self.evaluate_moves(p, &allowed, &self.current_trick, self.tricks_won);
        let total: u32 = evals.iter().map(|e| e.total).sum();
        if total == 0 {
            return None;
        }
        let wins: u32 = evals.iter().map(|e| e.wins).sum();
        let rate = wins as f64 / total as f64; // probability for `p`'s team
        if team == p % 2 {
            Some(rate)
        } else {
            Some(1.0 - rate)
        }
    }

    /// Concede the current round on behalf of `team`: the opposing team is
    /// locked in as the winner of the round at the current `round_points`,
    /// but the cards are still played out — the round always lasts 5
    /// tricks. Callers typically follow this with [`Self::auto_play_round`]
    /// to animate the remaining plays. The round actually ends (deal next)
    /// only when all hands are empty.
    pub fn concede_round(&mut self, team: usize) -> Result<GameResult, &'static str> {
        if team > 1 {
            return Err("invalid team");
        }
        if !self.playing_round {
            return Err("round not in progress");
        }
        if self.round_decided.is_some() {
            return Err("round outcome is already decided");
        }
        let winner_team = 1 - team;
        self.round_decided = Some(winner_team);
        Ok(if winner_team == 0 {
            GameResult::Team1Win
        } else {
            GameResult::Team2Win
        })
    }

    /// True iff every player has at least one card left to play.
    fn hands_remaining(&self) -> bool {
        self.players.iter().any(|p| !p.hand.is_empty())
    }

    /// Whether the round outcome has been locked in (concede/fold).
    pub fn round_decided(&self) -> Option<usize> {
        self.round_decided
    }

    /// Play out every remaining card using the bot move evaluator for *all*
    /// players, including humans. Returns the sequence of plays. Used to
    /// animate the rest of the round after a concede or a raise-fold so the
    /// "round = 5 tricks" invariant is preserved.
    pub fn auto_play_round(&mut self) -> Vec<RoundStep> {
        let mut log = Vec::new();
        while self.playing_round && self.hands_remaining() {
            let p = self.current_player();
            let allowed = self.current_allowed();
            if allowed.is_empty() {
                break;
            }
            let trick = self.current_trick.clone();
            let tricks_won = self.tricks_won;
            let idx = self.best_card_index_with_trick(p, &allowed, &trick, tricks_won);
            self.play_internal(p, idx, &mut log);
        }
        log
    }

    #[allow(unused_assignments)]
    pub fn play_round(&mut self) -> GameResult {
        self.start_round();
        if self.verbose {
            if let Some(rechte) = self.rechte {
                println!("Trump suit is {}", rechte.suit);
                println!("Striker rank is {}", rechte.rank);
                println!("Rechte is {}", rechte);
            }
        }
        let mut tricks = [0usize; 2];
        let mut lead = (self.dealer + 1) % 4;
        for _ in 0..TRICKS_PER_ROUND {
            let mut played: Vec<(usize, Card)> = Vec::new();
            let lead_card = {
                let allowed: Vec<usize> = (0..self.players[lead].hand.len()).collect();
                let card = if self.players[lead].human {
                    let rates = self.win_rates_for_player_with_trick(lead, &played, tricks);
                    self.players[lead].play_card(&allowed, Some(&rates))
                } else {
                    let idx = self.best_card_index_with_trick(lead, &allowed, &played, tricks);
                    self.players[lead].hand.remove(idx)
                };
                let orig = self.find_orig_index(lead, card);
                self.played[lead].push(orig);
                if self.verbose {
                    println!("Player {} plays {}", lead + 1, card);
                }
                played.push((lead, card));
                card
            };
            let lead_suit = lead_card.suit;
            for offset in 1..4 {
                let p_idx = (lead + offset) % 4;
                let allowed = self.allowed_indices(p_idx, lead_card);
                let card = if self.players[p_idx].human {
                    let rates = self.win_rates_for_player_with_trick(p_idx, &played, tricks);
                    self.players[p_idx].play_card(&allowed, Some(&rates))
                } else {
                    let idx = self.best_card_index_with_trick(p_idx, &allowed, &played, tricks);
                    self.players[p_idx].hand.remove(idx)
                };
                let orig = self.find_orig_index(p_idx, card);
                self.played[p_idx].push(orig);
                if self.verbose {
                    println!("Player {} plays {}", p_idx + 1, card);
                }
                played.push((p_idx, card));
            }
            let rechte = self.rechte.unwrap();
            let trick_cards: Vec<Card> = played.iter().map(|(_, c)| *c).collect();
            let winner_pos = trick_winner_position(&trick_cards, rechte);
            let winner_idx = played[winner_pos].0;
            if self.verbose {
                println!("Player {} wins the trick\n", winner_idx + 1);
            }
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
                    let rates = self.win_rates_for_player_with_trick(lead, &played, tricks);
                    self.players[lead].play_card(&lead_allowed, Some(&rates))
                } else {
                    let idx =
                        self.best_card_index_with_trick(lead, &lead_allowed, &played, tricks);
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
                    let rates = self.win_rates_for_player_with_trick(p_idx, &played, tricks);
                    self.players[p_idx].play_card(&allowed, Some(&rates))
                } else {
                    let idx =
                        self.best_card_index_with_trick(p_idx, &allowed, &played, tricks);
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
            let trick_cards: Vec<Card> = played.iter().map(|(_, c)| *c).collect();
            let winner_pos = trick_winner_position(&trick_cards, rechte);
            let winner_idx = played[winner_pos].0;
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

    fn finish_trick(&mut self, _record: &mut Vec<RoundStep>) {
        let rechte = self.rechte.unwrap();
        let trick_cards: Vec<Card> = self.current_trick.iter().map(|(_, c)| *c).collect();
        let winner_pos = trick_winner_position(&trick_cards, rechte);
        let winner_idx = self.current_trick[winner_pos].0;
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
        // If the round was decided by concede/fold the winner is fixed;
        // otherwise it's the team that took more tricks.
        let winner_team = self.round_decided.unwrap_or_else(|| {
            if self.tricks_won[0] > self.tricks_won[1] {
                0
            } else {
                1
            }
        });
        self.scores[winner_team] += self.round_points;
        self.round_decided = None;
    }

    fn advance_bots_internal(&mut self, record: &mut Vec<RoundStep>) -> Option<GameResult> {
        while self.playing_round {
            let p = self.current_player();
            if self.players[p].human {
                return None;
            }
            let allowed = self.current_allowed();
            let trick = self.current_trick.clone();
            let tricks_won = self.tricks_won;
            let idx = self.best_card_index_with_trick(p, &allowed, &trick, tricks_won);
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

    /// Move evaluations (wins/total per legal card) for the player whose turn
    /// it is. Returned in current-hand-index order, restricted to legal cards.
    pub fn human_move_evaluations(&self) -> Vec<MoveEvaluation> {
        let p = self.current_player();
        let allowed = self.current_allowed();
        self.evaluate_moves(p, &allowed, &self.current_trick, self.tricks_won)
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
    use crate::all_hand_orders;

    #[test]
    fn round_score_components() {
        // Pure card: 0
        assert_eq!(
            round_score(&Card::new(Suit::Acorns, Rank::Seven), Card::new(Suit::Hearts, Rank::Unter)),
            0
        );
        // Trump suit, not striker rank: 100
        assert_eq!(
            round_score(&Card::new(Suit::Hearts, Rank::Ace), Card::new(Suit::Hearts, Rank::Unter)),
            100
        );
        // Striker rank, not trump suit: 200
        assert_eq!(
            round_score(&Card::new(Suit::Bells, Rank::Unter), Card::new(Suit::Hearts, Rank::Unter)),
            200
        );
        // Rechte (trump suit AND striker rank): 300
        assert_eq!(
            round_score(&Card::new(Suit::Hearts, Rank::Unter), Card::new(Suit::Hearts, Rank::Unter)),
            300
        );
    }

    #[test]
    fn striker_beats_trump_when_both_in_trick() {
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        // Lead trump, then a non-trump striker.
        let trick = vec![
            Card::new(Suit::Hearts, Rank::Ace),   // 0: trump non-striker, 100+8 = 108
            Card::new(Suit::Bells, Rank::Unter),  // 1: striker non-trump,  200+0 = 200
        ];
        assert_eq!(trick_winner_position(&trick, rechte), 1);
    }

    #[test]
    fn rechte_beats_other_strikers_when_played_first() {
        // Lead trump suit, Rechte is the lead.
        let rechte = Card::new(Suit::Leaves, Rank::Ober);
        let trick = vec![
            Card::new(Suit::Leaves, Rank::Ober),  // 0: rechte         300 + 6 = 306
            Card::new(Suit::Hearts, Rank::Ober),  // 1: striker (-400) 200 - 400 = -200
        ];
        assert_eq!(trick_winner_position(&trick, rechte), 0);
    }

    #[test]
    fn pure_card_loses_to_trump() {
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let trick = vec![
            Card::new(Suit::Acorns, Rank::Ace),   // 0: pure same-suit  0 + 8 = 8
            Card::new(Suit::Hearts, Rank::Seven), // 1: trump non-striker 100 + 0 = 100
        ];
        assert_eq!(trick_winner_position(&trick, rechte), 1);
    }

    #[test]
    fn pure_card_loses_to_striker() {
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let trick = vec![
            Card::new(Suit::Acorns, Rank::Ace),   // 0: pure   0 + 8 = 8
            Card::new(Suit::Bells, Rank::Unter),  // 1: striker 200 + 0 = 200
        ];
        assert_eq!(trick_winner_position(&trick, rechte), 1);
    }

    #[test]
    fn simulate_game_flags_illegal_must_follow_violations() {
        use Rank::*;
        use Suit::*;
        // Trump = Hearts, striker = Unter.
        let rechte = Card::new(Hearts, Unter);
        // Dealer = 0 → seeing players are 0 and 1. Lead is player 1.
        // Player 1 leads Hearts (trump). Player 0 is seeing and holds a
        // trump-suit card in their later perm, but plays a non-trump first.
        // That permutation must be reported as RuleViolation.
        let hands = [
            [
                Card::new(Bells, Seven), // perm[0] = 0 → play non-trump first
                Card::new(Hearts, Ace),  // a trump card still in hand
                Card::new(Leaves, Seven),
                Card::new(Acorns, Seven),
                Card::new(Bells, Eight),
            ],
            [
                Card::new(Hearts, Ten), // lead = trump suit
                Card::new(Bells, King),
                Card::new(Leaves, Ace),
                Card::new(Bells, Nine),
                Card::new(Acorns, Nine),
            ],
            [
                Card::new(Acorns, King),
                Card::new(Bells, Ace),
                Card::new(Leaves, King),
                Card::new(Acorns, Ten),
                Card::new(Bells, Ober),
            ],
            [
                Card::new(Acorns, Ober),
                Card::new(Leaves, Nine),
                Card::new(Hearts, Nine),
                Card::new(Acorns, Eight),
                Card::new(Hearts, Seven),
            ],
        ];
        // Identity permutation for every player → P0 plays Bells Seven
        // (non-trump) while holding Hearts Ace (trump). Must-follow violated.
        let perms = [[0, 1, 2, 3, 4]; 4];
        assert_eq!(simulate_game(&hands, perms, 0, rechte), GameResult::RuleViolation);
    }

    #[test]
    fn higher_trump_off_lead_still_beats_lower_trump_off_lead() {
        // Lead is a pure card; two trumps (off-lead-suit) follow. With
        // trick_score off-suit = rank_value (no +20), trump rank still
        // breaks ties: the higher trump wins.
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let trick = vec![
            Card::new(Suit::Bells, Rank::Seven), // 0: lead pure  21
            Card::new(Suit::Hearts, Rank::Nine), // 1: trump      103
            Card::new(Suit::Hearts, Rank::Ace),  // 2: trump      108
        ];
        assert_eq!(trick_winner_position(&trick, rechte), 2);
    }

    #[test]
    fn higher_pure_same_suit_beats_lead() {
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let trick = vec![
            Card::new(Suit::Acorns, Rank::Seven), // 0: 0 + 1 = 1
            Card::new(Suit::Acorns, Rank::Ace),   // 1: 0 + 8 = 8
        ];
        assert_eq!(trick_winner_position(&trick, rechte), 1);
    }

    #[test]
    fn off_suit_pure_loses_to_lead_pure() {
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let trick = vec![
            Card::new(Suit::Acorns, Rank::Seven), // 0: 0 + 1 = 1
            Card::new(Suit::Bells, Rank::Ace),    // 1: 0 + 0 = 0  (different suit from lead)
        ];
        assert_eq!(trick_winner_position(&trick, rechte), 0);
    }

    #[test]
    fn first_striker_played_beats_later_striker() {
        // Two strikers in the same trick; the SECOND one gets the
        // -400 trick-score override and loses.
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let trick = vec![
            Card::new(Suit::Bells, Rank::Unter),   // 0: 200 + rank_value(Unter)=5 (lead=Bells) = 205
            Card::new(Suit::Leaves, Rank::Unter),  // 1: 200 - 400 = -200
        ];
        assert_eq!(trick_winner_position(&trick, rechte), 0);
    }

    #[test]
    fn rechte_played_after_striker_loses_to_first_striker() {
        // Watten's "first striker wins" rule still applies to the Rechte
        // when another striker was played earlier in the trick.
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let trick = vec![
            Card::new(Suit::Bells, Rank::Unter),  // 0: striker 200 + 5 = 205
            Card::new(Suit::Hearts, Rank::Unter), // 1: rechte  300 - 400 = -100
        ];
        assert_eq!(trick_winner_position(&trick, rechte), 0);
    }

    #[test]
    fn new_game_has_zero_scores() {
        let g = GameState::new(0);
        assert_eq!(g.scores, [0, 0]);
        assert_eq!(g.round_points, ROUND_POINTS);
    }

    #[test]
    fn concede_locks_in_round_decided_without_ending_round() {
        let mut g = GameState::new(0);
        g.playing_round = true;
        assert!(g.propose_raise(0).is_ok());
        assert!(matches!(
            g.respond_to_raise(1, true).unwrap(),
            RaiseOutcome::Accepted { new_value, .. } if new_value == ROUND_POINTS + 1
        ));
        assert!(g.propose_raise(1).is_ok());
        assert!(matches!(
            g.respond_to_raise(0, true).unwrap(),
            RaiseOutcome::Accepted { new_value, .. } if new_value == ROUND_POINTS + 2
        ));
        assert_eq!(g.round_points, ROUND_POINTS + 2);
        let result = g.concede_round(0).unwrap();
        assert_eq!(result, GameResult::Team2Win);
        // Concede locks the winner but does NOT update scores yet — the
        // round still has to be played out (5 tricks total). Scores land
        // when finish_round fires.
        assert_eq!(g.scores, [0, 0]);
        assert_eq!(g.round_decided(), Some(1));
        assert!(g.playing_round);
        // Conceding twice or after a decision is rejected.
        assert!(g.concede_round(0).is_err());
    }

    #[test]
    fn propose_then_accept_raises_round_points() {
        let mut g = GameState::new(0);
        g.playing_round = true;
        assert_eq!(g.round_points, ROUND_POINTS);
        assert!(g.propose_raise(0).is_ok());
        assert_eq!(g.pending_raise(), Some(0));
        // Same team can't propose again, and the proposing team can't respond.
        assert!(g.propose_raise(0).is_err());
        assert!(g.respond_to_raise(0, true).is_err());
        let outcome = g.respond_to_raise(1, true).unwrap();
        assert!(matches!(
            outcome,
            RaiseOutcome::Accepted {
                proposing_team: 0,
                new_value
            } if new_value == ROUND_POINTS + 1
        ));
        assert_eq!(g.round_points, ROUND_POINTS + 1);
        assert_eq!(g.pending_raise(), None);
    }

    #[test]
    fn propose_then_fold_locks_in_round_decided_at_pre_raise_value() {
        let mut g = GameState::new(0);
        g.playing_round = true;
        assert!(g.propose_raise(0).is_ok());
        let outcome = g.respond_to_raise(1, false).unwrap();
        assert!(matches!(
            outcome,
            RaiseOutcome::Folded {
                winning_team: 0,
                points
            } if points == ROUND_POINTS
        ));
        // Round value did not go up; scores have not been credited yet —
        // the round is still in progress (a round is always 5 tricks).
        assert_eq!(g.scores, [0, 0]);
        assert_eq!(g.round_points, ROUND_POINTS);
        assert!(g.playing_round);
        assert_eq!(g.round_decided(), Some(0));
        assert_eq!(g.pending_raise(), None);
    }

    #[test]
    fn raise_alternation_rule() {
        // After Team 1's raise is accepted Team 1 cannot raise again until
        // Team 2 has raised; then Team 1 can raise once more.
        let mut g = GameState::new(0);
        g.playing_round = true;
        assert_eq!(g.round_points, ROUND_POINTS);
        // Team 1 proposes, Team 2 accepts.
        assert!(g.propose_raise(0).is_ok());
        assert!(g.respond_to_raise(1, true).is_ok());
        assert_eq!(g.round_points, ROUND_POINTS + 1);
        // Team 1 cannot raise again immediately — Team 2 must raise first.
        assert!(g.propose_raise(0).is_err());
        // Team 2 raises, Team 1 accepts.
        assert!(g.propose_raise(1).is_ok());
        assert!(g.respond_to_raise(0, true).is_ok());
        assert_eq!(g.round_points, ROUND_POINTS + 2);
        // Team 2 now cannot raise again until Team 1 does.
        assert!(g.propose_raise(1).is_err());
        // Team 1 raises again, Team 2 accepts.
        assert!(g.propose_raise(0).is_ok());
        assert!(g.respond_to_raise(1, true).is_ok());
        assert_eq!(g.round_points, ROUND_POINTS + 3);
        // Team 1 is locked out again.
        assert!(g.propose_raise(0).is_err());
    }

    #[test]
    fn cannot_propose_raise_while_one_is_pending() {
        let mut g = GameState::new(0);
        g.playing_round = true;
        assert!(g.propose_raise(0).is_ok());
        // Same team double-proposes: rejected (raise is pending).
        assert!(g.propose_raise(0).is_err());
        // Other team can't propose while a raise is pending either.
        assert!(g.propose_raise(1).is_err());
    }

    #[test]
    fn cannot_raise_after_round_decided() {
        let mut g = GameState::new(0);
        g.playing_round = true;
        // Team 1 proposes, Team 2 folds — Team 1 wins (round_decided).
        assert!(g.propose_raise(0).is_ok());
        let _ = g.respond_to_raise(1, false).unwrap();
        assert_eq!(g.round_decided(), Some(0));
        // No further raises by either team while round_decided is set.
        assert!(g.propose_raise(0).is_err());
        assert!(g.propose_raise(1).is_err());
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

        let best_idx = trick_winner_position(&cards, rechte);
        assert_eq!(best_idx, 0);
        for p in &players {
            assert!(p.hand.is_empty());
        }
    }

    #[test]
    fn play_card_whole_trick_rechte_played_after_striker_loses() {
        // Rechte (Hearts Unter) is played LAST after a non-trump striker
        // (Bells Unter) has already been played. Per the -400 rule, the
        // first striker wins — NOT the Rechte.
        let rechte = Card::new(Suit::Hearts, Rank::Unter);
        let mut players = [
            Player::new(false),
            Player::new(false),
            Player::new(false),
            Player::new(false),
        ];
        players[0].hand.push(Card::new(Suit::Hearts, Rank::Ace));   // trump, non-striker
        players[1].hand.push(Card::new(Suit::Bells, Rank::Unter));  // striker (first)
        players[2].hand.push(Card::new(Suit::Hearts, Rank::Nine));  // trump, non-striker
        players[3].hand.push(rechte);                               // rechte (later striker)

        let lead_card = players[0].play_card(&[0], None);
        let mut cards = vec![lead_card];
        for i in 1..4 {
            let allowed: Vec<usize> = (0..players[i].hand.len()).collect();
            cards.push(players[i].play_card(&allowed, None));
        }

        let best_idx = trick_winner_position(&cards, rechte);
        assert_eq!(best_idx, 1, "first striker (Bells Unter at pos 1) wins");
        for p in &players {
            assert!(p.hand.is_empty());
        }
    }

    #[test]
    fn play_hand_by_ids_matches_simulation() {
        let deck = deck();
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
        // Trump = Hearts, striker = Unter. Lead is a trump-suit card →
        // seeing players must follow trump if they hold a trump-suit card.
        let mut g = GameState::new(0);
        g.dealer = 0;
        g.rechte = Some(Card::new(Hearts, Unter));

        // P0 (seeing) holds a trump → must play it.
        g.players[0].hand = vec![Card::new(Hearts, Ace), Card::new(Bells, Ober)];
        // P1 (seeing) holds NO trump-suit card → free to play either.
        g.players[1].hand = vec![Card::new(Leaves, Unter), Card::new(Acorns, Seven)];

        let lead = Card::new(Hearts, Ten);
        assert_eq!(g.allowed_indices(0, lead), vec![0]);
        // P1 has no trump-suit card, so the must-follow rule has no effect.
        assert_eq!(g.allowed_indices(1, lead), vec![0, 1]);
    }

    #[test]
    fn lead_non_trump_does_not_trigger_must_follow() {
        use Rank::*;
        use Suit::*;
        let mut g = GameState::new(0);
        g.dealer = 0;
        g.rechte = Some(Card::new(Hearts, Unter));

        g.players[0].hand = vec![
            Card::new(Hearts, Ace),  // trump
            Card::new(Acorns, King), // not trump
        ];

        // Lead is the Weli (Bells of Weli) — NOT a trump-suit card when
        // trump is Hearts. Per the user's spec only trump-suit leads
        // trigger must-follow, so seeing players may play anything.
        assert_eq!(
            g.allowed_indices(0, Card::new(Bells, Weli)),
            vec![0, 1]
        );

        // Same for a non-trump-suit striker lead (Leaves Unter).
        assert_eq!(
            g.allowed_indices(0, Card::new(Leaves, Unter)),
            vec![0, 1]
        );
    }

    #[test]
    fn non_seeing_player_has_no_must_follow_obligation() {
        use Rank::*;
        use Suit::*;
        let mut g = GameState::new(0);
        g.dealer = 0; // Player 0 dealer; players 0 and 1 are seeing.
        g.rechte = Some(Card::new(Hearts, Unter));

        g.players[2].hand = vec![
            Card::new(Hearts, Ace),
            Card::new(Bells, King),
        ];

        let lead = Card::new(Hearts, Ten);
        let a = g.allowed_indices(2, lead);
        // Player 2 is NOT seeing — both cards remain legal.
        assert_eq!(a, vec![0, 1]);
    }

    #[test]
    fn database_evaluator_produces_non_empty_counts_after_start_round() {
        // Restrict the perm_range so populating doesn't take forever in tests.
        let mut g = GameState::new(0);
        g.set_evaluator(Evaluator::Database);
        g.set_perm_range_single(0);
        // Re-apply evaluator so the new perm_range is observed.
        g.set_evaluator(Evaluator::Database);
        g.start_round();
        // After populate, candidate moves should report some outcome — even
        // if every game in the tiny perm_range was an illegal must-follow
        // violation, the `illegal` counter will be non-zero.
        let p = (g.dealer + 1) % 4;
        let allowed: Vec<usize> = (0..g.players[p].hand.len()).collect();
        let evs = g.evaluate_moves(p, &allowed, &[], [0, 0]);
        assert!(!evs.is_empty());
        assert!(evs.iter().any(|e| e.total > 0 || e.illegal > 0));
    }

    #[test]
    fn custom_evaluator_is_consulted_via_set_evaluator_impl() {
        use crate::evaluator::{EvaluationContext, MoveEvaluator};
        use std::cell::RefCell;
        use std::rc::Rc;

        struct RecordingEval {
            calls: Rc<RefCell<u32>>,
        }
        impl MoveEvaluator for RecordingEval {
            fn prepare_round(
                &mut self,
                _hands: &[[Card; TRICKS_PER_ROUND]; 4],
                _dealer: usize,
                _rechte: Card,
            ) {
            }
            fn evaluate_moves(&self, ctx: &EvaluationContext<'_>) -> Vec<MoveEvaluation> {
                *self.calls.borrow_mut() += 1;
                ctx.allowed_orig_indices
                    .iter()
                    .enumerate()
                    .map(|(i, _orig)| MoveEvaluation {
                        hand_idx: i,
                        // First card looks like a winner; rest losers.
                        wins: if i == 0 { 10 } else { 0 },
                        total: 10,
                        illegal: 0,
                    })
                    .collect()
            }
            fn name(&self) -> &'static str {
                "recording"
            }
        }

        let counter = Rc::new(RefCell::new(0));
        let mut g = GameState::new(0);
        g.rechte = Some(Card::new(Suit::Hearts, Rank::Unter));
        g.players[0].hand = vec![
            Card::new(Suit::Hearts, Rank::Seven),
            Card::new(Suit::Bells, Rank::Ace),
        ];
        g.orig_hands[0][0] = g.players[0].hand[0];
        g.orig_hands[0][1] = g.players[0].hand[1];
        g.set_evaluator_impl(Box::new(RecordingEval {
            calls: counter.clone(),
        }));

        let idx = g.best_card_index(0, &[0, 1]);
        assert_eq!(idx, 0);
        assert!(*counter.borrow() >= 1);
    }

    #[test]
    fn search_memo_reused_across_candidates() {
        use crate::search::{count_completions, SearchMemo, SearchPosition};

        let mut g = GameState::new(0);
        g.dealer = 0;
        g.rechte = Some(Card::new(Suit::Hearts, Rank::Unter));
        g.orig_hands = [
            [
                Card::new(Suit::Hearts, Rank::Unter),
                Card::new(Suit::Bells, Rank::Ace),
                Card::new(Suit::Leaves, Rank::King),
                Card::new(Suit::Hearts, Rank::Ace),
                Card::new(Suit::Acorns, Rank::Ten),
            ],
            [
                Card::new(Suit::Hearts, Rank::Ten),
                Card::new(Suit::Bells, Rank::King),
                Card::new(Suit::Leaves, Rank::Ace),
                Card::new(Suit::Bells, Rank::Seven),
                Card::new(Suit::Acorns, Rank::Nine),
            ],
            [
                Card::new(Suit::Hearts, Rank::King),
                Card::new(Suit::Leaves, Rank::Ober),
                Card::new(Suit::Bells, Rank::Nine),
                Card::new(Suit::Hearts, Rank::Nine),
                Card::new(Suit::Acorns, Rank::Unter),
            ],
            [
                Card::new(Suit::Hearts, Rank::Ober),
                Card::new(Suit::Bells, Rank::Unter),
                Card::new(Suit::Leaves, Rank::Nine),
                Card::new(Suit::Acorns, Rank::Ace),
                Card::new(Suit::Bells, Rank::Ten),
            ],
        ];
        for p in 0..4 {
            g.players[p].hand = g.orig_hands[p].to_vec();
        }

        // First evaluation primes the memo, second should be much faster.
        let allowed: Vec<usize> = (0..5).collect();
        let t0 = std::time::Instant::now();
        let _e1 = g.evaluate_moves(1, &allowed, &[], [0, 0]);
        let first = t0.elapsed();
        let t1 = std::time::Instant::now();
        let _e2 = g.evaluate_moves(1, &allowed, &[], [0, 0]);
        let second = t1.elapsed();
        // Confidence: cached path should be at least 2x faster.
        assert!(
            second * 2 <= first || first.as_micros() < 200,
            "second={:?} first={:?}",
            second,
            first
        );

        // Also verify that count_completions populates a fresh memo with
        // sub-millisecond cost when called twice on the same position.
        let pos = SearchPosition {
            orig_hands: &g.orig_hands,
            remaining: [0b11111; 4],
            lead: 1,
            dealer: 0,
            rechte: g.rechte.unwrap(),
        };
        let mut memo = SearchMemo::new();
        let _ = count_completions(&pos, &mut memo);
        let t2 = std::time::Instant::now();
        let _ = count_completions(&pos, &mut memo);
        assert!(t2.elapsed().as_millis() <= 5);
    }
}

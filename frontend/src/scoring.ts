// TypeScript mirror of `src/rules.rs`. Kept in sync by hand — the Vitest
// suite plays full games through the WASM bindings and compares the trick
// winners against this implementation, so any drift between the two is
// caught immediately.

export interface JsCard {
  suit: string;
  rank: string;
}

export interface TrickEntry {
  card: JsCard;
  player: number;
}

export const RANK_VALUES: Record<string, number> = {
  Seven: 1,
  Eight: 2,
  Nine: 3,
  Ten: 4,
  Unter: 5,
  Ober: 6,
  King: 7,
  Ace: 8,
  Weli: 9,
};

export const RANK_DISPLAY: Record<string, string> = {
  Seven: '7',
  Eight: '8',
  Nine: '9',
  Ten: '10',
  Unter: 'Unter',
  Ober: 'Ober',
  King: 'King',
  Ace: 'Ace',
  Weli: 'Weli',
};

export const displayRank = (r: string): string => RANK_DISPLAY[r] ?? r;

// Per the user's spec:
//   round_score = (trump_suit ? 100 : 0) + (striker_rank ? 200 : 0)
//   trick_score:
//     - if card is striker rank AND an earlier striker was played → -400
//     - else if card.suit === lead_suit                            → rank + 20
//     - else                                                       → rank
//   total = round_score + trick_score (compare with strict >, earlier wins ties)
export function roundScore(card: JsCard, rechte: JsCard): number {
  let s = 0;
  if (card.suit === rechte.suit) s += 100;
  if (card.rank === rechte.rank) s += 200;
  return s;
}

export function trickScore(
  card: JsCard,
  position: number,
  trick: TrickEntry[],
  rechte: JsCard
): number {
  if (card.rank === rechte.rank) {
    for (let earlier = 0; earlier < position; earlier++) {
      if (trick[earlier].card.rank === rechte.rank) return -400;
    }
  }
  const leadSuit = trick[0].card.suit;
  const rv = RANK_VALUES[card.rank] ?? 0;
  return card.suit === leadSuit ? rv + 20 : rv;
}

export function cardTotalScore(
  card: JsCard,
  position: number,
  trick: TrickEntry[],
  rechte: JsCard
): number {
  return roundScore(card, rechte) + trickScore(card, position, trick, rechte);
}

export function trickWinnerIndex(
  trick: TrickEntry[],
  rechte: JsCard | null
): number {
  if (!rechte || trick.length === 0) return 0;
  let bestPos = 0;
  let bestScore = cardTotalScore(trick[0].card, 0, trick, rechte);
  for (let pos = 1; pos < trick.length; pos++) {
    const s = cardTotalScore(trick[pos].card, pos, trick, rechte);
    if (s > bestScore) {
      bestScore = s;
      bestPos = pos;
    }
  }
  return bestPos;
}

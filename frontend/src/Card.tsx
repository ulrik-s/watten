import type { FC, KeyboardEvent } from 'react';
import hearts from './svg/hearts.svg';
import bells from './svg/bells.svg';
import leaves from './svg/leaves.svg';
import acorns from './svg/acorns.svg';

export interface CardProps {
  suit: string;
  rank: string;
  faceDown?: boolean;
  onClick?: () => void;
  selectable?: boolean;
}

const suitMap: Record<string, string> = {
  Hearts: hearts,
  Bells: bells,
  Leaves: leaves,
  Acorns: acorns,
};

export const CardView: FC<CardProps> = ({
  suit,
  rank,
  faceDown,
  onClick,
  selectable,
}) => {
  // A card with an `onClick` is an interactive control: give it button
  // semantics and keyboard support so it isn't mouse-only.
  const interactive = typeof onClick === 'function';
  const onKeyDown = interactive
    ? (e: KeyboardEvent<HTMLDivElement>) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onClick?.();
        }
      }
    : undefined;
  return (
    <div
      className={`card ${selectable ? 'selectable' : ''}`}
      onClick={onClick}
      onKeyDown={onKeyDown}
      role={interactive ? 'button' : undefined}
      tabIndex={interactive ? 0 : undefined}
      data-suit={faceDown ? '' : suit}
      data-rank={faceDown ? '' : rank}
    >
      {faceDown ? (
        <div className="back" />
      ) : (
        <>
          <span className="rank">{rank}</span>
          <img src={suitMap[suit]} className="suit" alt={suit} />
        </>
      )}
    </div>
  );
};

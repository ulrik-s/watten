import React from 'react';
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

export const CardView: React.FC<CardProps> = ({ suit, rank, faceDown, onClick, selectable }) => {
  return (
    <div className={`card ${selectable ? 'selectable' : ''}`} onClick={onClick}>
      {faceDown ? (
        <div className="back" />
      ) : (
        <>
          <span className="rank">{rank}</span>
          <img src={suitMap[suit]} className="suit" />
        </>
      )}
    </div>
  );
};

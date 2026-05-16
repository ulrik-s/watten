import React from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CardView } from '../src/Card';

describe('CardView', () => {
  it('renders the rank text when face-up', () => {
    render(<CardView suit="Hearts" rank="Ace" />);
    expect(screen.getByText('Ace')).toBeInTheDocument();
  });

  it('renders a face-down back instead of rank text', () => {
    const { container } = render(<CardView suit="Hearts" rank="Ace" faceDown />);
    expect(screen.queryByText('Ace')).toBeNull();
    expect(container.querySelector('.back')).toBeInTheDocument();
  });

  it('fires onClick only when selectable', () => {
    const fn = vi.fn();
    const { container, rerender } = render(
      <CardView suit="Hearts" rank="7" onClick={fn} />
    );
    fireEvent.click(container.querySelector('.card')!);
    expect(fn).toHaveBeenCalledTimes(1);

    rerender(<CardView suit="Hearts" rank="7" onClick={fn} selectable />);
    const card = container.querySelector('.card')!;
    expect(card.classList.contains('selectable')).toBe(true);
    fireEvent.click(card);
    expect(fn).toHaveBeenCalledTimes(2);
  });
});

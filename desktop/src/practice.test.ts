import { describe, expect, test } from 'bun:test';
import { initialPracticeState, practiceCommand, updatePractice } from './practice-controls';

describe('practice presentation', () => {
  test('accepts common presentation navigation without stealing modifier chords', () => {
    for (const key of ['ArrowRight', 'ArrowDown', 'PageDown', ' ']) expect(practiceCommand(key)).toBe('next');
    for (const key of ['ArrowLeft', 'ArrowUp', 'PageUp']) expect(practiceCommand(key)).toBe('previous');
    expect(practiceCommand('Home')).toBe('first');
    expect(practiceCommand('End')).toBe('last');
    expect(practiceCommand('B')).toBe('blank');
    expect(practiceCommand('ArrowRight', { ctrlKey: true })).toBeNull();
    expect(practiceCommand('Home', { altKey: true })).toBeNull();
    expect(practiceCommand('b', { metaKey: true })).toBeNull();
    expect(practiceCommand('x')).toBeNull();
  });
  test('keeps navigation within the five-slide deck', () => {
    expect(updatePractice(initialPracticeState, 'previous', 5).index).toBe(0);
    const end = updatePractice(initialPracticeState, 'last', 5);
    expect(end.index).toBe(4);
    expect(updatePractice(end, 'next', 5).index).toBe(4);
    expect(updatePractice(end, 'first', 5)).toEqual(initialPracticeState);
    expect(updatePractice(initialPracticeState, 'next', 5).index).toBe(1);
  });
  test('reveals a blanked slide without accidentally advancing it', () => {
    const blank = updatePractice({ index: 2, mode: 'slides' }, 'blank', 5);
    expect(blank.mode).toBe('blank');
    expect(updatePractice(blank, 'next', 5)).toEqual({ index: 2, mode: 'slides' });
    expect(updatePractice(blank, 'blank', 5)).toEqual({ index: 2, mode: 'slides' });
  });
  test('Escape ends even a blanked presentation; resume preserves the slide', () => {
    const ended = updatePractice({ index: 3, mode: 'blank' }, 'end', 5);
    expect(ended).toEqual({ index: 3, mode: 'ended' });
    expect(updatePractice(ended, 'next', 5)).toEqual(ended);
    expect(updatePractice(ended, 'resume', 5)).toEqual({ index: 3, mode: 'slides' });
    expect(updatePractice(ended, 'restart', 5)).toEqual(initialPracticeState);
  });
});

export type PracticeMode = 'slides' | 'blank' | 'ended';
export interface PracticeState { index: number; mode: PracticeMode }
export type PracticeCommand = 'next' | 'previous' | 'first' | 'last' | 'blank' | 'end' | 'resume' | 'restart';
export const initialPracticeState: PracticeState = { index: 0, mode: 'slides' };

export function practiceCommand(key: string, modifiers: { ctrlKey?: boolean; altKey?: boolean; metaKey?: boolean } = {}): PracticeCommand | null {
  if (modifiers.ctrlKey || modifiers.altKey || modifiers.metaKey) return null;
  if (['ArrowRight', 'ArrowDown', 'PageDown', ' '].includes(key)) return 'next';
  if (['ArrowLeft', 'ArrowUp', 'PageUp'].includes(key)) return 'previous';
  if (key === 'Home') return 'first';
  if (key === 'End') return 'last';
  if (key.toLowerCase() === 'b') return 'blank';
  if (key === 'Escape') return 'end';
  if (key === 'Enter') return 'resume';
  return null;
}

export function updatePractice(state: PracticeState, command: PracticeCommand, slideCount: number): PracticeState {
  const last = Math.max(0, slideCount - 1);
  if (command === 'restart') return { ...initialPracticeState };
  if (command === 'end') return { ...state, mode: 'ended' };
  if (command === 'resume') return { ...state, mode: 'slides' };
  if (state.mode === 'ended') return state;
  if (command === 'blank') return { ...state, mode: state.mode === 'blank' ? 'slides' : 'blank' };
  // A navigation key reveals a blanked slide without skipping its content.
  if (state.mode === 'blank') return { ...state, mode: 'slides' };
  if (command === 'first') return { index: 0, mode: 'slides' };
  if (command === 'last') return { index: last, mode: 'slides' };
  return { index: Math.max(0, Math.min(last, state.index + (command === 'next' ? 1 : -1))), mode: 'slides' };
}

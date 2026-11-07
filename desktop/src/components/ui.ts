import type { MacroStep, Mapping, Settings } from '../types';

export function stepSummary(step: MacroStep): string {
  if (step.kind === 'key') return step.keys || 'Choose a shortcut';
  if (step.kind === 'delay') return `${step.ms} ms`;
  return step.text ? `“${step.text.slice(0,28)}${step.text.length > 28 ? '…' : ''}”` : 'Type text';
}
export function mappingSummary(mapping: Mapping): string {
  if (!mapping.enabled) return 'Disabled';
  if (!mapping.steps.length) return 'Add an action';
  return mapping.steps.map(stepSummary).join(' → ');
}
export function captureShortcut(event: {key:string;ctrlKey:boolean;altKey:boolean;shiftKey:boolean;metaKey:boolean}): string | null {
  if (['Control','Shift','Alt','Meta'].includes(event.key)) return null;
  const aliases: Record<string,string> = { ' ': 'Space', ArrowRight: 'Right', ArrowLeft:'Left', ArrowUp:'Up', ArrowDown:'Down', Esc:'Escape' };
  const key = aliases[event.key] ?? (event.key.length === 1 ? event.key.toUpperCase() : event.key);
  if (!/^(?:[A-Z0-9]|F(?:[1-9]|1\d|2[0-4])|Right|Left|Up|Down|Escape|Space|Enter|Tab|Backspace|Home|End|PageUp|PageDown|Delete|Insert)$/.test(key)) return null;
  return [...(event.ctrlKey ? ['Ctrl']:[]), ...(event.altKey ? ['Alt']:[]), ...(event.shiftKey ? ['Shift']:[]), ...(event.metaKey ? ['Win']:[]),key].join('+');
}
export function shortcutError(value: string): string | null {
  const text = value.trim();
  if (!text) return 'Choose a shortcut, or enter None to do nothing.';
  if (text.toLowerCase() === 'none') return null;
  if (text.length > 100) return 'Keep shortcuts within 100 characters.';
  if (/[^\x00-\x7f]/.test(text)) return 'Use a letter, digit, F1 to F24, or a navigation key.';
  const modifiers = new Set<string>();
  let mainKey = false;
  const modifierNames: Record<string,string> = { CTRL:'Ctrl', CONTROL:'Ctrl', ALT:'Alt', SHIFT:'Shift', WIN:'Win', WINDOWS:'Win', META:'Win' };
  for (const part of text.split('+')) {
    const key = part.trim().toUpperCase();
    const modifier = modifierNames[key];
    if (modifier) {
      if (modifiers.has(modifier)) return `${modifier} appears more than once.`;
      modifiers.add(modifier);
      continue;
    }
    if (!/^(?:[A-Z0-9]|F(?:[1-9]|1\d|2[0-4])|(?:ARROW)?(?:RIGHT|LEFT|UP|DOWN)|ESCAPE|ESC|SPACE|SPACEBAR|ENTER|RETURN|TAB|BACKSPACE|HOME|END|PAGEUP|PGUP|PAGEDOWN|PGDOWN|PGDN|DELETE|DEL|INSERT|INS)$/.test(key)) return 'Use a letter, digit, F1 to F24, or a navigation key.';
    if (mainKey) return 'Use one main key with optional Ctrl, Alt, Shift, or Win.';
    mainKey = true;
  }
  return mainKey ? null : 'Add a main key, for example Ctrl+F5.';
}
export function actionNameError(name: string): string | null {
  if (!name.trim()) return 'Give this action a name.';
  if (Array.from(name).length > 80) return 'Keep the action name within 80 characters.';
  if (/[\u0000-\u001f\u007f-\u009f]/.test(name)) return 'Remove control characters from the action name.';
  return null;
}
export function stepError(step: MacroStep): string | null {
  if (step.kind === 'key') return shortcutError(step.keys);
  if (step.kind === 'delay' && (!Number.isInteger(step.ms) || step.ms < 0 || step.ms > 30000)) return 'Use a delay from 0 to 30,000 milliseconds.';
  if (step.kind === 'text' && step.text.includes('\0')) return 'Text cannot contain a null character.';
  return null;
}
export function mappingError(mapping: Mapping): string | null {
  const nameError = actionNameError(mapping.name);
  if (nameError) return nameError;
  if (mapping.enabled && !mapping.steps.length) return 'Add at least one step, or turn this mapping off.';
  if (mapping.steps.length > 32) return 'A macro can have up to 32 steps.';
  if (mapping.steps.reduce((total, step) => total + (step.kind === 'text' ? Array.from(step.text).length : 0),0) > 2000) return 'Keep total text across the macro within 2,000 characters.';
  if (mapping.steps.filter(step => step.kind === 'delay').reduce((total, step) => total + (step.kind === 'delay' ? step.ms : 0),0) > 30000) return 'Keep total delays within 30 seconds.';
  for (const [index,step] of mapping.steps.entries()) {
    const error = stepError(step);
    if (error) return `Step ${index + 1}: ${error}`;
  }
  return null;
}

export function settingsIssue(settings: Settings): {profileId:string;mappingId?:string;message:string}|null {
  for (const profile of settings.profiles) {
    if (!profile.name.trim()) return {profileId:profile.id,message:'Give this profile a name.'};
    for (const mapping of profile.mappings) {
      const message = mappingError(mapping);
      if (message) return {profileId:profile.id,mappingId:mapping.id,message};
    }
  }
  return null;
}
export function readImportedSettings(text: string): Settings {
  const parsed: unknown = JSON.parse(text);
  if (!parsed || typeof parsed !== 'object') throw new Error('Choose an EarControl settings JSON file.');
  const value = parsed as Partial<Settings>;
  if (value.version !== 2 || !Array.isArray(value.profiles) || !value.profiles.length || value.profiles.length > 32 || typeof value.activeProfileId !== 'string' || typeof value.keyboardFallback !== 'boolean' || typeof value.cooldownMs !== 'number' || !['light','dark','system'].includes(String(value.theme))) throw new Error('This file is not a valid EarControl 2 settings export.');
  if (!Number.isInteger(value.cooldownMs) || (value.cooldownMs !== 0 && (value.cooldownMs < 80 || value.cooldownMs > 2000))) throw new Error('Repeat delay must be off or between 80 and 2000 milliseconds.');
  for (const profile of value.profiles) {
    if (!profile || typeof profile.id !== 'string' || typeof profile.name !== 'string' || typeof profile.description !== 'string' || typeof profile.accent !== 'string' || !Array.isArray(profile.mappings)) throw new Error('A profile in this file is incomplete.');
    for (const mapping of profile.mappings) {
      if (!mapping || typeof mapping.id !== 'string' || typeof mapping.trigger !== 'string' || typeof mapping.name !== 'string' || typeof mapping.enabled !== 'boolean' || !Array.isArray(mapping.steps)) throw new Error('A mapping in this file is incomplete.');
      for (const step of mapping.steps) {
        if (!step || !['key','delay','text'].includes(step.kind) || (step.kind === 'key' && typeof step.keys !== 'string') || (step.kind === 'delay' && typeof step.ms !== 'number') || (step.kind === 'text' && typeof step.text !== 'string')) throw new Error('A macro step in this file is invalid.');
      }
    }
  }
  return value as Settings;
}

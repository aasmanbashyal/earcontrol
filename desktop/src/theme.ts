import type { Settings } from './types';

export type Theme = Settings['theme'];
export const LIVE_THEME_KEY = 'earcontrol-theme';
export const SAVED_THEME_KEY = 'earcontrol-saved-theme';

export function resolveTheme(preference: Theme, systemDark: boolean): 'light'|'dark' {
  return preference === 'system' ? systemDark ? 'dark' : 'light' : preference;
}

export function readSavedTheme(storage: Pick<Storage,'getItem'>, fallback: Theme = 'system'): Theme {
  const preference = storage.getItem(SAVED_THEME_KEY) ?? storage.getItem(LIVE_THEME_KEY);
  return preference === 'light' || preference === 'dark' || preference === 'system' ? preference : fallback;
}

export function readLiveTheme(storage: Pick<Storage,'getItem'>): Theme {
  const preference=storage.getItem(LIVE_THEME_KEY);
  return preference==='light'||preference==='dark'||preference==='system'?preference:readSavedTheme(storage);
}

export function applyTheme(preference: Theme, systemDark: boolean): 'light'|'dark' {
  const resolved = resolveTheme(preference, systemDark);
  document.documentElement.dataset.theme = resolved;
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', resolved === 'dark' ? '#101010' : '#fafafa');
  return resolved;
}

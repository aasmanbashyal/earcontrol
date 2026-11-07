import { describe, expect, test } from 'bun:test';
import { LIVE_THEME_KEY, SAVED_THEME_KEY, readLiveTheme, readSavedTheme, resolveTheme } from './theme';

describe('theme preference',()=>{
  test('explicit themes ignore the system and System follows both appearances',()=>{
    for(const dark of [true,false]) {
      expect(resolveTheme('dark',dark)).toBe('dark');
      expect(resolveTheme('light',dark)).toBe('light');
      expect(resolveTheme('system',dark)).toBe(dark?'dark':'light');
    }
  });
  test('startup uses the saved preference rather than an unsaved live preview',()=>{
    const values = new Map([[SAVED_THEME_KEY,'system'],[LIVE_THEME_KEY,'light']]);
    const storage = {getItem:(key:string)=>values.get(key)??null};
    expect(readSavedTheme(storage)).toBe('system');
    expect(readLiveTheme(storage)).toBe('light');
    values.set(SAVED_THEME_KEY,'dark');
    expect(readSavedTheme(storage)).toBe('dark');
    values.delete(SAVED_THEME_KEY);
    expect(readSavedTheme(storage)).toBe('light');
    values.set(LIVE_THEME_KEY,'invalid');
    expect(readSavedTheme(storage)).toBe('system');
    expect(readSavedTheme(storage,'dark')).toBe('dark');
    values.set(LIVE_THEME_KEY,'system');
    expect(readLiveTheme(storage)).toBe('system');
  });
});

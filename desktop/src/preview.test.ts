import { describe, expect, test } from 'bun:test';
import { loadPreviewSettings, PREVIEW_SETTINGS_KEY, previewDefaults, savePreviewSettings } from './preview';

function storage(initial: Record<string,string> = {}) {
  const values = new Map(Object.entries(initial));
  return { values, getItem: (key:string) => values.get(key) ?? null, setItem: (key:string,value:string) => {values.set(key,value);} };
}

describe('browser preview settings',()=>{
  test('saves repeat delay off and profiles only to the separate preview key',()=>{
    const local = storage({'desktop-settings':'leave untouched'});
    const settings = previewDefaults();
    settings.cooldownMs = 0;
    settings.profiles[0].name = 'Preview controls';
    savePreviewSettings(settings,local);
    expect(loadPreviewSettings(local)).toEqual(settings);
    expect(local.getItem('desktop-settings')).toBe('leave untouched');
    expect([...local.values.keys()]).toEqual(['desktop-settings',PREVIEW_SETTINGS_KEY]);
  });
  test('restores saved preview preferences before the cached display theme',()=>{
    const local = storage({'earcontrol-theme':'dark'});
    savePreviewSettings({...previewDefaults(),theme:'light'},local);
    expect(loadPreviewSettings(local).theme).toBe('light');
    expect(loadPreviewSettings(storage({'earcontrol-theme':'light'})).theme).toBe('light');
    expect(loadPreviewSettings(storage()).theme).toBe('dark');
  });
  test('invalid saves preserve the previous preview and damaged storage is not overwritten',()=>{
    const local = storage();
    savePreviewSettings(previewDefaults(),local);
    const before = local.getItem(PREVIEW_SETTINGS_KEY);
    const invalid = previewDefaults();
    invalid.profiles[0].mappings[0].steps = [];
    expect(()=>savePreviewSettings(invalid,local)).toThrow('Add at least one step');
    expect(local.getItem(PREVIEW_SETTINGS_KEY)).toBe(before);
    local.setItem(PREVIEW_SETTINGS_KEY,'{damaged');
    expect(loadPreviewSettings(local)).toEqual(previewDefaults());
    expect(local.getItem(PREVIEW_SETTINGS_KEY)).toBe('{damaged');
  });
});

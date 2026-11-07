import { mappingError, readImportedSettings } from './components/ui';
import type { Settings } from './types';
import { readSavedTheme } from './theme';

// Browser review data never uses the desktop settings file or its storage key.
export const PREVIEW_SETTINGS_KEY = 'earcontrol-browser-preview-settings-v2';

export function previewDefaults(theme: Settings['theme'] = 'dark'): Settings {
  return {
    version: 2, activeProfileId: 'slides', keyboardFallback: true, cooldownMs: 180, theme,
    profiles: [{
      id: 'slides', name: 'Google Slides', description: 'Present with your headphones', accent: '#a64619',
      mappings: [
        { id: 'slides-play', trigger: 'play_pause', name: 'Next slide', enabled: true, steps: [{ kind: 'key', keys: 'Right' }] },
        { id: 'slides-next', trigger: 'next', name: 'Next slide', enabled: true, steps: [{ kind: 'key', keys: 'Right' }] },
        { id: 'slides-previous', trigger: 'previous', name: 'Previous slide', enabled: true, steps: [{ kind: 'key', keys: 'Left' }] },
        { id: 'slides-stop', trigger: 'stop', name: 'Exit presentation', enabled: true, steps: [{ kind: 'key', keys: 'Escape' }] },
        { id: 'slides-volume-up', trigger: 'volume_up', name: 'Volume up', enabled: false, steps: [] },
        { id: 'slides-volume-down', trigger: 'volume_down', name: 'Volume down', enabled: false, steps: [] },
        { id: 'slides-mute', trigger: 'mute', name: 'Mute', enabled: false, steps: [] },
      ],
    }],
  };
}

function validatePreviewSettings(text: string): Settings {
  const settings = readImportedSettings(text);
  if (!settings.profiles.some(profile => profile.id === settings.activeProfileId)) throw new Error('Choose an existing profile.');
  for (const profile of settings.profiles) {
    if (!profile.name.trim()) throw new Error('Give each profile a name.');
    for (const mapping of profile.mappings) {
      const error = mappingError(mapping);
      if (error) throw new Error(profile.name + ' / ' + mapping.name + ': ' + error);
    }
  }
  return settings;
}

export function loadPreviewSettings(storage: Pick<Storage,'getItem'>): Settings {
  const theme = readSavedTheme(storage,'dark');
  const saved = storage.getItem(PREVIEW_SETTINGS_KEY);
  if (saved) {
    try { return validatePreviewSettings(saved); }
    catch { /* Leave a damaged preview export untouched; show editable defaults. */ }
  }
  return previewDefaults(theme);
}

export function savePreviewSettings(settings: Settings, storage: Pick<Storage,'setItem'>): Settings {
  const next = validatePreviewSettings(JSON.stringify(settings));
  storage.setItem(PREVIEW_SETTINGS_KEY, JSON.stringify(next));
  return next;
}

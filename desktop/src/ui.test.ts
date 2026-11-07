import { describe, expect, test } from 'bun:test';
import { actionNameError, captureShortcut, mappingError, readImportedSettings, settingsIssue, shortcutError, stepError } from './components/ui';
import type { Mapping, Settings } from './types';

const settings:Settings={version:2,activeProfileId:'slides',profiles:[{id:'slides',name:'Slides',description:'Present',accent:'#a64619',mappings:[{id:'next',trigger:'play_pause',name:'Next slide',enabled:true,steps:[{kind:'key',keys:'Right'}]}]}],keyboardFallback:true,cooldownMs:180,theme:'light'};
const event=(key:string,options:Partial<{ctrlKey:boolean;shiftKey:boolean;altKey:boolean;metaKey:boolean}>={})=>({key,ctrlKey:false,shiftKey:false,altKey:false,metaKey:false,...options});

describe('shortcut recorder',()=>{
  test('normalizes supported Windows keys and modifier chords',()=>{
    expect(captureShortcut(event('ArrowRight'))).toBe('Right');
    expect(captureShortcut(event('l',{ctrlKey:true,shiftKey:true}))).toBe('Ctrl+Shift+L');
    expect(captureShortcut(event(' '))).toBe('Space');
    expect(captureShortcut(event('F24'))).toBe('F24');
    expect(captureShortcut(event('Escape'))).toBe('Escape');
    expect(captureShortcut(event('Tab',{altKey:true}))).toBe('Alt+Tab');
  });
  test('does not turn modifier-only presses or unsupported punctuation into a macro',()=>{
    expect(captureShortcut(event('Control',{ctrlKey:true}))).toBeNull();
    expect(captureShortcut(event('?',{shiftKey:true}))).toBeNull();
    expect(captureShortcut(event('AudioVolumeUp'))).toBeNull();
  });
});

describe('shortcut validation',()=>{
  test('accepts the native parser aliases and explicit no-op',()=>{
    for(const keys of [' shift + control + l ', 'Win+Alt+F24', 'Ctrl+0', 'ArrowRight', 'Esc', 'PageDown', 'PgDn', 'Return', 'Spacebar', 'Ins', 'Del', ' none ']) {
      expect(shortcutError(keys)).toBeNull();
    }
  });
  test('shows invalid shortcuts before they reach a save or test request',()=>{
    for(const keys of ['', ' ', 'Ctrl', 'Ctrl+Shift', 'Ctrl+Control+A', 'Win+Meta+A', 'A+B', 'A+A', 'Ctrl++A', 'Alt+', 'F0', 'F25', 'F01', 'None+A', 'MediaNext', 'é']) {
      expect(shortcutError(keys)).not.toBeNull();
    }
  });
});

describe('settings import',()=>{
  test('accepts a settings export without altering it',()=>{
    expect(readImportedSettings(JSON.stringify(settings))).toEqual(settings);
  });
  test('preserves disabled repeat delay and accepts existing delay settings',()=>{
    for(const cooldownMs of [0,80,180,2000]) {
      expect(readImportedSettings(JSON.stringify({...settings,cooldownMs})).cooldownMs).toBe(cooldownMs);
    }
    for(const cooldownMs of [-1,1,79,80.5,2001]) {
      expect(()=>readImportedSettings(JSON.stringify({...settings,cooldownMs}))).toThrow('Repeat delay');
    }
  });
  test('rejects malformed and incompatible data before rendering it',()=>{
    expect(()=>readImportedSettings('[]')).toThrow();
    expect(()=>readImportedSettings('{')).toThrow();
    expect(()=>readImportedSettings(JSON.stringify({...settings,version:1}))).toThrow();
    expect(()=>readImportedSettings(JSON.stringify({...settings,profiles:[{...settings.profiles[0],mappings:[{...settings.profiles[0].mappings[0],steps:[{kind:'text',text:{html:'bad'}}]}]}]}))).toThrow();
  });
});

describe('macro editor validation',()=>{
  const mapping:Mapping=settings.profiles[0].mappings[0];
  test('requires an action when enabled, but allows an unused mapping',()=>{
    expect(mappingError({...mapping,steps:[]})).not.toBeNull();
    expect(mappingError({...mapping,enabled:false,steps:[]})).toBeNull();
  });
  test('rejects unbounded delay sequences and text',()=>{
    expect(mappingError({...mapping,steps:[{kind:'delay',ms:20000},{kind:'delay',ms:20000}]})).not.toBeNull();
    expect(mappingError({...mapping,steps:[{kind:'text',text:'x'.repeat(2001)}]})).not.toBeNull();
    expect(mappingError({...mapping,steps:[{kind:'text',text:'x'.repeat(1001)},{kind:'text',text:'y'.repeat(1000)}]})).not.toBeNull();
    expect(mappingError({...mapping,steps:[{kind:'delay',ms:NaN}]})).not.toBeNull();
  });
  test('identifies the invalid shortcut step',()=>{
    expect(mappingError({...mapping,steps:[{kind:'delay',ms:500},{kind:'key',keys:'Ctrl'}]})).toContain('Step 2');
    expect(mappingError({...mapping,steps:[{kind:'key',keys:'Ctrl+Shift+L'}]})).toBeNull();
  });
  test('counts Unicode text like native validation and rejects null characters',()=>{
    expect(mappingError({...mapping,steps:[{kind:'text',text:'😀'.repeat(2000)}]})).toBeNull();
    expect(mappingError({...mapping,steps:[{kind:'text',text:'😀'.repeat(2001)}]})).not.toBeNull();
    expect(mappingError({...mapping,steps:[{kind:'text',text:'hello\0world'}]})).not.toBeNull();
  });
  test('global save locates invalid actions even in an inactive profile',()=>{
    const invalid = {...settings.profiles[0],id:'other',mappings:[{...mapping,id:'broken',steps:[{kind:'key' as const,keys:'Ctrl'}]}]};
    const all = {...settings,profiles:[settings.profiles[0],invalid]};
    expect(settingsIssue(all)).toEqual({profileId:'other',mappingId:'broken',message:mappingError(invalid.mappings[0])!});
    expect(settingsIssue({...all,profiles:[settings.profiles[0],{...invalid,name:' '}]})).toEqual({profileId:'other',message:'Give this profile a name.'});
    expect(settingsIssue(settings)).toBeNull();
  });
  test('field errors clear as the action is corrected and aggregate limits remain checked',()=>{
    expect(actionNameError(' ')).toBe('Give this action a name.');
    expect(actionNameError('Next slide')).toBeNull();
    expect(stepError({kind:'key',keys:'Ctrl'})).not.toBeNull();
    expect(stepError({kind:'key',keys:'Ctrl+L'})).toBeNull();
    expect(stepError({kind:'delay',ms:-1})).not.toBeNull();
    expect(stepError({kind:'delay',ms:15000})).toBeNull();
    expect(mappingError({...mapping,steps:[{kind:'delay',ms:20000},{kind:'delay',ms:20000}]})).toBe('Keep total delays within 30 seconds.');
  });
});

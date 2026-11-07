import { useCallback, useEffect, useRef, useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { ArrowLeft, ArrowRight, Check, Keyboard, Monitor, RotateCcw, Type } from 'lucide-react';
import { BrandIcon } from './components/Icons';
import { WindowChrome } from './components/WindowChrome';
import { applyTheme, LIVE_THEME_KEY, SAVED_THEME_KEY, readLiveTheme, type Theme } from './theme';
import { initialPracticeState, practiceCommand, updatePractice, type PracticeCommand } from './practice-controls';

const slides = [
  { label: 'Start here', title: 'Try your buttons.', body: 'Start control in EarControl, then return to this window. Press the headphone button mapped to Right.', hint: 'A successful press moves to slide 02.' },
  { label: 'Next slide', title: 'One press. One slide.', body: 'Your next-slide shortcut reached this window. Press the button mapped to Left to go back, or continue with Right.', hint: 'Try a few quick presses to check the repeat setting.' },
  { label: 'Navigation', title: 'Move at your pace.', body: 'Next and previous move one slide at a time. Home returns to the first slide. End jumps to the last.', hint: 'Keyboard shortcuts and headphone macros use the same controls here.' },
  { label: 'Presentation controls', title: 'Pause the screen.', body: 'Press B to blank the slide. Press B or a navigation key to reveal it. Escape ends the presentation.', hint: 'After Escape, press Enter or choose Resume to continue.' },
  { label: 'Text macros', title: 'Test the rest.', body: 'Click the text field to test a text macro. Click this slide again when you want to return to navigation.', hint: 'Use Restart deck to repeat the check from the beginning.' },
];

export function Practice() {
  const [state, setState] = useState(initialPracticeState);
  const [text, setText] = useState('');
  const [typing, setTyping] = useState(false);
  const [lastKey, setLastKey] = useState<{ key: string; count: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const slideRef = useRef<HTMLElement>(null);
  const dispatch = useCallback((command: PracticeCommand) => setState(current => updatePractice(current, command, slides.length)), []);
  const focusSlide = () => slideRef.current?.focus({ preventScroll: true });
  const control = (command: PracticeCommand) => { dispatch(command); focusSlide(); };

  useEffect(() => {
    const system = matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      let preference: Theme = 'system';
      try {
        preference = readLiveTheme(localStorage);
      } catch { /* System theme also works without storage. */ }
      applyTheme(preference,system.matches);
    };
    const storage = (event: StorageEvent) => { if (event.key === LIVE_THEME_KEY || event.key === SAVED_THEME_KEY || event.key === null) apply(); };
    apply();
    window.addEventListener('storage', storage);
    system.addEventListener('change', apply);
    return () => { window.removeEventListener('storage', storage); system.removeEventListener('change', apply); };
  }, []);

  useEffect(() => {
    focusSlide();
    const onKey = (event: KeyboardEvent) => {
      const target = event.target instanceof HTMLElement ? event.target : null;
      if (target?.closest('input,textarea,select,[contenteditable="true"]')) return;
      if (target?.closest('button,a') && (event.key === ' ' || event.key === 'Enter')) return;
      const command = practiceCommand(event.key, event);
      if (!command) return;
      event.preventDefault();
      dispatch(command);
      const key = event.key === ' ' ? 'Space' : event.key.replace('Arrow', '');
      setLastKey(current => ({ key, count: (current?.count ?? 0) + 1 }));
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [dispatch]);

  const slide = slides[state.index];
  return <div className="practice-app">
    <header className="practice-toolbar" data-tauri-drag-region={isTauri() ? 'deep' : undefined}><div className="practice-brand"><BrandIcon size={22}/><span>EarControl<small>Practice deck</small></span></div><div className="practice-header-actions"><button className="button compact" data-tauri-drag-region="false" onClick={() => control('restart')}><RotateCcw size={14}/>Restart deck</button><WindowChrome variant="controls" title="Practice deck" onError={setError}/></div></header>
    <main className="practice-main">
      <div className="practice-workspace">
        <section ref={slideRef} className={'practice-slide ' + (state.mode !== 'slides' ? 'practice-slide-' + state.mode : '')} tabIndex={0} aria-label="Slide navigation" onClick={focusSlide}>
          <div className="practice-slide-meta"><span>{state.mode === 'slides' ? slide.label : state.mode === 'blank' ? 'Screen paused' : 'Presentation ended'}</span><span className="practice-counter"><span aria-hidden="true">{String(state.index + 1).padStart(2, '0')} / {String(slides.length).padStart(2, '0')}</span><span className="sr-only">Slide {state.index + 1} of {slides.length}</span></span></div>
          {state.mode === 'slides' ? <div className="practice-slide-content" aria-live="polite"><h1>{slide.title}</h1><p>{slide.body}</p><div className="practice-slide-hint">{slide.hint}</div></div> : <div className="practice-slide-content practice-paused"><span className="practice-pause-symbol">{state.mode === 'blank' ? <Monitor size={28}/> : <Check size={28}/>}</span><h1>{state.mode === 'blank' ? 'Screen blanked.' : 'Presentation ended.'}</h1><p>{state.mode === 'blank' ? 'Press B or a navigation key to reveal this slide.' : 'Press Enter or choose Resume to return to this slide.'}</p><button className="button" onClick={() => control('resume')}>Resume<ArrowRight size={14}/></button></div>}
          <div className="practice-slide-bottom"><span className="practice-focus-hint">{typing ? 'Click this slide to navigate' : 'Ready for navigation'}</span><div className="practice-progress" aria-hidden="true">{slides.map((_, index) => <i key={index} className={index === state.index ? 'current' : ''}/>)}</div></div>
        </section>
        <div className="practice-navigation"><button className="button" aria-label="Previous slide" disabled={state.index === 0 || state.mode !== 'slides'} onClick={() => control('previous')}><ArrowLeft size={14}/>Previous</button><span>{state.index + 1} of {slides.length}</span><button className="button" aria-label="Next slide" disabled={state.index === slides.length - 1 || state.mode !== 'slides'} onClick={() => control('next')}>Next<ArrowRight size={14}/></button></div>
      </div>

      <aside className="practice-tools">
        <section className="practice-tool"><h2><Keyboard size={15}/>Received shortcut</h2><div className="practice-last-key" aria-live="polite">{lastKey ? <><kbd>{lastKey.key}</kbd><span>{lastKey.count} received</span></> : <span>No shortcuts yet</span>}</div><p>Keyboard input and headphone actions appear here while this window is active.</p></section>
        <section className="practice-tool"><h2><Type size={15}/>Text macro test</h2><label className="sr-only" htmlFor="practice-text">Text macro test</label><textarea id="practice-text" value={text} maxLength={10000} rows={4} placeholder="Click here, then run a text macro." onChange={event => setText(event.target.value)} onFocus={() => setTyping(true)} onBlur={() => setTyping(false)}/><div className="practice-text-footer"><span>{text.length} characters</span><button className="text-button" disabled={!text} onClick={() => setText('')}>Clear</button></div><p className={typing ? 'practice-typing-hint' : ''}>{typing ? 'Text field focused. Click the slide to resume navigation.' : 'Text stays in this window and clears when it closes.'}</p></section>
        <details className="practice-key-guide"><summary>Keyboard reference</summary><dl><div><dt>Next / previous</dt><dd><kbd>←</kbd><kbd>→</kbd></dd></div><div><dt>First / last</dt><dd><kbd>Home</kbd><kbd>End</kbd></dd></div><div><dt>Blank screen</dt><dd><kbd>B</kbd></dd></div><div><dt>End presentation</dt><dd><kbd>Esc</kbd></dd></div></dl><p>Page Up, Page Down, Up, Down, and Space also navigate.</p></details>
      </aside>
    </main>
    {error && <div className="practice-error" role="alert">{error}<button className="text-button" onClick={() => setError(null)}>Dismiss</button></div>}
    <footer className="practice-footer"><span><span className={'status-dot ' + (typing ? '' : 'connected')}/>{typing ? 'Text input' : 'Slide navigation'}</span><span>Start control in EarControl, then keep this window in front.</span></footer>
  </div>;
}

import { useEffect, useRef, useState, type ReactNode } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Copy, Minus, Square, X } from 'lucide-react';

interface WindowChromeProps { title: string; icon?: ReactNode; onError?: (message: string) => void; variant?: 'bar'|'controls' }

export function WindowChrome({title,icon,onError,variant='bar'}:WindowChromeProps) {
  const desktop=isTauri();
  const [maximized,setMaximized]=useState(false);
  const [pending,setPending]=useState(false);
  const errorCallback=useRef(onError);
  errorCallback.current=onError;
  const report=(error:unknown)=>errorCallback.current?.(error instanceof Error?error.message:String(error));

  useEffect(()=>{
    if(!desktop)return;
    let alive=true;
    let unlisten:(()=>void)|undefined;
    const current=getCurrentWindow();
    const update=()=>void current.isMaximized().then(value=>{if(alive)setMaximized(value);}).catch(report);
    update();
    void current.onResized(update).then(stop=>{if(alive)unlisten=stop;else stop();}).catch(report);
    return()=>{alive=false;unlisten?.();};
  },[desktop]);

  useEffect(()=>{
    const nativeTitle=title.startsWith('EarControl')?title:'EarControl · '+title;
    if(desktop)void getCurrentWindow().setTitle(nativeTitle).catch(report);
  },[desktop,title]);

  const act=async(action:'minimize'|'maximize'|'close')=>{
    if(!desktop||pending)return;
    setPending(true);
    try {
      const current=getCurrentWindow();
      if(action==='minimize')await current.minimize();
      else if(action==='close')await current.close();
      else {await current.toggleMaximize();setMaximized(await current.isMaximized());}
    } catch(error) {report(error);} finally {setPending(false);}
  };

  const controls=desktop?<div className="window-controls" data-tauri-drag-region="false" role="group" aria-label="Window controls">
      <button type="button" className="window-control" aria-label="Minimize window" title="Minimize" disabled={pending} onClick={()=>void act('minimize')}><Minus size={14}/></button>
      <button type="button" className="window-control" aria-label={maximized?'Restore window':'Maximize window'} title={maximized?'Restore':'Maximize'} disabled={pending} onClick={()=>void act('maximize')}>{maximized?<Copy size={12}/>:<Square size={12}/>}</button>
      <button type="button" className="window-control window-close" aria-label="Close window" title="Close" disabled={pending} onClick={()=>void act('close')}><X size={16}/></button>
    </div>:null;
  if(variant==='controls')return controls;

  // Tauri's drag-region handler also implements double-click maximize on Windows.
  return <header className="window-chrome" data-tauri-drag-region={desktop?'deep':undefined}>
    <div className="window-chrome-title">{icon}<span>{title}</span></div>
    {controls}
  </header>;
}

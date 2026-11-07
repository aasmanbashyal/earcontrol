import { useEffect, useId, useRef } from 'react';

export function RepeatDelay({value,onChange,disabled=false}: {value:number;onChange:(milliseconds:number)=>void;disabled?:boolean}) {
  const id=useId();
  const previousDelay=useRef(value || 180);
  useEffect(()=>{if(value>0)previousDelay.current=value;},[value]);
  const enabled=value>0;
  const amount=enabled?value:previousDelay.current;

  return <div className="setting-row cooldown-setting">
    <div><label className="setting-title" htmlFor={id}>Repeat delay</label><p id={id+'-description'}>Ignore repeated presses of the same command within this interval.</p></div>
    <div className="setting-control"><span className="value-chip">{enabled?value+' ms':'Off'}</span><input id={id} className="switch" type="checkbox" role="switch" aria-label="Repeat delay" aria-describedby={id+'-description'} checked={enabled} disabled={disabled} onChange={event=>onChange(event.target.checked?previousDelay.current:0)}/></div>
    <div className={'slider-wrap'+(!enabled?' disabled':'')}><input aria-label="Repeat delay in milliseconds" type="range" min={80} max={2000} step={10} value={amount} disabled={disabled||!enabled} onChange={event=>onChange(Number(event.target.value))}/><div><span>80 ms</span><span>2,000 ms</span></div></div>
    <p className="setting-note">Duplicate reports of a single press are still filtered when repeat delay is off.</p>
  </div>;
}

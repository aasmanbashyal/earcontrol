import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { ArrowDown, ArrowUp, ChevronRight, Clock3, Keyboard, Play, Plus, Trash2, Type } from 'lucide-react';
import type { MacroStep, Mapping } from '../types';
import { TRIGGERS } from '../types';
import { actionNameError, captureShortcut, mappingError, stepError, stepSummary } from './ui';
import { TriggerIcon } from './Icons';

export function MacroEditor({mapping,onChange,onTest,canTest,invalidDraft,saving}: {mapping:Mapping;onChange:(mapping:Mapping)=>void;onTest:()=>void;canTest:boolean;invalidDraft:boolean;saving:boolean}) {
  const [recording,setRecording] = useState<number|null>(null);
  const id=useId();
  const editor=useRef<HTMLElement>(null);
  const nameInput=useRef<HTMLInputElement>(null);
  const pendingFocus=useRef<string|null>(null);
  useEffect(()=>{nameInput.current?.focus({preventScroll:true});},[mapping.id]);
  useEffect(()=>setRecording(null),[mapping.id,saving]);
  useLayoutEffect(()=>{
    if(!pendingFocus.current)return;
    const element=editor.current?.querySelector<HTMLElement>(pendingFocus.current);
    pendingFocus.current=null;
    element?.focus();
  },[mapping.steps]);
  const trigger = TRIGGERS.find(item=>item.id===mapping.trigger);
  const updateStep = (index:number,step:MacroStep) => onChange({...mapping,steps:mapping.steps.map((old,i)=>i===index?step:old)});
  const add = (step:MacroStep) => {
    if(mapping.steps.length>=32)return;
    pendingFocus.current=`[data-step-input="${mapping.steps.length}"]`;
    onChange({...mapping,enabled:mapping.steps.length?mapping.enabled:true,steps:[...mapping.steps,step]});
  };
  const remove = (index:number) => {
    const steps=mapping.steps.filter((_,i)=>i!==index);
    pendingFocus.current=steps.length?`[data-step-input="${Math.min(index,steps.length-1)}"]`:'[data-add-step]';
    onChange({...mapping,steps,enabled:steps.length>0&&mapping.enabled});
    setRecording(null);
  };
  const move = (index:number,delta:number) => {
    const steps=[...mapping.steps];
    [steps[index],steps[index+delta]]=[steps[index+delta],steps[index]];
    pendingFocus.current=`[data-step-input="${index+delta}"]`;
    onChange({...mapping,steps});setRecording(null);
  };
  const error=mappingError(mapping);
  const nameError=actionNameError(mapping.name);
  const aggregateError=error&&!nameError&&!mapping.steps.some(step=>stepError(step))?error:null;
  return <section ref={editor} className="editor-canvas" aria-busy={saving} aria-label={'Macro for '+trigger?.label}>
    <fieldset className="editor-fields" disabled={saving}>
      <div className="editor-intro">
        <div className="eyebrow">Headphone button</div>
        <div className="trigger-chip"><TriggerIcon trigger={mapping.trigger} size={17}/>{trigger?.label ?? mapping.trigger}<ChevronRight size={16}/><span>{mapping.steps.length} {mapping.steps.length===1?'step':'steps'}</span></div>
        <div className="editor-name-row">
          <label className="field flex-1"><span>Action name</span><input ref={nameInput} aria-label="Action name" value={mapping.name} maxLength={80} aria-invalid={!!nameError} aria-describedby={nameError?id+'-name-error':undefined} onChange={event=>onChange({...mapping,name:event.target.value})} placeholder="Name this action"/>{nameError&&<span className="field-error" id={id+'-name-error'}>{nameError}</span>}</label>
          <label className="switch-label editor-enable" title={!mapping.steps.length?'Add a step to enable this mapping':undefined}><span>{mapping.enabled?'Enabled':'Disabled'}</span><input className="switch" type="checkbox" aria-label={'Enable '+trigger?.label+' mapping'} checked={mapping.enabled} disabled={!mapping.steps.length} onChange={event=>onChange({...mapping,enabled:event.target.checked})}/></label>
        </div>
      </div>
      <div className="steps-heading"><h3>Steps</h3><span>Run in order</span></div>
      <div className="macro-steps">
        {mapping.steps.map((step,index)=>{
          const fieldError=stepError(step);
          const errorId=id+'-step-'+index+'-error';
          const fieldProps={'data-step-input':index,'aria-invalid':!!fieldError,'aria-describedby':fieldError?errorId:undefined};
          return <div className={`macro-step ${recording===index?'recording':''}`} key={`${mapping.id}-${index}`}>
            <div className="step-number">{String(index+1).padStart(2,'0')}</div>
            <div className="step-content">
              <div className="step-top">
                <div className="step-kind">{step.kind==='key'?<Keyboard size={16}/>:step.kind==='delay'?<Clock3 size={16}/>:<Type size={16}/>}<strong>{step.kind==='key'?'Press a shortcut':step.kind==='delay'?'Wait':'Type text'}</strong></div>
                <div className="step-tools"><button className="icon-button small" title="Move step up" aria-label={`Move step ${index+1} up`} disabled={index===0} onClick={()=>move(index,-1)}><ArrowUp size={15}/></button><button className="icon-button small" title="Move step down" aria-label={`Move step ${index+1} down`} disabled={index===mapping.steps.length-1} onClick={()=>move(index,1)}><ArrowDown size={15}/></button><button className="icon-button small danger-hover" title="Remove step" aria-label={`Remove step ${index+1}`} onClick={()=>remove(index)}><Trash2 size={15}/></button></div>
              </div>
              {step.kind==='key'?<div className="shortcut-row">
                <input {...fieldProps} aria-label={`Shortcut for step ${index+1}`} value={step.keys} placeholder="For example, Ctrl+Shift+L" onChange={event=>updateStep(index,{kind:'key',keys:event.target.value})}/>
                <button className={`button compact ${recording===index?'record-button':''}`} aria-label={recording===index?`Cancel recording step ${index+1}`:`Record shortcut for step ${index+1}`} aria-pressed={recording===index} onClick={event=>{setRecording(recording===index?null:index);event.currentTarget.focus();}} onBlur={()=>setRecording(null)} onKeyDown={event=>{if(recording!==index)return;event.preventDefault();event.stopPropagation();if(event.repeat)return;const keys=captureShortcut(event);if(keys){updateStep(index,{kind:'key',keys});setRecording(null);}}}><Keyboard size={15}/>{recording===index?'Cancel recording':'Record'}</button>
              </div>:step.kind==='delay'?<div className="delay-row">
                <input {...fieldProps} type="number" min={0} max={30000} step={50} aria-label={`Delay for step ${index+1} in milliseconds`} value={step.ms} onChange={event=>updateStep(index,{kind:'delay',ms:Number(event.target.value)})}/><span>milliseconds</span><div className="delay-presets">{[250,500,1000].map(ms=><button key={ms} onClick={()=>updateStep(index,{kind:'delay',ms})}>{ms<1000?`${ms} ms`:'1 sec'}</button>)}</div>
              </div>:<textarea {...fieldProps} aria-label={`Text for step ${index+1}`} value={step.text} rows={3} onChange={event=>updateStep(index,{kind:'text',text:event.target.value})} placeholder="Text to type into the selected window"/>}
              {fieldError&&<p className="field-error" id={errorId}>{fieldError}</p>}
              {recording===index&&<p className="record-hint" role="status">Press a key or shortcut, including Escape. Click Cancel recording to stop.</p>}
            </div>
          </div>;
        })}
        {!mapping.steps.length&&<div className="empty-steps"><Plus size={25}/><h3>No steps yet</h3><p>Add a shortcut, delay, or text below.</p></div>}
      </div>
      <div className="add-step-row"><span>Add a step</span><button data-add-step className="button compact" disabled={mapping.steps.length>=32} onClick={()=>add({kind:'key',keys:'Right'})}><Keyboard size={15}/>Shortcut</button><button className="button compact" disabled={mapping.steps.length>=32} onClick={()=>add({kind:'delay',ms:500})}><Clock3 size={15}/>Delay</button><button className="button compact" disabled={mapping.steps.length>=32} onClick={()=>add({kind:'text',text:''})}><Type size={15}/>Text</button></div>
      {aggregateError&&<p className="inline-error" role="alert" tabIndex={-1} data-validation-summary>{aggregateError}</p>}
      <div className="macro-preview"><div className="eyebrow">Preview</div><div className="sequence-line">{mapping.steps.length?mapping.steps.map((step,index)=><span className="sequence-part" key={index}><kbd>{stepSummary(step)}</kbd>{index<mapping.steps.length-1&&<ChevronRight size={14}/>}</span>):<span className="muted">No steps yet</span>}</div></div>
      <footer className="editor-footer"><p>{!canTest?'Choose a target window above to test this macro.':invalidDraft?'Fix the draft issue before testing.':!mapping.enabled?'Enable this mapping to test it.':'Test saves changes and starts in 3 seconds. Switch to the target window.'}</p><button className="button" onClick={onTest} title={canTest?'Run this macro in the selected window after 3 seconds':'Choose a target window first'} disabled={invalidDraft||saving||!canTest||!mapping.enabled}><Play size={14}/>Test macro</button></footer>
    </fieldset>
  </section>;
}

import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronDown, Search } from 'lucide-react';

export interface SelectOption { value: string; label: string; description?: string }
interface SelectMenuProps {
  label: string;
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  placeholder?: string;
  searchable?: boolean;
  className?: string;
}

/** A themed listbox shared by profile, window, and activity selectors. */
export function SelectMenu({label,value,options,onChange,disabled=false,placeholder='Choose an option',searchable=false,className=''}: SelectMenuProps) {
  const id=useId();
  const trigger=useRef<HTMLButtonElement>(null);
  const popover=useRef<HTMLDivElement>(null);
  const listbox=useRef<HTMLDivElement>(null);
  const search=useRef<HTMLInputElement>(null);
  const typeahead=useRef({text:'',at:0});
  const [open,setOpen]=useState(false);
  const [query,setQuery]=useState('');
  const [active,setActive]=useState(0);
  const [position,setPosition]=useState({top:0,left:0,width:240,maxHeight:320,above:false});
  const selected=options.find(option=>option.value===value);
  const visible=options.filter(option=>(option.label+' '+(option.description??'')).toLocaleLowerCase().includes(query.toLocaleLowerCase()));
  const activeIndex=Math.max(0,Math.min(active,visible.length-1));
  const activeId=visible.length ? id+'-option-'+activeIndex : undefined;

  const close=(restoreFocus=false)=>{setOpen(false);if(restoreFocus)trigger.current?.focus();};
  const show=(edge?: 'first'|'last')=>{
    if(disabled)return;
    typeahead.current={text:'',at:0};
    setQuery('');
    setActive(edge==='first'?0:edge==='last'?options.length-1:Math.max(0,options.findIndex(option=>option.value===value)));
    setOpen(true);
  };
  const choose=(option:SelectOption)=>{onChange(option.value);close(true);};

  useLayoutEffect(()=>{
    if(!open)return;
    const place=()=>{
      const rect=trigger.current?.getBoundingClientRect();
      if(!rect)return;
      const margin=8,gap=6;
      const width=Math.min(Math.max(rect.width,240),window.innerWidth-margin*2);
      const below=window.innerHeight-rect.bottom-gap-margin;
      const above=rect.top-gap-margin;
      const downward=below>=200||below>=above;
      const maxHeight=Math.max(80,Math.min(320,downward?below:above));
      setPosition({left:Math.max(margin,Math.min(rect.left,window.innerWidth-width-margin)),top:downward?rect.bottom+gap:rect.top-gap,width,maxHeight,above:!downward});
    };
    place();
    window.addEventListener('resize',place);
    window.addEventListener('scroll',place,true);
    return()=>{window.removeEventListener('resize',place);window.removeEventListener('scroll',place,true);};
  },[open]);

  useEffect(()=>{
    if(!open)return;
    (searchable?search.current:listbox.current)?.focus();
    const dismiss=(event:PointerEvent|FocusEvent)=>{
      const node=event.target as Node;
      if(!trigger.current?.contains(node)&&!popover.current?.contains(node))setOpen(false);
    };
    document.addEventListener('pointerdown',dismiss);
    document.addEventListener('focusin',dismiss);
    return()=>{document.removeEventListener('pointerdown',dismiss);document.removeEventListener('focusin',dismiss);};
  },[open,searchable]);

  useEffect(()=>{if(disabled)setOpen(false);},[disabled]);
  useEffect(()=>{
    if(open)document.getElementById(id+'-option-'+activeIndex)?.scrollIntoView({block:'nearest'});
  },[open,activeIndex,id,query]);

  const navigate=(event:KeyboardEvent)=>{
    if(event.key==='Escape') {event.preventDefault();event.stopPropagation();close(true);return;}
    if(event.key==='Tab') {close(true);return;}
    if(event.key==='ArrowDown'||event.key==='ArrowUp') {
      event.preventDefault();
      setActive(index=>Math.max(0,Math.min(visible.length-1,index+(event.key==='ArrowDown'?1:-1))));
    } else if(event.key==='Home'||event.key==='End') {
      if(searchable&&event.target===search.current)return;
      event.preventDefault();setActive(event.key==='Home'?0:Math.max(0,visible.length-1));
    } else if(event.key==='Enter'||!searchable&&event.key===' ') {
      event.preventDefault();if(visible[activeIndex])choose(visible[activeIndex]);
    } else if(!searchable&&event.key.length===1&&!event.ctrlKey&&!event.altKey&&!event.metaKey) {
      event.preventDefault();
      const now=Date.now();
      const text=(now-typeahead.current.at<600?typeahead.current.text:'')+event.key.toLocaleLowerCase();
      typeahead.current={text,at:now};
      const index=visible.findIndex(option=>option.label.toLocaleLowerCase().startsWith(text));
      if(index>=0)setActive(index);
    }
  };

  return <div className={'select-menu '+className}>
    <button ref={trigger} type="button" className={'select-trigger '+(!selected?'placeholder':'')} aria-label={label} aria-describedby={id+'-value'} aria-haspopup="listbox" aria-expanded={open} aria-controls={open?id:undefined} disabled={disabled} title={selected?.label??placeholder} onClick={()=>open?close(true):show()} onKeyDown={event=>{
      if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();show(event.key==='Home'?'first':event.key==='End'?'last':undefined);}
    }}><span id={id+'-value'}>{selected?.label??placeholder}</span><ChevronDown size={14}/></button>
    {open&&createPortal(<div ref={popover} className="select-popover" style={{position:'fixed',top:position.top,left:position.left,width:position.width,maxHeight:position.maxHeight,transform:position.above?'translateY(-100%)':undefined,zIndex:60}} onKeyDown={navigate}>
      {searchable&&<div className="select-search"><Search size={14}/><input ref={search} role="combobox" aria-label={'Search '+label.toLocaleLowerCase()} aria-controls={id} aria-expanded="true" aria-autocomplete="list" aria-activedescendant={activeId} value={query} placeholder="Search windows…" onChange={event=>{setQuery(event.target.value);setActive(0);}}/></div>}
      <div ref={listbox} id={id} className="select-options" role="listbox" aria-label={label} tabIndex={searchable?undefined:-1} aria-activedescendant={searchable?undefined:activeId}>
        {visible.map((option,index)=><div key={option.value} id={id+'-option-'+index} role="option" aria-selected={option.value===value} className={'select-option '+(index===activeIndex?'highlighted ':'')+(option.value===value?'selected':'')} onPointerMove={()=>setActive(index)} onMouseDown={event=>event.preventDefault()} onClick={()=>choose(option)}>
          <span className="select-option-copy"><span>{option.label}</span>{option.description&&<small>{option.description}</small>}</span>{option.value===value&&<Check size={14}/>}
        </div>)}
        {!visible.length&&<div className="select-empty" role="status">{query?'No matching windows':'No options available'}</div>}
      </div>
    </div>,document.body)}
  </div>;
}

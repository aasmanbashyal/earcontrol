import { useEffect, useRef, type ReactNode } from 'react';
import { X } from 'lucide-react';

export function Modal({title,children,onClose,wide=false,busy=false}: {title:string;children:ReactNode;onClose:()=>void;wide?:boolean;busy?:boolean}) {
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  const busyRef = useRef(busy);
  closeRef.current = onClose;
  busyRef.current = busy;
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const focusable = 'button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),[tabindex="0"]';
    const first = ref.current?.querySelector<HTMLElement>('[data-autofocus]') ?? ref.current?.querySelector<HTMLElement>(focusable);
    (first ?? ref.current)?.focus();
    const handle = (event:KeyboardEvent) => {
      if(event.defaultPrevented) return;
      if(event.key === 'Escape') { event.preventDefault(); if(!busyRef.current) closeRef.current(); }
      if(event.key !== 'Tab') return;
      const nodes = Array.from(ref.current?.querySelectorAll<HTMLElement>(focusable) ?? []).filter(node=>node.getClientRects().length>0);
      if(!nodes.length) { event.preventDefault(); ref.current?.focus(); return; }
      const firstNode=nodes[0],lastNode=nodes[nodes.length-1];
      if(!ref.current?.contains(document.activeElement)) { event.preventDefault(); firstNode.focus(); }
      else if(event.shiftKey && (document.activeElement===firstNode||document.activeElement===ref.current)) { event.preventDefault(); lastNode.focus(); }
      else if(!event.shiftKey && (document.activeElement===lastNode||document.activeElement===ref.current)) { event.preventDefault(); firstNode.focus(); }
    };
    document.addEventListener('keydown',handle);
    return () => { document.removeEventListener('keydown',handle); previous?.focus(); };
  },[]);
  return <div className="modal-scrim" onMouseDown={event=>{if(!busy&&event.target===event.currentTarget)onClose();}}><div ref={ref} className={`modal ${wide?'modal-wide':''}`} role="dialog" aria-modal="true" aria-label={title} aria-busy={busy} tabIndex={-1}><header className="modal-heading"><h2>{title}</h2><button className="icon-button" aria-label="Close dialog" disabled={busy} onClick={onClose}><X size={19}/></button></header>{children}</div></div>;
}

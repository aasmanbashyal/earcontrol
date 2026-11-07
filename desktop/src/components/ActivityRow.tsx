import { Check, CircleHelp, Radio } from 'lucide-react';
import type { ActivityEvent } from '../types';
import { TRIGGERS } from '../types';

export function ActivityRow({event,full=false}:{event:ActivityEvent;full?:boolean}) {
  const label=TRIGGERS.find(item=>item.id===event.trigger)?.label ?? event.trigger;
  const isError=/error|fail|cancelled|skipped/i.test(event.outcome);
  const isAction=/sent|complete|started|executed/i.test(event.outcome);
  return <div className={'activity-row '+(full?'full':'')}><span className={'event-icon '+(isError?'error':isAction?'success':'')}>{isError?<CircleHelp size={16}/>:isAction?<Check size={16}/>:<Radio size={16}/>}</span><div className="event-content"><strong>{label||'Controller'}</strong><p>{event.detail}</p>{full&&<small>{event.source}</small>}</div><span className={'event-outcome '+(isError?'error':isAction?'success':'')}>{event.outcome}</span><time dateTime={new Date(event.timestamp).toISOString()}>{new Date(event.timestamp).toLocaleTimeString([],{hour:'2-digit',minute:'2-digit',second:'2-digit'})}</time></div>;
}

import type { ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';

export function EmptyState({icon:Icon,title,detail,children}:{icon:LucideIcon;title:string;detail:string;children?:ReactNode}) {
  return <div className="empty-state"><div className="empty-icon"><Icon size={27}/></div><h3>{title}</h3><p>{detail}</p>{children}</div>;
}

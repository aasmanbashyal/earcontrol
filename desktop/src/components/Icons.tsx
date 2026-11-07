import { CircleHelp, SkipBack, SkipForward, Square, Volume1, Volume2, VolumeX, createLucideIcon } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { Trigger } from '../types';

// Lucide 0.468 has separate Play and Pause icons, but no combined media command.
const PlayPause = createLucideIcon('PlayPause', [
  ['path', { d: 'm3 4 10 8-10 8V4Z', key: 'play' }],
  ['path', { d: 'M17 4v16M21 4v16', key: 'pause' }],
]);

const triggerIcons: Record<Trigger, LucideIcon> = {
  play_pause: PlayPause,
  next: SkipForward,
  previous: SkipBack,
  stop: Square,
  volume_up: Volume2,
  volume_down: Volume1,
  mute: VolumeX,
};

export function TriggerIcon({ trigger, size = 16, className }: { trigger: string; size?: number; className?: string }) {
  const Icon = triggerIcons[trigger as Trigger] ?? CircleHelp;
  return <Icon size={size} strokeWidth={1.75} className={className} aria-hidden="true"/>;
}

export function BrandIcon({ size = 24, className }: { size?: number; className?: string }) {
  return <svg width={size} height={size} viewBox="120 170 975 975" className={className} fill="currentColor" aria-hidden="true">
    <path fillRule="evenodd" d="M430 349c48-2 110 27 141 58 29 29 22 86 8 131-17 61-65 130-113 152-20 9-33 10-52 5-47-12-105-43-130-84-24-38-20-82-6-125 23-72 86-137 152-137Zm-44 52c24-1 29 27 23 57-8 44-33 93-64 112-20 14-39 8-44-16-10-36 11-89 36-124 18-23 38-30 49-29Z"/>
    <path d="M614 430c96-40 197-7 266 66 73 75 87 159 44 264-38 87-110 155-192 188-79 35-165 17-225-22-53-38-69-97-55-180l3-24c71-18 120-62 148-117 27-56 30-115 11-175Z"/>
  </svg>;
}

export type Trigger = 'play_pause' | 'next' | 'previous' | 'stop' | 'volume_up' | 'volume_down' | 'mute';
export type MacroStep = { kind: 'key'; keys: string } | { kind: 'delay'; ms: number } | { kind: 'text'; text: string };
export interface Mapping { id: string; trigger: Trigger; name: string; enabled: boolean; steps: MacroStep[] }
export interface Profile { id: string; name: string; description: string; accent: string; mappings: Mapping[] }
export interface Settings { version: number; activeProfileId: string; profiles: Profile[]; keyboardFallback: boolean; cooldownMs: number; theme: 'light' | 'dark' | 'system' }
export interface TargetWindow { handle: number; processId: number; title: string }
export interface DeviceInfo { id: string; name: string; connected: boolean; transport: string; kind: string }
export interface ActivityEvent { id: number; timestamp: number; trigger: string; source: string; outcome: string; detail: string }
export interface RuntimeStatus { mode: 'idle' | 'monitor' | 'control'; target: TargetWindow | null; runningMacro: boolean; mediaReady: boolean; message: string }
export interface Bootstrap { settings: Settings; status: RuntimeStatus; version: string; warning: string | null }

export const TRIGGERS: { id: Trigger; label: string }[] = [
  { id: 'play_pause', label: 'Play / pause' },
  { id: 'next', label: 'Next track' },
  { id: 'previous', label: 'Previous track' },
  { id: 'stop', label: 'Stop playback' },
  { id: 'volume_up', label: 'Volume up' },
  { id: 'volume_down', label: 'Volume down' },
  { id: 'mute', label: 'Mute' },
];

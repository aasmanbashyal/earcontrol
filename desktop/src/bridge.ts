import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { version as appVersion } from '../package.json';
import type { ActivityEvent, Bootstrap, DeviceInfo, RuntimeStatus, Settings, TargetWindow } from './types';
import { loadPreviewSettings, previewDefaults, savePreviewSettings } from './preview';

export const isDesktop = () => isTauri();
const requireDesktop = () => { if (!isDesktop()) throw new Error('Open the EarControl desktop app to use this control.'); };
const previewStatus = (): RuntimeStatus => ({ mode: 'idle', target: null, runningMacro: false, mediaReady: false, message: 'Browser preview. Headphone capture and shortcuts require the desktop app.' });
const practiceTarget: TargetWindow = { handle: -1, processId: 0, title: 'Practice deck · browser preview' };
let practiceWindow: Window | null = null;

export async function getBootstrap(): Promise<Bootstrap> {
  if (isDesktop()) return invoke('get_bootstrap');
  let settings = previewDefaults();
  try { settings = loadPreviewSettings(window.localStorage); } catch { /* Preview still opens when browser storage is blocked. */ }
  return {
    version: appVersion, warning: 'Browser preview. Saved changes stay in this browser and do not change the desktop app.',
    settings, status: previewStatus(),
  };
}
export async function listWindows(): Promise<TargetWindow[]> { return isDesktop() ? invoke('list_windows') : practiceWindow && !practiceWindow.closed ? [{...practiceTarget}] : []; }
export async function listDevices(): Promise<DeviceInfo[]> { return isDesktop() ? invoke('list_devices') : []; }
export async function saveSettings(settings: Settings): Promise<Settings> {
  if (isDesktop()) return invoke('save_settings', { settings });
  return savePreviewSettings(settings, window.localStorage);
}
export async function setMode(mode: RuntimeStatus['mode'], target: TargetWindow | null): Promise<RuntimeStatus> {
  if (isDesktop()) return invoke('set_mode', { mode, target });
  if (mode === 'idle') return previewStatus();
  throw new Error('Headphone monitoring and control require the desktop app. Browser preview can edit and save its own settings.');
}
export async function getStatus(): Promise<RuntimeStatus> { return isDesktop() ? invoke('get_status') : previewStatus(); }
export async function openBluetoothSettings(): Promise<void> { requireDesktop(); return invoke('open_bluetooth_settings'); }
export async function openPractice(): Promise<TargetWindow> {
  if (isDesktop()) return invoke('open_practice');
  practiceWindow = window.open(new URL('practice.html', window.location.href).href, 'earcontrol-practice-preview');
  if (!practiceWindow) throw new Error('The browser blocked the practice deck. Allow pop-ups for this preview and try again.');
  return {...practiceTarget};
}
export async function previewMacro(mappingId: string, target: TargetWindow): Promise<void> { requireDesktop(); return invoke('preview_macro', { mappingId, target }); }
export async function subscribeActivity(callback: (event: ActivityEvent) => void): Promise<() => void> {
  return isDesktop() ? listen<ActivityEvent>('activity', event => callback(event.payload)) : () => {};
}
export async function subscribeStatus(callback: (status: RuntimeStatus) => void): Promise<() => void> {
  return isDesktop() ? listen<RuntimeStatus>('status', event => callback(event.payload)) : () => {};
}

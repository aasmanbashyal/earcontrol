import { useCallback, useEffect, useRef, useState } from 'react';
import { Activity, Archive, ArrowRight, Bluetooth, Check, CheckCircle2, ChevronRight, CircleHelp, Copy, Download, ExternalLink, Headphones, Keyboard, ListOrdered, LoaderCircle, Monitor, Moon, MoreHorizontal, Palette, Play, Plus, Radio, RefreshCw, Save, Settings2, SlidersHorizontal, Square, Sun, Trash2, Upload, X } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import * as bridge from './bridge';
import type { ActivityEvent, DeviceInfo, Mapping, Profile, RuntimeStatus, Settings, TargetWindow } from './types';
import { TRIGGERS } from './types';
import { version as appVersion } from '../package.json';
import { Modal } from './components/Modal';
import { MacroEditor } from './components/MacroEditor';
import { BrandIcon, TriggerIcon } from './components/Icons';
import { RepeatDelay } from './components/RepeatDelay';
import { SelectMenu } from './components/SelectMenu';
import { WindowChrome } from './components/WindowChrome';
import { mappingSummary, readImportedSettings, settingsIssue } from './components/ui';
import { applyTheme, LIVE_THEME_KEY, SAVED_THEME_KEY } from './theme';
import { ActivityRow } from './components/ActivityRow';
import { EmptyState } from './components/EmptyState';
import './styles.css';

type View = 'mappings'|'macros'|'headphones'|'activity'|'settings';
const navigation: { id:View; title:string; icon:LucideIcon }[] = [
  {id:'mappings',title:'Buttons',icon:SlidersHorizontal}, {id:'macros',title:'Macros',icon:ListOrdered}, {id:'headphones',title:'Devices',icon:Headphones}, {id:'activity',title:'Activity',icon:Activity}, {id:'settings',title:'Settings',icon:Settings2},
];
const titles:Record<View,{title:string;description:string}> = {
  mappings:{title:'Button mappings',description:'Choose what your headphone buttons do.'},
  macros:{title:'Macros',description:'Edit the steps a button runs.'},
  headphones:{title:'Devices',description:'Bluetooth audio devices paired with this PC.'},
  activity:{title:'Activity',description:'See which commands arrive and what happens next.'},
  settings:{title:'Settings',description:'Appearance, input behavior, and backups.'},
};
const idle:RuntimeStatus={mode:'idle',target:null,runningMacro:false,mediaReady:false,message:'Ready when you are'};
const clone=<T,>(value:T):T=>JSON.parse(JSON.stringify(value)) as T;
const errorMessage=(error:unknown)=>error instanceof Error?error.message:String(error);
const newId=()=>crypto.randomUUID();


export default function App() {
  const [view,setView]=useState<View>('mappings');
  const [settings,setSettings]=useState<Settings|null>(null);
  const [saved,setSaved]=useState<Settings|null>(null);
  const [status,setStatus]=useState<RuntimeStatus>(idle);
  const [version,setVersion]=useState(appVersion);
  const [devices,setDevices]=useState<DeviceInfo[]>([]);
  const [windows,setWindows]=useState<TargetWindow[]>([]);
  const [target,setTarget]=useState<TargetWindow|null>(null);
  const [events,setEvents]=useState<ActivityEvent[]>([]);
  const [eventsFilter,setEventsFilter]=useState('all');
  const [devicesLoading,setDevicesLoading]=useState(true);
  const [volumeOpen,setVolumeOpen]=useState(false);
  const [windowsLoading,setWindowsLoading]=useState(false);
  const [busy,setBusy]=useState(false);
  const [fatal,setFatal]=useState<string|null>(null);
  const [deviceError,setDeviceError]=useState<string|null>(null);
  const [toast,setToast]=useState<{message:string;error:boolean}|null>(null);
  const [selectedMapping,setSelectedMapping]=useState<string|null>(null);
  const [profilesOpen,setProfilesOpen]=useState(false);
  const [deleteProfileId,setDeleteProfileId]=useState<string|null>(null);
  const [importOpen,setImportOpen]=useState<Settings|null>(null);
  const [issueReview,setIssueReview]=useState(0);
  const importRef=useRef<HTMLInputElement>(null);
  const deviceScan=useRef({busy:false,last:0});
  const deviceViewMounted=useRef(false);
  const notify=useCallback((message:string,error=false)=>setToast({message,error}),[]);
  const closeProfiles=useCallback(()=>setProfilesOpen(false),[]);
  const closeImport=useCallback(()=>setImportOpen(null),[]);
  const dirty=!!settings&&!!saved&&JSON.stringify(settings)!==JSON.stringify(saved);
  const draftIssue=settings?settingsIssue(settings):null;
  const profile=settings?.profiles.find(item=>item.id===settings.activeProfileId) ?? settings?.profiles[0];
  const connected=devices.filter(device=>device.connected);
  const enabledMappings=profile?.mappings.filter(mapping=>mapping.enabled) ?? [];
  const mapping=profile?.mappings.find(item=>item.id===selectedMapping) ?? profile?.mappings[0];

  useEffect(()=>{
    let alive=true;
    deviceViewMounted.current=true;
    const unsubscribers:(()=>void)[]=[];
    bridge.getBootstrap().then(data=>{if(!alive)return;setSettings(data.settings);setSaved(clone(data.settings));setStatus(data.status);setTarget(data.status.target);setVersion(data.version);if(data.warning&&bridge.isDesktop())notify(data.warning,true);}).catch(error=>{if(alive)setFatal(errorMessage(error));});
    const scanDevices=()=>{
      if(document.hidden||deviceScan.current.busy||Date.now()-deviceScan.current.last<3000)return;
      deviceScan.current={busy:true,last:Date.now()};
      setDevicesLoading(true);setDeviceError(null);
      bridge.listDevices().then(data=>{if(deviceViewMounted.current)setDevices(data);}).catch(error=>{if(deviceViewMounted.current)setDeviceError(errorMessage(error));}).finally(()=>{deviceScan.current.busy=false;if(deviceViewMounted.current)setDevicesLoading(false);});
    };
    scanDevices();
    window.addEventListener('focus',scanDevices);
    document.addEventListener('visibilitychange',scanDevices);
    bridge.listWindows().then(data=>{if(alive)setWindows(data);}).catch(error=>{if(alive)notify(errorMessage(error),true);});
    bridge.subscribeActivity(event=>{if(alive)setEvents(previous=>[event,...previous].slice(0,250));}).then(unsubscribe=>{if(alive)unsubscribers.push(unsubscribe);else unsubscribe();}).catch(error=>{if(alive)notify(`Activity connection failed: ${errorMessage(error)}`,true);});
    bridge.subscribeStatus(next=>{if(alive){setStatus(next);if(next.mode==='control')setTarget(next.target);}}).then(unsubscribe=>{if(alive)unsubscribers.push(unsubscribe);else unsubscribe();}).catch(error=>{if(alive)notify(`Status connection failed: ${errorMessage(error)}`,true);});
    return()=>{alive=false;deviceViewMounted.current=false;window.removeEventListener('focus',scanDevices);document.removeEventListener('visibilitychange',scanDevices);unsubscribers.forEach(unsubscribe=>unsubscribe());};
  },[notify]);

  useEffect(()=>{if(!toast)return;const timer=setTimeout(()=>setToast(null),toast.error?8000:4000);return()=>clearTimeout(timer);},[toast]);
  useEffect(()=>{
    const query=window.matchMedia('(prefers-color-scheme: dark)');
    const apply=()=>{
      if(!settings)return;
      applyTheme(settings.theme,query.matches);
      try {
        localStorage.setItem(LIVE_THEME_KEY,settings.theme);
        if(saved)localStorage.setItem(SAVED_THEME_KEY,saved.theme);
      } catch {/* Theme still applies when browser storage is unavailable. */}
    };
    apply();query.addEventListener('change',apply);return()=>query.removeEventListener('change',apply);
  },[settings?.theme,saved?.theme]);
  useEffect(()=>{window.scrollTo(0,0);},[view,selectedMapping,settings?.activeProfileId]);
  useEffect(()=>{
    if(!issueReview)return;
    const frame=requestAnimationFrame(()=>{
      const scope=document.querySelector(profilesOpen?'[role="dialog"][aria-label="Profiles"]':'.editor-canvas');
      const field=scope?.querySelector<HTMLElement>('[aria-invalid="true"], [data-validation-summary]');
      field?.focus({preventScroll:true});
      field?.scrollIntoView({block:'center'});
      setIssueReview(0);
    });
    return()=>cancelAnimationFrame(frame);
  },[issueReview,view,selectedMapping,profilesOpen]);

  const updateSettings=(updater:(current:Settings)=>Settings)=>{
    if(status.mode==='control'||status.runningMacro)void bridge.setMode('idle',null).then(setStatus).catch(error=>notify(errorMessage(error),true));
    setSettings(current=>current?updater(current):current);
  };
  const updateProfile=(next:Profile)=>updateSettings(current=>({...current,profiles:current.profiles.map(item=>item.id===next.id?next:item)}));
  const updateMapping=(next:Mapping)=>{if(profile)updateProfile({...profile,mappings:profile.mappings.map(item=>item.id===next.id?next:item)});};
  const reviewIssue=(issue=draftIssue)=>{
    if(!issue)return;
    updateSettings(current=>({...current,activeProfileId:issue.profileId}));
    if(issue.mappingId){setSelectedMapping(issue.mappingId);setView('macros');setProfilesOpen(false);}
    else setProfilesOpen(true);
    setIssueReview(value=>value+1);
  };
  const persist=async():Promise<Settings>=>{
    if(!settings)throw new Error('Settings have not loaded.');
    const issue=settingsIssue(settings);
    if(issue){reviewIssue(issue);throw new Error(issue.message);}
    const next=await bridge.saveSettings(settings);
    setSettings(next);setSaved(clone(next));
    setStatus(await bridge.getStatus());
    return next;
  };
  const save=async()=>{setBusy(true);try{await persist();notify('Your changes are saved.');}catch(error){notify(errorMessage(error),true);}finally{setBusy(false);}};
  const mode=async(next:'idle'|'monitor'|'control')=>{
    if(next==='control'&&!target){notify('Choose the window you want to control first.',true);return;}
    setBusy(true);try{if(next!=='idle'&&dirty)await persist();setStatus(await bridge.setMode(next,next==='control'?target:null));if(next==='control')notify('Control is on. Switch to your chosen window.');if(next==='monitor')setView('activity');}catch(error){notify(errorMessage(error),true);}finally{setBusy(false);}
  };
  const refreshWindows=async()=>{setWindowsLoading(true);try{const next=await bridge.listWindows();setWindows(next);setTarget(old=>old?next.find(item=>item.handle===old.handle&&item.processId===old.processId)??null:null);}catch(error){notify(errorMessage(error),true);}finally{setWindowsLoading(false);}};
  const refreshDevices=async()=>{if(deviceScan.current.busy)return;deviceScan.current={busy:true,last:Date.now()};setDevicesLoading(true);setDeviceError(null);try{setDevices(await bridge.listDevices());}catch(error){setDeviceError(errorMessage(error));}finally{deviceScan.current.busy=false;setDevicesLoading(false);}};
  const practice=async()=>{setBusy(true);try{setStatus(await bridge.setMode('idle',null));const next=await bridge.openPractice();setTarget(next);setWindows(previous=>[next,...previous.filter(item=>item.handle!==next.handle)]);notify(bridge.isDesktop()?'Practice deck selected. Start control, then switch to the deck.':'Practice deck opened. Use your keyboard to try its navigation.');}catch(error){notify(errorMessage(error),true);}finally{setBusy(false);}};
  const testMacro=async()=>{if(!mapping||!target)return;setBusy(true);try{if(dirty)await persist();await bridge.previewMacro(mapping.id,target);setStatus(await bridge.getStatus());notify('Test starts in 3 seconds. Switch to the chosen window.');}catch(error){notify(errorMessage(error),true);}finally{setBusy(false);}};
  const bluetooth=()=>{void bridge.openBluetoothSettings().catch(error=>notify(errorMessage(error),true));};
  const editMapping=(next:Mapping)=>{setSelectedMapping(next.id);setView('macros');};
  const addProfile=(duplicate=false)=>{
    if(!settings||!profile||settings.profiles.length>=32)return;
    const id=newId();
    const next:Profile=duplicate?{...clone(profile),id,name:`${profile.name.slice(0,70)} copy`,mappings:profile.mappings.map(item=>({...clone(item),id:newId()}))}:{id,name:'New profile',description:'Your own set of headphone controls.',accent:'#a64619',mappings:TRIGGERS.map(trigger=>({id:newId(),trigger:trigger.id,name:trigger.label,enabled:false,steps:[]}))};
    updateSettings(current=>({...current,activeProfileId:id,profiles:[...current.profiles,next]}));
  };
  const removeProfile=(id:string)=>{
    if(!settings||settings.profiles.length<=1)return;
    updateSettings(current=>{const profiles=current.profiles.filter(item=>item.id!==id);return{...current,profiles,activeProfileId:current.activeProfileId===id?profiles[0].id:current.activeProfileId};});setDeleteProfileId(null);
  };
  const exportSettings=async()=>{
    setBusy(true);try{const next=dirty?await persist():settings;if(!next)return;const blob=new Blob([JSON.stringify(next,null,2)],{type:'application/json'});const url=URL.createObjectURL(blob);const anchor=document.createElement('a');anchor.href=url;anchor.download='earcontrol-profiles.json';anchor.click();setTimeout(()=>URL.revokeObjectURL(url),1000);notify('Export download requested.');}catch(error){notify(errorMessage(error),true);}finally{setBusy(false);}
  };
  const importSettings=async()=>{
    if(!importOpen)return;setBusy(true);try{const next=await bridge.saveSettings(importOpen);setSettings(next);setSaved(clone(next));setStatus(await bridge.getStatus());setImportOpen(null);notify('Profiles and settings imported.');}catch(error){notify(errorMessage(error),true);}finally{setBusy(false);}
  };

  if(fatal)return <><WindowChrome title="EarControl" icon={<BrandIcon size={16}/>}/><div className="startup"><BrandIcon size={36}/><h1>EarControl</h1><h2>Desktop connection required</h2><p>{fatal}</p><p className="muted">Open the installed Windows app to connect your headphones and run macros.</p><button className="button primary" onClick={()=>location.reload()}><RefreshCw size={16}/>Try again</button></div></>;
  if(!settings||!profile)return <><WindowChrome title="EarControl" icon={<BrandIcon size={16}/>}/><div className="startup"><BrandIcon size={36}/><h1>EarControl</h1><LoaderCircle size={24} className="spin"/><p>Getting your controls ready…</p></div></>;

  const visibleEvents=events.filter(event=>eventsFilter==='all'||(eventsFilter==='errors'?/error|fail|cancelled|skipped/i.test(event.outcome):eventsFilter==='actions'?/sent|complete|started|executed/i.test(event.outcome):/detect|monitor/i.test(event.outcome)));
  const sessionActive=status.mode!=='idle'||status.runningMacro;
  const lastInput=events.find(event=>TRIGGERS.some(trigger=>trigger.id===event.trigger));
  const mainMappings=profile.mappings.filter(item=>!['volume_up','volume_down','mute'].includes(item.trigger));
  const volumeMappings=profile.mappings.filter(item=>['volume_up','volume_down','mute'].includes(item.trigger));
  const renderMapping=(item:Mapping)=><div className={'mapping-row '+(!item.enabled?'mapping-off':'')} key={item.id}>
    <div className="mapping-command"><span className="command-icon"><TriggerIcon trigger={item.trigger} size={18}/></span><strong>{TRIGGERS.find(trigger=>trigger.id===item.trigger)?.label}</strong></div>
    <button className="action-editor-link" aria-label={'Edit '+TRIGGERS.find(trigger=>trigger.id===item.trigger)?.label+': '+(item.steps.length?item.name:'Add an action')} title={item.steps.length?mappingSummary({...item,enabled:true}):'Choose a shortcut or macro'} onClick={()=>editMapping(item)}><div><strong>{item.steps.length?item.name:'Add an action'}</strong>{item.steps.length===1&&item.steps[0].kind==='key'?<span className="mapping-shortcut"><kbd>{item.steps[0].keys}</kbd></span>:<p>{item.steps.length?mappingSummary({...item,enabled:true}):'Choose a shortcut or macro'}</p>}</div><ChevronRight size={16}/></button>
    <input className="switch" type="checkbox" aria-label={'Enable '+TRIGGERS.find(trigger=>trigger.id===item.trigger)?.label} checked={item.enabled} onChange={event=>{if(event.target.checked&&!item.steps.length){editMapping(item);return;}updateMapping({...item,enabled:event.target.checked});}}/>
  </div>;

  const currentNavigation=navigation.find(item=>item.id===view)!;
  const PageIcon=currentNavigation.icon;
  return <div className="app-shell">
    <aside className="sidebar">
      <a className="brand" href="#buttons" onClick={event=>{event.preventDefault();setView('mappings');}}><BrandIcon size={25}/><span>EarControl</span></a>
      <div className="sidebar-profile"><span className="field-caption">Profile</span><div className="profile-picker"><SelectMenu label="Active profile" value={settings.activeProfileId} options={settings.profiles.map(item=>({value:item.id,label:item.name}))} disabled={busy} onChange={activeProfileId=>updateSettings(current=>({...current,activeProfileId}))}/><button className="icon-button" disabled={busy} onClick={()=>setProfilesOpen(true)} title="Manage profiles" aria-label="Manage profiles"><MoreHorizontal size={17}/></button></div></div>
      <nav aria-label="Main navigation">{navigation.map(item=><button key={item.id} className={'nav-item '+(view===item.id?'active':'')} aria-current={view===item.id?'page':undefined} onClick={()=>setView(item.id)}><item.icon size={17}/><span>{item.title}</span>{item.id==='activity'&&events.length>0&&<span className="nav-count">{events.length}</span>}</button>)}</nav>
      <div className="sidebar-bottom"><button className="sidebar-device" onClick={()=>setView('headphones')}><span className={'status-dot '+(connected.length?'connected':'')}/><span>{devicesLoading?'Checking devices':deviceError?'Device status unavailable':connected.length+' connected '+(connected.length===1?'device':'devices')}</span><ChevronRight size={14}/></button></div>
    </aside>
    <main className="workspace">
      <header className="session-bar" aria-label="Control session" data-tauri-drag-region={bridge.isDesktop()?'deep':undefined}>
        <div className="target-field" data-tauri-drag-region="false"><span className="field-caption">Target window</span><div className="target-control"><Monitor size={17}/><SelectMenu label="Target window" placeholder="Choose a window to control" value={target?target.handle+':'+target.processId:''} options={windows.map(item=>({value:item.handle+':'+item.processId,label:item.title}))} disabled={busy||status.mode==='control'||status.runningMacro} searchable onChange={value=>setTarget(windows.find(item=>item.handle+':'+item.processId===value)??null)}/><button className="icon-button" title="Refresh windows" aria-label="Refresh target windows" disabled={busy||windowsLoading||status.mode==='control'||status.runningMacro} onClick={()=>void refreshWindows()}><RefreshCw size={15} className={windowsLoading?'spin':''}/></button></div></div>
        <div className="session-actions" data-tauri-drag-region="false"><span className={'mode-label '+(status.runningMacro?'running':status.mode)}><i className="status-dot"/>{status.runningMacro?'Running macro':status.mode==='control'?'Controlling':status.mode==='monitor'?'Monitoring':'Idle'}</span>{sessionActive?<button className="button stop" onClick={()=>void mode('idle')} disabled={busy}><Square size={14}/>Stop</button>:<><button className="button" title="Listen without sending keys" onClick={()=>void mode('monitor')} disabled={busy}><Radio size={15}/>Monitor</button><button className="button primary" title={draftIssue?'Review the draft issue before starting':!target?'Choose a target window first':'Start sending mapped actions'} onClick={()=>void mode('control')} disabled={busy||!target||!!draftIssue}>{busy?<LoaderCircle className="spin" size={15}/>:<Play size={14}/>}<span>{dirty?'Save & start':'Start control'}</span></button></>}</div>
        <WindowChrome title={currentNavigation.title} variant="controls" onError={message=>notify(message,true)}/>
      </header>
      <div className="session-help" role="status">{status.mode==='control'?'Switch to the target window. Actions run only while it is in front.':status.mode==='monitor'?'Press a headphone button to see its command. Monitoring does not send keys.':status.runningMacro?'Switch to the target window. Stop cancels the test.':!target?'Choose an open window above, or open the practice deck to try your mappings.':'Ready. Start control, then switch to the target window.'}</div>
      <div className="page-content">
        {!bridge.isDesktop()&&<div className="preview-notice"><span className="status-dot"/><span>UI preview · Changes save in this browser. Device controls require the desktop app.</span></div>}
        <div className="page-heading"><div><h1><PageIcon size={20}/>{titles[view].title}</h1><p>{titles[view].description}</p></div><button className="button compact" onClick={()=>void practice()} disabled={busy||status.runningMacro}><Monitor size={15}/>Practice deck<ExternalLink size={12}/></button></div>
        <fieldset className="workspace-fields" disabled={busy}>
          {view==='mappings'&&<>
            <section className="panel mappings-panel"><div className="panel-heading"><div><h2>{profile.name}<span className="count-badge">{enabledMappings.length} of {profile.mappings.length} enabled</span></h2>{profile.description&&<p>{profile.description}</p>}</div><button className="icon-button" onClick={()=>setProfilesOpen(true)} aria-label="Manage profiles" title="Manage profiles"><MoreHorizontal size={18}/></button></div><div className="mapping-columns"><span>Headphone button</span><span>Action / shortcut</span><span>On</span></div>{mainMappings.map(renderMapping)}<button className="volume-toggle" aria-expanded={volumeOpen} onClick={()=>setVolumeOpen(!volumeOpen)}><ChevronRight size={14} style={{transform:volumeOpen?'rotate(90deg)':undefined}}/><span>Volume controls</span><span>{volumeMappings.filter(item=>item.enabled).length} enabled</span></button>{volumeOpen&&<><p className="panel-note">Requires keyboard media-key events. Keep keyboard media keys enabled in Settings.</p>{volumeMappings.map(renderMapping)}</>}</section>
            <div className="below-panel"><p>This profile applies to all devices. Monitor identifies each button's command.</p><button className="text-button" onClick={()=>setView('headphones')}>View devices<ArrowRight size={14}/></button></div>
            <section className="last-input"><div className="panel-heading"><h2>Button check</h2><button className="text-button" onClick={()=>setView('activity')}>View activity<ArrowRight size={13}/></button></div>{lastInput?<ActivityRow event={lastInput}/>:<div className="quiet-empty"><span className="feedback-icon"><Radio size={21}/></span><div><h3>{status.mode==='monitor'?'Waiting for a headphone command':'Check what your buttons send'}</h3><p>{status.mode==='monitor'?'Press a headphone button. Its command will appear here.':'Monitor listens for commands without sending shortcuts.'}</p></div><button className="button" disabled={busy||status.mode==='monitor'} onClick={()=>void mode('monitor')}><Radio size={15}/>{status.mode==='monitor'?'Listening':'Monitor'}</button></div>}</section>
            <details className="help-details"><summary><ChevronRight size={14} className="disclosure-chevron" aria-hidden="true"/><span>Getting started</span></summary><ol><li>Pair your headphones with this PC in <button className="text-button" onClick={bluetooth}>Bluetooth settings</button>.</li><li>Start Monitor and try each gesture to identify its command.</li><li>Choose a target window, start control, then switch to that window.</li></ol><p>Windows reports media commands, not tap counts or the earbud side. Browser shortcuts go to the focused tab or field.</p></details>
          </>}

          {view==='macros'&&mapping&&<div className="studio-layout"><aside className="studio-sidebar"><h2 className="section-label">Button</h2>{profile.mappings.map(item=><button key={item.id} className={'studio-mapping '+(mapping.id===item.id?'active':'')} aria-pressed={mapping.id===item.id} onClick={()=>setSelectedMapping(item.id)}><TriggerIcon trigger={item.trigger} size={16}/><span><strong>{TRIGGERS.find(trigger=>trigger.id===item.trigger)?.label}</strong><small>{item.enabled?item.steps.length+' '+(item.steps.length===1?'step':'steps'):'Disabled'}</small></span></button>)}</aside><MacroEditor key={mapping.id} mapping={mapping} onChange={updateMapping} onTest={()=>void testMacro()} canTest={!!target&&!status.runningMacro} invalidDraft={!!draftIssue} saving={busy}/></div>}

          {view==='headphones'&&<>
            <section className="panel"><div className="panel-heading"><div><h2>Paired audio devices</h2><p>{deviceError&&!devices.length?'Scan unavailable':<>{deviceError?'Last known status · ':''}{connected.length} connected · {devices.length} paired</>}</p></div><button className="button compact" disabled={devicesLoading} onClick={()=>void refreshDevices()}><RefreshCw size={15} className={devicesLoading?'spin':''}/>{devicesLoading?'Refreshing…':'Refresh'}</button></div>{deviceError&&<p className="inline-error" role="alert">Could not read device status. {deviceError}</p>}{devices.length?[...devices].sort((a,b)=>Number(b.connected)-Number(a.connected)||a.name.localeCompare(b.name)).map(device=><article className="device-row" key={device.id}><div className="device-icon"><Headphones size={21}/></div><div className="device-details"><h3>{device.name}</h3><p>{device.kind} · {device.transport}</p></div><span className={'connection-pill '+(device.connected?'online':'')}><i className="status-dot"/>{device.connected?'Connected':'Disconnected'}</span></article>):<EmptyState icon={Headphones} title={deviceError?'Device status unavailable':devicesLoading?'Checking devices…':'No Bluetooth audio devices found'} detail={deviceError?'Refresh to try again. Windows Bluetooth settings can still manage your devices.':'Pair your headphones in Windows Bluetooth settings, then refresh this list.'}/>}<div className="panel-footer"><span>Windows manages pairing and connections.</span><button className="text-button" onClick={bluetooth}><Bluetooth size={15}/>Bluetooth settings<ExternalLink size={13}/></button></div></section>
            <div className="device-help"><h2>Using more than one device</h2><p>All devices share the active profile. Windows media events do not identify the source device, so EarControl cannot assign a separate profile to each headset.</p><h2>Buttons not appearing?</h2><p>Start Monitor and try a button. Pause other media apps and select EarControl in Windows media controls. Some Bluetooth LE devices may be absent from this list even when their commands work.</p><button className="button compact" onClick={()=>void mode('monitor')} disabled={status.mode==='monitor'}><Radio size={15}/>{status.mode==='monitor'?'Monitoring':'Start monitor'}</button></div>
          </>}

          {view==='activity'&&<section className="panel activity-panel"><div className="panel-heading"><div><h2>Received commands <span className="count-badge">{events.length}</span></h2><p>Up to 250 recent events. Cleared when the app closes.</p></div><div className="inline-actions"><SelectMenu label="Filter activity" className="filter-select" value={eventsFilter} onChange={setEventsFilter} options={[{value:'all',label:'All events'},{value:'actions',label:'Actions'},{value:'detected',label:'Detected'},{value:'errors',label:'Needs attention'}]}/><button className="icon-button" title="Clear activity" aria-label="Clear activity" onClick={()=>setEvents([])} disabled={!events.length}><Trash2 size={15}/></button></div></div>{visibleEvents.length?visibleEvents.map((event,index)=><ActivityRow key={event.id+'-'+index} event={event} full/>):<EmptyState icon={Activity} title={events.length?'No matching events':status.mode==='monitor'?'Waiting for a button press':'No commands yet'} detail={events.length?'Choose another filter to see recent activity.':status.mode==='monitor'?'Try a headphone gesture. Its command will appear here.':'Start Monitor, then try a headphone gesture.'}>{!events.length&&status.mode!=='monitor'&&<button className="button" onClick={()=>void mode('monitor')}><Radio size={15}/>Start monitor</button>}</EmptyState>}</section>}

          {view==='settings'&&<div className="settings-layout">
            <section className="settings-section"><h2><Palette size={17}/>Appearance</h2><p>Choose a theme for this app.</p><div className="theme-options" role="group" aria-label="Theme">{([{id:'light',label:'Light',icon:Sun},{id:'dark',label:'Dark',icon:Moon},{id:'system',label:'System',icon:Monitor}] as const).map(theme=><button key={theme.id} className={'theme-option '+(settings.theme===theme.id?'selected':'')} aria-pressed={settings.theme===theme.id} onClick={()=>updateSettings(current=>({...current,theme:theme.id}))}><theme.icon size={16}/>{theme.label}</button>)}</div></section>
            <section className="settings-section"><h2><Keyboard size={17}/>Input</h2><div className="setting-row"><div><h3>Keyboard media keys</h3><p>Capture standard media keys, including optional volume controls.</p></div><input className="switch" aria-label="Listen for keyboard media keys" type="checkbox" checked={settings.keyboardFallback} onChange={event=>updateSettings(current=>({...current,keyboardFallback:event.target.checked}))}/></div><RepeatDelay value={settings.cooldownMs} onChange={cooldownMs=>updateSettings(current=>({...current,cooldownMs}))} disabled={busy}/></section>
            <section className="settings-section"><h2><Archive size={17}/>Backup</h2><p>Export profiles and preferences, or restore a saved setup.</p><div className="backup-actions"><button className="button" onClick={()=>void exportSettings()}><Download size={16}/>Export settings</button><button className="button" onClick={()=>importRef.current?.click()}><Upload size={16}/>Import settings</button><input ref={importRef} hidden type="file" accept=".json,application/json" onChange={event=>{const file=event.target.files?.[0];event.target.value='';if(!file)return;if(file.size>2*1024*1024){notify('Choose a settings file no larger than 2 MB.',true);return;}void file.text().then(text=>setImportOpen(readImportedSettings(text))).catch(error=>notify(errorMessage(error),true));}}/></div><p className="small-muted">Targets and activity history are excluded from backups.</p></section>
            <div className="about-row"><BrandIcon size={22}/><strong>EarControl</strong><span>Version {version}</span><span>Windows · Local profiles</span></div>
          </div>}
        </fieldset>
      </div>
      {dirty&&!profilesOpen&&<div className="save-bar" role="status"><span>{draftIssue?'Changes need attention':'Unsaved changes'}</span><div className="inline-actions">{draftIssue&&<button className="button compact" disabled={busy} onClick={()=>reviewIssue()}>Review issue</button>}<button className="button compact" disabled={busy} onClick={()=>{if(saved)updateSettings(()=>clone(saved));}}>Discard</button><button className="button primary compact" disabled={busy||!!draftIssue} onClick={()=>void save()}>{busy?<LoaderCircle size={15} className="spin"/>:<Save size={15}/>}Save changes</button></div></div>}
    </main>
    {profilesOpen&&<Modal title="Profiles" onClose={closeProfiles} busy={busy} wide>
      <p className="modal-intro">A profile stores the actions assigned to your buttons.</p>
      <fieldset className="workspace-fields" disabled={busy}>
        <div className="profile-list">{settings.profiles.map((item,index)=><div className={'profile-edit-card '+(item.id===profile.id?'active':'')} key={item.id}>
          <div className="profile-edit-top"><button className="profile-radio" aria-label={'Select '+item.name} aria-pressed={item.id===profile.id} onClick={()=>updateSettings(current=>({...current,activeProfileId:item.id}))}>{item.id===profile.id?<CheckCircle2 size={20}/>:<span/>}</button><input aria-label={'Profile name '+item.name} aria-invalid={!item.name.trim()} aria-describedby={!item.name.trim()?'profile-error-'+index:undefined} value={item.name} maxLength={80} onChange={event=>updateProfile({...item,name:event.target.value})}/><button className="icon-button danger-hover" aria-label={'Delete '+item.name} disabled={settings.profiles.length===1} onClick={()=>setDeleteProfileId(item.id)}><Trash2 size={16}/></button></div>
          {!item.name.trim()&&<p className="field-error" id={'profile-error-'+index}>Give this profile a name.</p>}
          <input className="description-input" aria-label={'Description for '+item.name} value={item.description} maxLength={240} placeholder="Description, optional" onChange={event=>updateProfile({...item,description:event.target.value})}/>
          {deleteProfileId===item.id&&<div className="delete-confirm"><span>Remove this profile and its mappings?</span><button className="text-button" onClick={()=>setDeleteProfileId(null)}>Cancel</button><button className="button danger compact" onClick={()=>removeProfile(item.id)}>Remove</button></div>}
        </div>)}</div>
        <div className="profile-add-actions"><button className="button" onClick={()=>addProfile(false)} disabled={settings.profiles.length>=32}><Plus size={16}/>New profile</button><button className="button" onClick={()=>addProfile(true)} disabled={settings.profiles.length>=32}><Copy size={16}/>Duplicate active</button></div>
        {draftIssue?.mappingId&&<p className="inline-error">{draftIssue.message} <button className="text-button" onClick={()=>reviewIssue()}>Review action</button></p>}
        <div className="modal-footer"><span>{settings.profiles.length} of 32 profiles</span><button className="button primary" disabled={!!draftIssue} onClick={async()=>{setBusy(true);try{await persist();setProfilesOpen(false);notify('Profiles saved.');}catch(error){notify(errorMessage(error),true);}finally{setBusy(false);}}}><Check size={16}/>{busy?'Saving…':'Save changes'}</button></div>
      </fieldset>
    </Modal>}
    {importOpen&&<Modal title="Import settings" onClose={closeImport} busy={busy}><p className="modal-intro">This replaces your current profiles and preferences. Active control will stop.</p><div className="import-preview"><strong>{importOpen.profiles.length} profiles</strong><p>{importOpen.profiles.map(item=>item.name).join(', ')}</p></div><div className="modal-footer"><button className="button" onClick={closeImport} disabled={busy}>Cancel</button><button className="button primary" disabled={busy} onClick={()=>void importSettings()}>{busy?'Importing…':'Replace and import'}</button></div></Modal>}
    {toast&&<div className={'toast '+(toast.error?'error':'')} role={toast.error?'alert':'status'}>{toast.error?<CircleHelp size={18}/>:<CheckCircle2 size={18}/>}<span>{toast.message}</span><button aria-label="Dismiss notification" onClick={()=>setToast(null)}><X size={16}/></button></div>}
  </div>;
}

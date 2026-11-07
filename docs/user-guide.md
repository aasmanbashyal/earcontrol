# EarControl user guide

## First use

1. Pair headphones directly to the PC in Windows Bluetooth settings. The Devices screen opens these settings and lists paired classic Bluetooth audio devices.
2. Click **Monitor** and try each gesture. Activity shows what Windows receives, without typing into another window.
3. Choose a profile and target window. Start the Google Slides slideshow first; refresh and select its window if the slideshow opens separately.
4. Click **Start control**, then switch to the target. Keep the slideshow tab and canvas focused. A window target cannot distinguish browser tabs, text fields, or the address bar.
5. Click **Stop** to cancel control and any running macro. Closing stops capture; minimizing keeps it running. Every launch begins idle.

**Practice deck** opens a local window and selects it as the target. It supports slide navigation and has a text field for testing text macros. Click its slide background to return to navigation.

## Profiles and macros

Google Slides and PowerPoint start with these mappings:

| Headphone command | Shortcut | Action |
| --- | --- | --- |
| Play / pause | Right | Next slide |
| Next track | Right | Next slide |
| Previous track | Left | Previous slide |
| Stop playback | Escape | Exit presentation |

The Everyday profile supplies general navigation shortcuts. The menu beside the profile selector creates, duplicates, renames, and removes profiles. Edits remain drafts until saved. Starting control or testing saves pending changes first; editing while controlling or testing cancels the active session.

**Buttons** lets you enable each command and choose its action. **Macros** edits its name and ordered steps:

- **Shortcut** sends one key or modifier chord. Type it or click Record and press the chord. Recording only runs while that button is focused.
- **Delay** waits a chosen number of milliseconds.
- **Text** types Unicode without replacing the clipboard.

Reorder with the up/down controls. **Test macro** gives three seconds to switch to the selected window. Stop cancels the countdown or execution. Losing target focus or holding a physical modifier also stops the macro. Other commands are skipped while a macro runs.

Supported keys include letters, digits, F1-F24, arrows, Escape, Enter, Space, Tab, Backspace, Delete, Insert, Home, End, PageUp, and PageDown, with Ctrl, Alt, Shift, or Win. `None` is a no-op. Limits are 32 steps, 30 seconds of total delay, and 2,000 text characters. Text is sent in small Unicode-safe batches with cancellation checks between batches.

Settings includes light/dark/system themes, media-key capture, repeat delay, and JSON import/export. Repeat delay can be switched off; duplicate reports from Windows are still filtered. Import validates the complete document before replacing settings. Every imported profile must include all seven commands; unused commands can be disabled. Profiles are stored in the Windows app configuration directory for `app.earcontrol.desktop`, in `profiles.json`. Activity stays in memory; targets and active sessions are never persisted.

## Headphone support

Capture uses Windows System Media Transport Controls and optional standard media keys. It is not tied to Redmi or another brand. Headphones sending supported play/pause, next, previous, or stop commands can be mapped. Optional volume up/down/mute mappings require Windows keyboard media-key events and are disabled by default.

Windows selects which media session receives headphone commands. Pause competing media apps and select EarControl in Windows media controls if the monitor stays empty. A browser video can take over that session. EarControl does not play audio or use the microphone.

EarControl recovers held-button tracking when Windows reports a device change or wakes from sleep, and when capture restarts. A device that disconnects without releasing its media key no longer leaves that button suppressed in the next session. Duplicate filtering uses the time Windows input was captured, so a delayed worker cannot turn two reports of one press into two actions.

Events do not identify the headphone, earbud side, or tap count. Connected devices share the active profile and per-command cooldown. Identical commands from separate devices within that cooldown can still be merged because Windows supplies no independent device identity through these APIs. The paired-device list is informational, not an input filter. Discovery covers classic Bluetooth audio devices. LE-only devices may be absent even when Windows routes their media commands. Brand-specific GATT protocols, battery reports, noise-cancellation controls, firmware changes, and raw gesture detection are not implemented.

If headphones expose only play/pause and next-track, map next-track to Left for both directions. The app cannot create a gesture the device does not send. Physical compatibility depends on the headphones and Windows driver.

Keep EarControl and the target at normal user permissions. Windows blocks input to higher-integrity applications. Release Ctrl, Alt, Shift, and Windows before running macros. The hook ignores injected events and observes only supported media keys.

# EarControl 2.1.2 for Windows

Headphone buttons, shortcuts, and macros for Windows 10/11 x64. Microsoft Edge WebView2 Runtime is required. The installer checks for it; the portable app uses the installed runtime. No .NET or Rust installation is needed.

## Open the app

Extract the ZIP and open `EarControl.exe`. Close any older copy first. The installer offers a per-user installation instead.

1. Pair headphones directly to the PC in Windows Bluetooth settings.
2. Click **Monitor** and try each gesture. Activity shows the commands received without typing into another window.
3. Choose a profile and target window. Start the slideshow before selecting its window.
4. Click **Start control**, then switch to the target. Keep its presentation canvas focused. A window target cannot distinguish browser tabs or text fields.
5. Click **Stop** to cancel control and any running macro. Closing stops capture; minimizing keeps it running. Every launch begins idle.

**Practice deck** opens a local window for slide navigation and text-macro tests. The Google Slides and PowerPoint profiles use play/pause and next track for Right, previous track for Left, and stop playback for Escape. Optional volume commands start disabled.

## Settings and macros

Use Buttons for mappings and Macros for ordered shortcut, delay, and text steps. Edits remain drafts until saved. Starting control or testing saves pending changes first. Editing while controlling or testing cancels that session. Macro testing gives three seconds to focus the target.

Settings includes light/dark/system themes, media-key capture, repeat delay, and JSON import/export. Profiles are stored in the Windows app configuration directory for `app.earcontrol.desktop`, in `profiles.json`. Updating the portable files does not overwrite profiles. Activity stays in memory.

## Compatibility

Windows chooses which media session receives headphone commands. Pause competing media apps and select EarControl in Windows media controls if monitoring stays empty. EarControl does not play audio or use the microphone.

Commands do not identify the headphone, earbud side, or tap count. Devices share one active profile and per-command cooldown. The paired-device list is informational, not an input filter. LE-only devices may be absent from the list even when their commands work. Physical compatibility depends on the headphones and Windows driver.

Keep EarControl and the target at normal user permissions. Windows blocks input to higher-integrity applications. Release Ctrl, Alt, Shift, and Windows before running macros. Losing target focus stops macro output.

Verify downloads against the release's SHA-256 manifest. Release binaries are not code-signed.

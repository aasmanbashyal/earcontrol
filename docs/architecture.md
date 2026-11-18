# Architecture

EarControl is a Windows and macOS Rust application. egui/eframe draws the main interface and a separate practice viewport with OpenGL. Both use custom window controls, keyboard focus and the same live theme preference. There is no webview, IPC command bridge, JavaScript application or browser preview.

```text
Cargo.toml                  Workspace and release profile
desktop/
  Cargo.toml                Application version and dependencies
  build.rs                  Windows icon, version and application manifest
  assets/                   Icons, emergency font and third-party notices
  src/
    lib.rs, main.rs         Process and window lifecycle
    ui.rs                   Screens, drafts, profiles, recording, file dialogs
    design.rs               Original palette, typography, icons and controls
    practice.rs             Practice UI and slide state machine
    practice_accessibility.rs Windows accessibility bridge for the practice viewport
    practice_accessibility_macos.rs  No-op on macOS, where eframe's AccessKit serves the deck
    runtime.rs              Session state, event history, workers, cancellation
    window/                 Single instance, target identity, settings location
      windows.rs, macos.rs  One implementation per platform behind the same API
    model.rs                Compatible version-2 settings schema and validation
    settings.rs             Bounded reads and atomic persistence
    native/                 Media capture, devices and input delivery
      windows.rs            Media sessions, keyboard hook, SendInput, Bluetooth API
      macos.rs              Now Playing commands, media-key event tap, Quartz events
    engine.rs               Deduplication, macro order and cancellation gates
    shortcuts.rs            Shortcut parser and key representation
  benches/workloads.rs      Core and headless UI benchmarks
scripts/                    PowerShell build, package, verify and benchmark tasks
vendor/                     Narrow eframe/egui-winit feature splits
docs/                       Product, development and benchmark reports
media/launch/               Historical 2.x demo source and asset credits
dist/                       Generated portable app, ZIP, installer, checksums
```

Windows media events enter `native.rs`. The shared runtime checks the active session, capture generation, duplicate gate, mapping, target process/window identity and physical modifiers. Macro workers run shortcut, delay and Unicode text steps in order. Stop increments the generation; every wait and text batch rechecks cancellation and focus. Editing while controlling or testing stops that session. Closing the main window stops capture and closes the practice window; minimizing keeps control running.

The UI holds a draft separate from saved runtime settings. Validation runs after edits and again before persistence or execution. Invalid imports never replace saved data. Native file dialogs handle import/export. Bluetooth enumeration runs on a worker so it does not block the UI. Runtime events request repainting; the idle UI does not run a frame timer. Activity is newest-first and bounded to 250 entries.

An event-driven Windows media-session observer reports the currently selected route separately from capture initialization and received commands. It runs outside the low-level keyboard hook thread. Retry restarts capture in the selected mode, reinstalls the optional keyboard hook and resets per-generation report counts. Button check uses current-session outcomes, so history and late results from cancelled macros cannot masquerade as a newly received command. The observer never sends playback commands or forces Windows to choose a session.

Windows font files are mapped once and borrowed by egui. The mappings and read-only file handles live for the process lifetime; their sharing mode prevents concurrent modification or removal. If mapping fails, a cached owned copy supplies the font. A small embedded font keeps basic controls usable when Windows fonts are unavailable. Decorative dash meshes use a bounded cache keyed by geometry, color, DPI and tessellation settings.

The vendored UI crates split optional URL-opening and image-clipboard support out of the clipboard feature. EarControl retains native text clipboard and accessibility support. See [vendor scope and maintenance](../vendor/README.md) before upgrading those crates. [Optimization measurements](optimization.md) describe the performance changes and their limits.

On macOS, `native/macos.rs` implements the same interface. While capture runs, EarControl registers MPRemoteCommandCenter handlers and sets its Now Playing state to playing, without playing audio. The optional media-key fallback is a Quartz event tap on NX_SYSDEFINED events, run on its own thread with its own run loop. It consumes only mapped keys and applies the same held-key rules as the Windows hook. Shortcuts map the stored Windows virtual keys to macOS key codes, with Win as Command and Alt as Option. They are posted as tagged Quartz events, so the tap ignores them. Sending input and the event tap need Accessibility access. A target is a Quartz window number plus process ID. Focus checks require the target's process to be frontmost and the target to be its front-most ordinary window. The main and practice windows keep the native macOS title bar.

Settings stay at `%APPDATA%/app.earcontrol.desktop/profiles.json` on Windows and `~/Library/Application Support/app.earcontrol.desktop/profiles.json` on macOS. The JSON schema remains version 2, including camelCase fields and tagged macro steps. Session mode, targets, activity and practice text are never persisted. `EARCONTROL_SETTINGS_PATH` supplies an isolated path for testing and benchmarking.

A target is a window and process identity, not a browser tab. Headphone events do not identify a source headset or earbud side. The paired-device list remains informational.

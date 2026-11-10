# Architecture

EarControl is a Windows Rust application. egui/eframe draws the main interface and a separate practice viewport with OpenGL. Both use custom window controls, keyboard focus and the same live theme preference. There is no webview, IPC command bridge, JavaScript application or browser preview.

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
    runtime.rs              Session state, event history, workers, cancellation
    window.rs               Single instance, target identity, settings location
    model.rs                Compatible version-2 settings schema and validation
    settings.rs             Bounded reads and atomic persistence
    native.rs               Windows media capture, devices and input delivery
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

Windows font files are mapped once and borrowed by egui. The mappings and read-only file handles live for the process lifetime; their sharing mode prevents concurrent modification or removal. If mapping fails, a cached owned copy supplies the font. A small embedded font keeps basic controls usable when Windows fonts are unavailable. Decorative dash meshes use a bounded cache keyed by geometry, color, DPI and tessellation settings.

The vendored UI crates split optional URL-opening and image-clipboard support out of the clipboard feature. EarControl retains native text clipboard and accessibility support. See [vendor scope and maintenance](../vendor/README.md) before upgrading those crates. [Optimization measurements](optimization.md) describe the performance changes and their limits.

Settings stay at `%APPDATA%/app.earcontrol.desktop/profiles.json`. The JSON schema remains version 2, including camelCase fields and tagged macro steps. Session mode, targets, activity and practice text are never persisted. `EARCONTROL_SETTINGS_PATH` supplies an isolated path for testing and benchmarking.

A target is a window and process identity, not a browser tab. Headphone events do not identify a source headset or earbud side. The paired-device list remains informational.

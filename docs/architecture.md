# Architecture

EarControl has two Tauri windows: the main control interface and a local practice deck. Both use React, shared UI components, and the same theme preference.

```text
desktop/
  src/
    App.tsx                 Main session, drafts, navigation, profiles
    Practice.tsx            Practice deck and received shortcut feedback
    components/             Editors, selectors, dialogs, window chrome, rows
    bridge.ts               Tauri commands/events and browser-preview boundary
    preview.ts              Isolated browser settings persistence
    theme.ts                Saved/live/system theme resolution
    types.ts                Shared frontend data contract
    *.test.ts               Frontend behavior tests
  src-tauri/src/
    lib.rs                  Tauri commands, application/window lifecycle
    model.rs                Settings schema, defaults, validation
    settings.rs             Atomic local settings persistence
    native.rs               Windows media capture, devices, window/input APIs
    engine.rs               Command gating, macro order, cancellation
    shortcuts.rs            Shortcut parser and key representation
scripts/                    Windows build and release verification
docs/                       Product, contributor, and release guidance
media/launch/               Editable demo composition and asset credits
dist/                       Generated portable app, ZIP, installer, checksums
```

## Command path

Windows media controls or keyboard media-key events enter `native.rs`. The engine filters duplicate reports and applies the active profile's mappings. Shortcut, delay, and text steps run in order. Target focus, physical modifier state, session generation, and cancellation checks gate output. Stop invalidates the session and cancels pending output.

The interface edits a draft and validates it before saving. Native settings validation remains authoritative. `settings.rs` replaces the settings file atomically; session state and targets are never persisted. UI activity is delivered through Tauri events and remains in memory.

## Boundaries

- A paired-device list is informational. Windows media events do not identify which headset sent them.
- A target is a window plus process identity, not an individual browser tab.
- Browser preview uses separate storage and cannot exercise native capture or injection.
- The practice deck shares theme code and UI controls but keeps its presentation state local.

The existing Rust modules already separate platform code, domain behavior, and persistence. Components are extracted when they have a reusable presentation role; state is kept together where a session transition must remain coordinated.

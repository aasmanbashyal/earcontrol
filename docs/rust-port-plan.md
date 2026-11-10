# Full Rust port

Branch: `feat/rust-native-ui`. Baseline: `82165a4`, EarControl 2.1.2.

## Scope

Replace React, TypeScript, Vite, Tauri and WebView2 with a native Rust executable. Keep the existing Windows capture, shortcut parser, macro engine and version-2 settings format. Render both windows with egui/eframe and OpenGL. Match the original layout, controls, custom window chrome, typography and neutral themes. Keep native file dialogs and keyboard operation.

## Implementation order

1. Preserve the release baseline and existing profile format. Create a Cargo workspace and move the native modules into the desktop crate.
2. Replace Tauri commands/events with a shared Rust runtime, bounded activity history and event-driven repainting. Preserve cancellation, focus checks, deduplication and single-instance behavior.
3. Port Buttons, Macros, Devices, Activity, Settings and profile management. Include draft save/discard, validation, shortcut recording, import confirmation and theme selection.
4. Port the separate practice window, slide controls, keyboard feedback and Unicode text field. Keep it selectable as a real target window.
5. Replace application build, packaging, release verification and CI with Cargo and PowerShell. Remove the web application and its dependencies. Keep historical demo media as documentation.
6. Run Rust tests, formatting, linting, native Windows smoke tests, UI checks at normal/minimum sizes, packaging and artifact verification.
7. After the port works, benchmark release builds on this machine. Record executable/ZIP size, time to a visible window, idle process-tree private bytes/working set/CPU and core/UI workloads. Include repetitions, raw results and limits. Do not count only the Tauri parent process or call simulated tests physical headphone tests.

## Acceptance

- Existing profiles load from `%APPDATA%/app.earcontrol.desktop/profiles.json` without conversion or automatic overwrite.
- Seven commands, profile CRUD, all macro step types, validation, countdown, cancellation, monitoring and window targeting work in Rust.
- Both windows use Rust UI code and require no browser runtime.
- Main screens and the practice deck match the original interface at normal and minimum sizes. A plain toolkit layout is not accepted as visual parity.
- Monitor must display received commands visibly. Distinguish real SMTC/runtime tests from the physical earbud check reported by the person testing the app.
- Release tasks produce a portable ZIP, per-user installer and checksums.
- Benchmark results and any unverified hardware/release checks are documented honestly.

The native window mechanism is documented in [eframe NativeOptions](https://docs.rs/eframe/0.33.3/eframe/struct.NativeOptions.html). The UI uses [egui contexts and viewports](https://docs.rs/egui/0.33.3/egui/struct.Context.html).

## Completion record

The application and practice window are implemented in Rust on this branch. The physical earbud Monitor check passed after restoring the shared single-instance identity and root-window repainting. The UI was revised against the original interface, including profile and import/export behavior, validation focus, practice accessibility, Unicode font fallbacks and window reopening.

Local tests, release artifacts, measured results and remaining rendering/release limits are recorded in [benchmarks and validation](benchmarks.md). The branch has not been published as a release.

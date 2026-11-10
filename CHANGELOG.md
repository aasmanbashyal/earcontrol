# Changelog

## 3.0.0, native Rust port

- Added live Windows media-routing status, per-session input diagnostics and Retry input. Restarting capture now refreshes the keyboard hook, including hooks Windows silently removed after a timeout.
- Kept Button check feedback scoped to the current capture session, including after retry or a late macro cancellation.
- Gave Settings a distinct gear icon, increased repeat-delay slider contrast, and enlarged text fields with consistent padding.
- Ported both application windows to egui/eframe, including profile editing, macro recording and testing, devices, activity, settings and practice controls.
- Removed React, TypeScript, Vite, Tauri and WebView2 from the application build and runtime.
- Preserved the version-2 JSON settings format, storage location and Windows input safeguards.
- Restored the legacy single-instance lock so 2.x and 3.x copies cannot publish competing EarControl media sessions.
- Added Rust UI/state tests, repeatable release benchmarks, Cargo/PowerShell CI and a per-user NSIS installer.
- Recreated the original window controls and neutral themes in Rust.
- Reduced the executable and font memory by using cached Windows font mappings and a small emergency font. Removed unused URL-opening and image-clipboard dependencies while retaining text clipboard and accessibility support.
- Reduced allocations in shortcut validation, duplicate filtering, Unicode macro dispatch and idle UI frames. Cached decorative line geometry without changing its rendered output.
- Recorded before-and-after Rust benchmarks and rejected compiler and renderer changes that worsened measured workloads.

## 2.1.2

- Root Bun commands for development, checks, packaging, and release verification.
- Product README, user guide, architecture, support, and release documentation.
- Tag validation and a GitHub workflow that prepares draft releases.
- Reusable activity and empty-state components extracted from the main app.
- Illustrated product demo with editable Hyperframes source and credited audio.

## 2.1.1

- Unified neutral accents across light and dark themes, charcoal dark surfaces, and consistent practice-deck styling.
- Refined Stop, Getting started, keyboard focus, compact desktop layouts, and shared draft validation.
- Removed the legacy .NET prototype from active source after archiving it locally, unused metadata/styles, and generated icon variants.
- Refreshed portable and installer packaging with version checks and SHA-256 manifests.

## 2.1.0

- Profiles, ordered shortcut/delay/text macros, optional volume commands, and repeat-delay settings.
- Capture recovery after device changes and restart, duplicate filtering, and cancellable Unicode-safe text output.

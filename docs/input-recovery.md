# Input recovery and control styling

Historical validation of build `87674e6` on November 10, 2026, on `feat/rust-native-ui`. See [the full QA record](qa.md) for the later Practice focus fix, physical control retest and current package hashes.

## Physical observation

The reported failure was an empty Monitor with Nothing earbuds. A fresh launch of the preceding build received Play/pause and Previous track during a physical test. Windows selected `EarControl.exe`; the passive diagnostic watcher observed no keyboard media-key reports during that test. This is consistent with media-session delivery, rather than the optional keyboard fallback. The original intermittent failure's cause was not established.

After the changes below, the rebuilt release was opened in Monitor. Its status showed Windows selecting EarControl, Button check displayed Next track as detected, and the person testing confirmed that commands appeared from the Nothing buds. This verifies that device and the tested gestures on this machine, not every headset or Windows driver.

## Changes

- Show the selected Windows media route separately from capture mode and received commands. Routing updates come from Windows events, with no polling timer or playback commands.
- Add Retry input. It restarts capture in the selected mode with the same target, cancels any macro, refreshes routing and reinstalls the optional keyboard hook. Windows can silently remove a timed-out low-level hook, so retaining a non-null handle was insufficient for recovery.
- Activate runtime mode before enabling native capture, and roll back on a failed start, so an early report cannot be discarded solely because startup still appears idle.
- Track report counts and Button check outcomes by capture generation. Old history, delayed cancellation results and cleared Activity no longer appear as proof of a new command.
- Keep routing queries off the low-level hook thread and make observer shutdown safe. Injected-key filtering remains unchanged.
- Replace the Settings slider icon with a gear, increase repeat-delay rail/fill/thumb contrast, and use padded single-line text fields with a 36-pixel minimum height. Shortcut fields align with Record.

Retry cannot force Windows to select EarControl. If another media app remains selected, pause it and choose EarControl in Windows media controls. A dead native input thread requires restarting the application; its Retry control is disabled.

## Checks

- Formatting and all-target Clippy passed. The environment emitted incremental-cache hard-link fallback warnings, not code diagnostics.
- The ordinary suite passed 72 tests, with the interactive native test ignored in that run.
- The separate Windows smoke test passed. It exercises real Right/Left/Unicode delivery to its owned window, direct SMTC dispatch, Monitor without shortcut injection, Stop/restart, retry mode/target preservation, report-count reset and recovery after deliberately removing the test's keyboard hook.
- The smoke test addresses a known EarControl media session directly. It does not prove Windows' physical-headphone routing; the physical result above is a separate check.
- Live release inspection confirmed distinct icons, visible slider contrast and padded fields in dark and light themes. The active routing banner and complete Retry button were visible on the first Monitor frame. The original test theme was restored without saving a configuration change.
- Existing layout checks passed at 860×620 and 1160×780 in both themes. These are headless layout checks, not a claim of live resizing at every DPI.
- Portable ZIP contents, installer generation and release checksums passed verification.

## Tested artifacts

The tested executable is 2,225,664 bytes with SHA-256 `6e40a2d9d760a33c217f08ff8c980cf8ab60d7a226d8dcf05351b2e8b4e0842f`.

The portable ZIP was 1,177,545 bytes with SHA-256 `3018e626c8e8a9cc753e730e7bdae3d3f4b4504d1a05e361e42909b5f93de0e6`. The historical `EarControl-3.0.0-input-ui-fix-win-x64.zip` delivery copy preserves that package. The normal versioned ZIP has since been rebuilt.

The earlier [optimization results](optimization.md) remain tied to their original executable hash. CPU and process benchmarks were not rerun for these subsequent fixes. No release was published and no installed upgrade/uninstall cycle was tested.

For future diagnosis, run `cargo run --release --locked --example media_diagnostics -- --watch-seconds 90` while Monitor is active. It reports media-session application identities and media-key flags only, without consuming keys, issuing media commands, or logging ordinary keys or media titles. Close the diagnostic after the physical test.

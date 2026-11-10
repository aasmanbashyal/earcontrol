# Rust optimization results

Benchmarks were completed before optimization. The completed Rust port, commit `aedea25`, is the baseline for this report. The changes remain on `feat/rust-native-ui`; no release has been published.

The requested threefold improvement was achieved for shortcut parsing, duplicate filtering and CPU-only Unicode macro dispatch. It was not achieved for executable size, memory, startup, profile validation or UI rendering. There is no measured threefold improvement in the whole application or physical earbud latency.

## Size and process measurements

Measured November 10, 2026, on Windows 11 Home 10.0.26300, Ryzen 9 7940HS, 16 logical processors, 31.21 GiB usable RAM and Radeon 780M, driver 31.0.14084.3001. Both builds use Rust 1.99.1, the Windows MSVC target, LTO and `opt-level = "s"`.

Process values are medians of five interleaved runs after an unmeasured warmup for each executable. Both native builds read the same isolated copy of the existing configuration. Each launch settles for five seconds before a five-second idle observation. Compilation and renderer probes had finished before these runs.

The ratio is baseline divided by optimized. A ratio of 3 would meet the threefold target for a lower-is-better metric.

| Metric | Initial Rust | Optimized Rust | Ratio | Reduction |
| --- | ---: | ---: | ---: | ---: |
| Executable, bytes | 3,794,944 | 2,193,920 | 1.73 | 42.2% |
| Portable ZIP, bytes | 2,034,127 | 1,162,680 | 1.75 | 42.8% |
| Installer, bytes | 1,700,222 | 970,299 | 1.75 | 42.9% |
| Private memory, MiB | 136.39 | 98.04 | 1.39 | 28.1% |
| Working set, MiB | 146.62 | 108.86 | 1.35 | 25.8% |
| Visible-window time, ms | 57.82 | 51.73 | 1.12 | 10.5% |
| CPU during idle observation, ms | 78 | 62 | 1.26 | 20.5% |
| Idle CPU, percent of one core | 1.4851% | 1.1831% | 1.26 | 20.3% |
| Processes | 1 | 1 | 1.00 | 0% |

The window-time ranges overlap, 47.36–68.97 ms before and 48.71–63.26 ms after. These samples do not establish a reliable startup speedup. Windows reporting a visible HWND is only a proxy; it does not establish that the first frame is painted or ready for interaction. Short idle CPU observations have coarse accounting and desktop noise, so they are not battery-life or steady-state energy results.

The private-memory target for this paired baseline would be at most 45.46 MiB; the executable target would be at most 1,264,981 bytes. The retained build is above both limits. It also retains its single process, native input safeguards, text clipboard, accessibility and Unicode font fallbacks.

The initial Rust process baseline was measured again for this comparison. Its numbers therefore differ slightly from the earlier [Tauri-to-Rust report](benchmarks.md), whose original raw data remains unchanged. MiB means 1,048,576 bytes. Working-set totals may include shared pages. CPU deltas cover processes present at both endpoints; a process born and exited between snapshots is not observed.

Raw data: [paired process samples](benchmark-results/optimized-processes.json), [artifact and workload provenance](benchmark-results/optimized-manifest.json).

## CPU workloads

Values are microseconds per operation. The original eleven workload inputs are unchanged. Each executable performs 100 warmup operations and 21 measured batches; medians and p95 values describe batch means, not individual input events. The optimized workload executable was rebuilt from the final production source and run after compilation finished.

The initial Rust workload data was preserved before changes. This is a comparison with that earlier quiet run on the same machine, rather than a new interleaved replay of both workload executables. Small differences, particularly the Macros and Activity rows, should not be treated as established gains.

| Workload | Before median | After median | After p95 batch mean | Observed speed ratio | 3× median target |
| --- | ---: | ---: | ---: | ---: | --- |
| Parse Ctrl+Shift+F5 | 0.46145 | 0.14541 | 0.23227 | 3.17 | Met |
| Validate default profiles | 5.98140 | 2.43250 | 4.09090 | 2.46 | Unmet |
| Decode and validate profiles | 28.62280 | 23.43510 | 25.83890 | 1.22 | Unmet |
| Accept command through duplicate gate | 0.17032 | 0.04045 | 0.05507 | 4.21 | Met |
| Unicode macro dispatch, no OS I/O | 6.96580 | 0.62140 | 1.05510 | 11.21 | Met |
| Buttons UI | 493.43700 | 400.22550 | 612.75400 | 1.23 | Unmet |
| Macros UI | 373.93000 | 366.91550 | 559.38400 | 1.02 | Unmet |
| Devices UI | 253.06650 | 210.35600 | 232.17300 | 1.20 | Unmet |
| Activity UI | 204.54450 | 191.38450 | 205.80800 | 1.07 | Unmet |
| Settings UI | 405.78050 | 298.72100 | 345.11900 | 1.36 | Unmet |
| Practice UI | 225.27250 | 145.33050 | 159.78450 | 1.55 | Unmet |

UI timings include layout and tessellation at 1160×780 for the main screens and 960×640 for practice. They exclude native windows, GPU rendering, device enumeration and Windows event delivery. Macro callbacks omit OS output and waits. These results do not measure headphone-to-shortcut latency.

A new large-configuration diagnostic renders Buttons with 32 profiles containing Unicode text macros. Its optimized median is 450.34 microseconds and p95 batch mean is 570.22 microseconds. There is no preserved equivalent baseline, so no improvement ratio is claimed for that row.

Raw data: [initial workloads](benchmark-results/workloads.json), [optimized workloads](benchmark-results/optimized-workloads.json).

## Changes retained

- Mapped Windows font files once and shared the immutable bytes instead of retaining duplicate owned buffers. Kept Segoe, Consolas, Nirmala and emoji fallbacks, with a cached owned-copy fallback if mapping fails.
- Replaced the framework's bundled fonts with a licensed 34,388-byte emergency subset. Windows fonts remain supplied by the operating system.
- Split unused URL-opening and image-clipboard handlers out of two narrowly vendored UI integration crates. Native text clipboard, accessibility, PNG icon decoding and file dialogs remain enabled. Third-party notices ship in both packages.
- Replaced temporary shortcut token strings and modifier sets with bounded stack storage. Validation avoids creating a key vector that would be discarded. Mapping uniqueness uses bounded arrays and a bit mask.
- Reused duplicate-filter entries and borrowed Unicode text slices while retaining 32-scalar batches, cancellation and target checks.
- Skipped whole-draft clones on idle frames, copied only the latest command for Button check, and cached exact decorative dash meshes. Input frames, UIA edits, held gestures and screen transitions still inspect draft changes.

The small compiler profile `z` reduced size but caused substantial CPU regressions. Optimizing only the application at level `3` increased the executable by 20.7% and slowed Unicode dispatch. Both were rejected. WGPU/DX12 increased empty-window memory on this driver and was also rejected.

An empty Glow window already used about 102 MiB in a separate diagnostic sample. A minimal D3D11 hardware window used about 33 MiB, but lacked egui drawing, fonts, clipboard, accessibility, child viewports and application state. A complete D3D11 integration remains a possible larger project; the blank-window measurement is not a threefold application-memory result. See [renderer and compiler experiments](renderer-experiments.md) for raw controls, source snapshots and limitations.

## Validation and remaining limits

- Formatting and all-target Clippy passed with warnings denied. The regular suite passed 67 tests, with the interactive native test ignored in that run.
- The separate native Windows test passed after optimization. It exercised real Right/Left and Unicode delivery, media-session commands, Monitor activity and repaint, disabled mappings, target identity, Stop and restart.
- The packaged executable was checked live for the main interface, practice navigation, Unicode glyph coverage and child-window accessibility. Unicode Ctrl+C/Ctrl+V survived closing and recreating practice, with the exact text `EarControl clipboard नमस्ते 🦀` preserved.
- Direct pointer clicks reopened practice from Buttons and Macros. Element-targeted computer-use actions intermittently failed to activate the main Practice button after closing a child, while direct clicks worked. The cause was not established; this remains an automation/accessibility validation limitation, not a claimed code fix. Native-editor clipboard interoperability was not completed because the computer-use launch approval timed out.
- The installer and portable package passed release verification, including ZIP contents, the packaged executable hash and checksums. Install/upgrade/uninstall testing and hosted CI remain outside this local validation.

The earlier physical earbud test succeeded before this optimization pass. The optimized build's Windows capture path passed automated native regression tests; physical earbuds were not pressed again during this pass. Full Indic shaping and color emoji remain renderer limitations described in the original port report.

## Reproduce

Preserve the initial Rust executable and workload JSON before rebuilding. Close EarControl and finish compilation before measuring. From PowerShell 7 at the repository root:

```powershell
./scripts/windows.ps1 -Task Check
./scripts/windows.ps1 -Task Test
./scripts/windows.ps1 -Task WindowsTest
./scripts/windows.ps1 -Task Package
./scripts/verify-release.ps1
./scripts/benchmark.ps1 -Baseline '.tools/rust-port-optimization-baseline/EarControl.exe' -Candidate 'dist/EarControl-3.0.0/EarControl.exe' -BaselineName rust_before -CandidateName rust_optimized -BaselineUsesIsolatedSettings -Runs 5 -SettleSeconds 5 -IdleSeconds 5 -Output docs/benchmark-results/optimized-processes.json
cargo bench --locked --bench workloads > docs/benchmark-results/optimized-workloads.json
```

`WindowsTest` needs an unlocked interactive desktop and sends input to its owned test window. The process script closes only processes it launched. Preserve existing result files before a new measurement. For quiet CPU timing, build with `cargo bench --locked --bench workloads --no-run`, then run the reported executable directly once compilation has stopped.

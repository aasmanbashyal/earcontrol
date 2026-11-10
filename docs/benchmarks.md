# Rust port benchmarks and validation

This historical report compares a preserved local EarControl 2.1.2 build from `82165a4` with the initial native Rust 3.0.0 build at `aedea25`. Raw results include the executable hashes, sizes, hardware, settings method, and individual samples. See the [3.0.0 release report](release-3.0.0.md) for the current candidate measured against the published GitHub release.

## Process comparison

Measured November 10, 2026, on Windows 11 Home 10.0.26300, AMD Ryzen 9 7940HS with 16 logical processors, 31.21 GiB usable RAM and Radeon 780M graphics, driver 31.0.14084.3001. Built with Rust 1.99.1 for `x86_64-pc-windows-msvc`, size optimization and LTO.

Values are medians of five runs. The Rust port used 36.9% less private memory and one process instead of seven. Window-creation times overlapped, so these runs do not establish a startup improvement.

| Metric | Tauri 2.1.2 | Rust 3.0.0 |
| --- | ---: | ---: |
| Time to visible window, ms | 56.01 | 54.37 |
| Window-time range, ms | 49.16 to 59.21 | 40.82 to 62.35 |
| Private memory, MiB | 214.54 | 135.31 |
| Summed working set, MiB | 418.61 | 140.99 |
| Process count | 7 | 1 |
| CPU time during idle observation, ms | 532 | 62 |
| Idle CPU, percent of one logical core | 10.05% | 1.18% |

CPU measurements are short observations with coarse accounting and background desktop activity. Read the method below before treating them as steady-state CPU or energy results. The executable hash in these process results was verified against the packaged and live-tested initial port. Later optimized packages have a different hash.

## Download size

| Artifact | 2.1.2 bytes | Rust 3.0.0 bytes |
| --- | ---: | ---: |
| Executable | 3,416,576 | 3,794,944 |
| Portable ZIP | 1,540,961 | 2,034,127 |
| New installer | Not compared | 1,700,222 |

The executable is 11.1% larger and the ZIP is 32.0% larger. The old executable relies on the separately installed WebView2 runtime; these download sizes do not include that shared installation. The Rust package includes its UI renderer and requires a working OpenGL driver.

## Core and UI workloads

Times below are microseconds per operation. The UI rows include layout and tessellation only.

| Workload | Median | p95 batch mean |
| --- | ---: | ---: |
| Parse Ctrl+Shift+F5 | 0.46 | 0.62 |
| Validate default profiles | 5.98 | 8.07 |
| Decode and validate profiles | 28.62 | 34.75 |
| Accept command through duplicate gate | 0.17 | 0.24 |
| Dispatch Unicode macro, OS output excluded | 6.97 | 8.51 |
| Buttons UI | 493.44 | 650.87 |
| Macros UI | 373.93 | 565.84 |
| Devices UI | 253.07 | 290.11 |
| Activity UI | 204.54 | 259.48 |
| Settings UI | 405.78 | 474.54 |
| Practice UI | 225.27 | 260.39 |

## Measurement method

The process comparison uses five measured launches per app after one unmeasured warmup each. Launch order alternates between versions. Both apps read the same configuration; the Rust build reads an isolated copy. Neither benchmark starts control or sends shortcuts. The script closes only processes it launched.

Startup is the time until Windows reports a visible top-level window. It does not measure the first painted frame or readiness for interaction. Each launch settles for five seconds before a five-second idle observation. Private bytes and working sets include the app's descendant processes, including WebView2 for 2.1.2. Summed working sets can count shared pages more than once.

Idle CPU is accumulated CPU time for processes present at both snapshots, divided by elapsed time and expressed as a percentage of one logical core. Samples fail if the process identities differ between snapshots. A process born and exited entirely between snapshots is not observed. These are warm runs on one shared desktop, not cold-boot or battery-life measurements.

The core and UI benchmark uses 21 warmed batches. Median and p95 describe batch means, not individual-event latency. UI measurements include layout and tessellation at 1160 by 780 for the main screens and 960 by 640 for practice, without a native window, GPU rendering, device discovery or Windows event delivery. Macro timing replaces OS output and waits with no-op callbacks. It is not an earbud-to-shortcut latency measurement. These Rust workloads have no comparable 2.1.2 frontend result.

Raw data: [process samples](benchmark-results/processes.json), [core and UI samples](benchmark-results/workloads.json).

## Reproduce

Preserve a 2.1.2 release executable before rebuilding. Close existing EarControl windows, then run from the repository root with PowerShell 7:

```powershell
./scripts/windows.ps1 -Task Check
./scripts/windows.ps1 -Task Test
./scripts/windows.ps1 -Task WindowsTest
./scripts/windows.ps1 -Task Package
./scripts/verify-release.ps1
./scripts/benchmark.ps1 -Baseline 'C:\path\to\2.1.2\EarControl.exe' -Candidate '.\dist\EarControl-3.0.0\EarControl.exe' -Runs 5 -SettleSeconds 5 -IdleSeconds 5
cargo bench --locked --bench workloads > docs/benchmark-results/workloads.json
```

`WindowsTest` requires an unlocked interactive desktop. It focuses an isolated test window and delivers real Windows keyboard and media events. Keep compilation and other CPU-heavy work separate from measured runs.

## Validation scope

Validation completed locally on November 10, 2026:

- Formatting and all-target Clippy passed with warnings denied.
- The normal Rust suite passed 61 tests. Its one ignored interactive Windows test passed separately.
- Headless UI checks rendered all five main screens at normal and minimum sizes in both themes. Practice layout checks covered compact widths and keyboard focus behavior.
- Live release checks confirmed Monitor feedback, main-screen layout, practice navigation and editor focus. Windows accessibility exposed the practice buttons and text field. Closing and reopening practice retained those controls and working navigation.
- Import cancellation, export without edits, validation-field navigation, pointer and keyboard switches, and Monitor target selection have focused regression coverage.

The physical earbud check passed: Monitor displayed a received command in Button check, and the person testing confirmed that commands appeared. This establishes capture with that connected device and driver. It does not establish compatibility with every headphone or gesture.

The native smoke test exercises Right, Left and Unicode input against its own test window, actual Windows media-session dispatch, Monitor activity and repaint, stopping and restarting capture, and the absence of injected shortcuts during Monitor. Unit and UI tests cover profile validation and persistence, macro order and cancellation gates, duplicate filtering, single-instance identity, practice controls and application interactions.

The UI follows the original neutral themes, sidebar, cards, custom window controls and practice layout. Native font rasterization, some vector icons, popups and file dialogs differ from the browser version. This is not a pixel-identical rendering claim. Windows font fallbacks cover the tested Nepali and emoji characters. The renderer does not provide full Indic shaping or color emoji, although the stored and delivered Unicode text is preserved.

The portable ZIP and installer are built locally and checked against their SHA-256 manifest. ZIP contents and the executable inside the ZIP are verified. Binaries are unsigned. Hosted CI and an installed 2.x-to-3.x upgrade/uninstall cycle have not been exercised in this workspace; see [releasing](releasing.md).

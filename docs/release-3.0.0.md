# EarControl 3.0.0 performance and release checks

The 3.0.0 candidate used 56.7% less idle private memory and shipped a 30.1% smaller executable than the published 2.1.2 build. It ran as one process and removes the WebView2 runtime requirement. Its titled window appeared later, while its Practice deck button became accessible in about half the time.

## Published 2.1.2 vs. 3.0.0

Measured November 10, 2026 on Windows 11 Home build 26300, a Ryzen 9 7940HS with 16 logical processors, 31.21 GiB usable RAM, and Radeon 780M graphics, driver 31.0.14084.3001. The 3.0.0 executable was built from `493bedb` with the current working-tree changes using Rust 1.99.1, x64 MSVC, static CRT, `opt-level = "s"`, and LTO. The 2.1.2 ZIP and installer matched the GitHub asset digests, and the executable matched the published manifest. [Published 2.1.2 artifacts](https://github.com/aasmanbashyal/earcontrol/releases/tag/v2.1.2)

| Metric | Published 2.1.2 | 3.0.0 candidate | Difference |
| --- | ---: | ---: | ---: |
| Executable, bytes | 3,417,088 | 2,387,456 | 30.1% smaller |
| Portable ZIP, bytes | 1,541,006 | 1,264,351 | 18.0% smaller |
| Installer, bytes | 1,282,996 | 1,052,102 | 18.0% smaller |
| Titled main window, median ms | 61.84 | 309.18 | 247.34 ms later |
| Titled main window, range ms | 59.03 to 67.49 | 252.43 to 326.53 | |
| Enabled Practice deck button via UI Automation, median ms | 581.69 | 275.02 | 52.7% sooner |
| Enabled Practice deck button via UI Automation, range ms | 579.75 to 643.84 | 240.11 to 298.79 | |
| Idle private memory, median MiB | 224.89 | 97.32 | 56.7% lower |
| Idle summed working set, median MiB | 404.65 | 103.94 | 74.3% lower |
| Idle CPU, median percent of one logical core | 0.503% | 0.000% recorded | No CPU counter increase for 3.0.0 in any sample |
| Processes | 7 | 1 | 6 fewer |

Seven paired idle runs alternated launch order after one warmup per version. Both apps read the same saved configuration, with 3.0.0 using a copy. The measurements include WebView2 descendants for 2.1.2. Each app settled for five seconds before a six-second idle sample. Summed working sets can count shared pages more than once. CPU accounting is coarse over this interval. Download sizes exclude the shared WebView2 installation used by 2.1.2. [Raw idle samples](benchmark-results/release-3.0.0-processes.json)

The startup row uses five separate paired launches and requires a visible, titled main window owned by the tested process. The original idle run's startup field could count an untitled helper window, so it was excluded from the startup comparison. The native renderer waits through the first frame before showing its window. The titled-window measurement includes that wait for 3.0.0. [Raw startup samples](benchmark-results/release-3.0.0-startup.json)

The button row uses five paired warm launches in a separate run. It times the first enabled Practice deck button reported by Windows UI Automation and includes the query overhead. It gives a common readiness milestone for the two UI stacks. [Raw UI readiness samples](benchmark-results/release-3.0.0-ui-ready.json)

The packaged executable was rebuilt after the idle and titled-window runs. A byte comparison found 24 changed bytes, all in PE timestamps and the CodeView build GUID. The executable code and other content matched. The UI readiness test used the final package hash `9ed9fc2df3440b310225999874ac837ff44ca20334bff41bcf6d128891cd7fe3`.

## CPU workloads

The current Rust build parsed a shortcut in 0.16 µs, accepted a command through the duplicate filter in 0.04 µs, and dispatched the test Unicode macro in 0.78 µs, all medians. Layout and tessellation took 138 to 360 µs across the six default screens. Buttons with 32 profiles and Unicode macros took 460 µs median and 520 µs p95 batch mean. The workloads use 21 warmed batches and exclude Windows input delivery, GPU rendering, and macro waits. These results have no equivalent 2.1.2 frontend baseline. [Raw CPU workload samples](benchmark-results/release-3.0.0-workloads.json)

## Input latency

The direct Windows media-session probe did not produce comparable command-to-key timings. Windows accepted the requests sent to 2.1.2, but the observer saw no matching tagged key events. The 3.0.0 run stopped when automation could not focus the Practice window. The Windows smoke test passed its capture and keyboard checks. Physical headphone-to-action latency remains unmeasured.

## Release checks

- Passed formatting, all-target Clippy with warnings denied, 89 regular Rust tests, and seven portable PE verification cases.
- Passed the interactive Windows smoke test. It checked Right, Left, Unicode, real Windows media-session events, Monitor behavior, hook recovery, and Stop/restart on an owned test window. Earlier [physical headphone QA](qa.md) used a different executable hash.
- Built and verified the x64 GUI executable, static CRT import checks, portable ZIP contents, installer, and SHA-256 manifest. The artifacts are unsigned.
- The installer now blocks installation over a registered 2.x copy. Both installers use the same per-user uninstall entry. An in-place install to a different directory could replace the entry for the old copy. Uninstall 2.x with **Delete app data** unchecked, then install 3.0 to keep your saved profiles. [Tauri installer template used by 2.1.2](https://raw.githubusercontent.com/tauri-apps/tauri/tauri-cli-v2.12.4/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi)

The hosted Windows workflow checks clean install, migration with saved profiles, and uninstall on a disposable VM before the release workflow prepares a draft. See [releasing](releasing.md) for the automated and interactive installer checks.

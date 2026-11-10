# Renderer and compiler-profile experiments

These experiments explain why the optimized app keeps Glow and `opt-level = "s"`. WGPU/DX12 increased empty-window memory on the measured machine. A minimal D3D11 window used less memory, but it did not render the application. The smaller `z` compiler profile and an app-only `3` profile both caused unacceptable workload regressions.

This report contains diagnostic controls from November 10, 2026. Use [optimization results](optimization.md) for the complete application's measured improvement, and [the original port benchmark](benchmarks.md) for the Tauri comparison. The controls below do not establish a threefold reduction in application memory, size, or overall latency.

## Environment and measurement

All controls ran on Windows 11 Home 10.0.26300, an AMD Ryzen 9 7940HS with 16 logical processors, 31.21 GiB usable RAM, and Radeon 780M graphics with driver 31.0.14084.3001. Builds used Rust 1.99.1 and `x86_64-pc-windows-msvc`. [Environment metadata](benchmark-results/renderer-experiments/environment.json) records the exact values.

Each memory sample launched one owned process, waited eight seconds, read process accounting and virtual-memory metadata, then closed that process. No capture or shortcut delivery was started. Controls used a 1160 by 780 logical window. The D3D11 readiness records confirm a 1740 by 1170 physical client area at 144 DPI.

Private bytes measure process-private committed memory. Working set includes resident shared pages. Neither number measures total GPU memory, and committed virtual regions are not the same as resident physical memory. The sampler used `VirtualQueryEx`, `QueryWorkingSetEx`, mapped-file names and loaded-module metadata. It did not read process-memory contents.

These are individual diagnostic samples, except for the explicitly listed DX12 repeat. They are not the five-run full-app benchmark medians. The D3D11 samples overlapped WGPU compilation, and a compilation-free interval was not independently verified for Glow. They support allocation inspection only. No CPU or startup comparison is made from any of these controls. Application configuration stayed unchanged, and the native-init control read an isolated copy.

## Memory controls

MiB means 1,048,576 bytes. The linked JSON preserves exact bytes, executable hashes, collection times, and region/page metadata.

| Control | Private bytes | Private MiB | Working-set bytes | Working MiB |
| --- | ---: | ---: | ---: | ---: |
| Empty eframe Glow | 106,557,440 | 101.62 | 118,226,944 | 112.75 |
| Empty Glow with idle native initialization | 105,848,832 | 100.95 | 118,804,480 | 113.30 |
| Blank Win32 D3D11 hardware | 34,885,632 | 33.27 | 52,068,352 | 49.66 |
| Blank Win32 D3D11 WARP | 3,440,640 | 3.28 | 35,815,424 | 34.16 |
| Empty eframe DX12, Performance policy | 462,422,016 | 441.00 | 498,040,832 | 474.97 |
| Empty eframe DX12, MemoryUsage policy | 143,994,880 | 137.32 | 174,333,952 | 166.26 |
| Empty eframe DX12, MemoryUsage repeat | 143,196,160 | 136.56 | 174,235,648 | 166.16 |

Raw data: [Glow and native init](benchmark-results/renderer-experiments/memory-diagnostics-renderer.json), [D3D11 hardware](benchmark-results/renderer-experiments/memory-diagnostics-d3d11-hardware.json), [D3D11 WARP](benchmark-results/renderer-experiments/memory-diagnostics-d3d11-warp.json), [DX12 Performance](benchmark-results/renderer-experiments/memory-diagnostics-dx12-performance.json), [DX12 MemoryUsage](benchmark-results/renderer-experiments/memory-diagnostics-dx12-memory-usage.json), [DX12 repeat](benchmark-results/renderer-experiments/memory-diagnostics-dx12-memory-usage-repeat.json).

### Glow and native initialization

The [renderer example](../desktop/examples/renderer_baseline.rs) was built with the normal all-`s` release profile. It creates the app's eframe/Glow window and paints only a solid panel. It does not install the app's custom fonts or construct `AppState` by default. `--native-init` adds `AppState` and initializes the Windows media bridge with the actual HWND. It never starts Monitor or control. Both modes retain eframe's accessibility integration.

The idle native bridge did not produce a visible positive memory delta in these two samples. Its slightly lower private-byte result is sampling variability, not evidence that initialization saves memory. Both modes loaded `opengl32.dll` and the AMD OpenGL driver `atio6axx.dll`. The empty Glow window already accounted for much of the observed process memory. This does not attribute every private allocation to the graphics driver.

An earlier [full-process diagnostic pair](benchmark-results/renderer-experiments/memory-diagnostics-native.json) measured the original Rust build at 148,930,560 private bytes and an intermediate optimized `z` build at 107,884,544 bytes. That pair helped investigate allocation ownership. The intermediate build was rejected later and is not the released candidate. Do not substitute these single samples for the final app benchmark.

### Direct3D11 allocation floor

The isolated probe pins `windows` 0.62.2 and creates a native Win32 window, a D3D11 feature-level 11.0 device, and a two-buffer RGBA8 flip-discard swap chain. It clears and presents three frames, flushes, and enters an idle message loop. Hardware is the default. `--warp` explicitly selects software rendering; hardware failures do not silently fall back.

The probe contains no egui renderer, draw shaders, font atlas, application state, clipboard integration, custom accessibility adapter, or media bridge. Windows can still load UI Automation support. The hardware result establishes a lower allocation floor for this limited program. It does not predict the memory of a complete D3D11 port. The low WARP result gives no evidence about software rendering speed.

The same 124,416-byte executable produced both samples. [Readiness metadata](benchmark-results/renderer-experiments/d3d11-hardware-ready.json) records dimensions, feature level, buffer count and completed presentations. Primary API references are [device and swap-chain creation](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-d3d11createdeviceandswapchain) and [flip-discard presentation](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/ne-dxgi-dxgi_swap_effect).

### WGPU/DX12

The isolated WGPU probe pins eframe 0.33.3 and WGPU 27.0.1, with DX12-only backend features and FXC. Its lockfile resolves egui-wgpu 0.33.3, wgpu-hal 27.0.4 and winit 0.30.13. It uses the same local eframe/egui-winit feature patches as the app, keeps accessibility, and disables bundled fonts. The creation callback verifies DX12 and records the adapter identity. No app state or native media bridge is initialized.

The two policies change WGPU's device memory hints. In the resolved wgpu-hal 27.0.4 DX12 allocator, Performance uses 256 MiB device and 64 MiB host blocks, while MemoryUsage uses 8 MiB and 4 MiB blocks. The smaller-block policy substantially reduced this probe's allocation, but both repeated MemoryUsage samples still exceeded empty Glow on this driver. These results do not support switching the app to WGPU for memory savings.

The empty WGPU executable was 3,469,824 bytes. An EXE-only ZIP was 1,679,924 bytes using PowerShell's default Optimal compression. This is not an application package-size comparison. Exact artifact hashes are in [the artifact manifest](benchmark-results/renderer-experiments/artifacts.json).

## Compiler-profile decisions

The retained release profile uses `opt-level = "s"`, one codegen unit, LTO, stripped symbols and abort-on-panic. Profile experiments held the application implementation constant within each comparison. The hybrid timing gate reused the saved all-`s` benchmark; the intervening `model.rs` difference was verified as formatting only. Benchmark builds use Cargo's bench behavior, including its panic restrictions, so the workload executable is distinct from the packaged application executable.

Each workload performs 100 warmup operations and then 21 timed batches. Two direct executable runs per profile were interleaved as `s, z, z, s` or `s, hybrid, hybrid, s`, after compilation finished. The combined result is the median of 42 batch means per profile. UI rows measure layout and tessellation without GPU submission. Macro callbacks omit Windows delivery and real waits. They do not measure end-to-end input latency.

### Reject `opt-level = "z"`

An early same-source size comparison with font changes but before the vendor feature split measured 2,424,832 bytes for `s` and 2,121,728 bytes for `z`, a 12.50% reduction. The subsequent workload gate included the feature split and found broad regressions. Those are separate controlled comparisons, not one final candidate measurement.

| Workload | `s`, microseconds | `z`, microseconds | Latency increase |
| --- | ---: | ---: | ---: |
| Parse Ctrl+Shift+F5 | 0.13703 | 0.28137 | 105.33% |
| Validate default profiles | 2.24820 | 2.78000 | 23.65% |
| Unicode macro dispatch, no OS I/O | 0.61850 | 2.82280 | 356.39% |
| Buttons UI | 390.05275 | 523.84725 | 34.30% |
| Macros UI | 329.69850 | 511.59600 | 55.17% |
| Settings UI | 314.29350 | 408.02025 | 29.82% |
| Practice UI | 138.80750 | 183.33575 | 32.08% |

[All profile results](benchmark-results/renderer-experiments/profile-gate-summary.json) include the remaining screens and the large-profile diagnostic. The four underlying `profile-gate-*.json` files retain every batch mean. The runtime cost outweighed the smaller executable, so `z` was rejected.

The [separate vendor experiment](benchmark-results/renderer-experiments/vendor-experiment-size-results.json) held `z` and application source fixed and measured 2,121,728 bytes before the feature split versus 1,905,152 after it, a 10.21% reduction. That experiment justified the narrow optional URL/image-clipboard patch, which also applies to `s`. It did not justify shipping `z`. See [vendor provenance and patch scope](../vendor/README.md).

### Reject app-only `opt-level = 3`

This experiment kept dependencies at `s` and set only the `earcontrol` package to `3` in an isolated manifest. Its release executable grew from 2,193,920 to 2,647,040 bytes, a 20.65% increase.

| Workload | All `s`, microseconds | App `3`, microseconds | Latency change |
| --- | ---: | ---: | ---: |
| Parse Ctrl+Shift+F5 | 0.14292 | 0.11302 | -20.92% |
| Validate default profiles | 2.26255 | 1.81575 | -19.75% |
| Unicode macro dispatch, no OS I/O | 0.62210 | 0.83860 | +34.80% |
| Buttons UI | 387.95175 | 388.00675 | +0.01% |
| Macros UI | 329.14900 | 353.38900 | +7.36% |
| Practice UI | 136.28800 | 140.18800 | +2.86% |

Parsing and validation improved, but UI gains were small or absent, Unicode dispatch regressed, and the executable grew. The app-only profile was rejected. [The summary](benchmark-results/renderer-experiments/hybrid-gate-summary.json) and four `hybrid-gate-*.json` files preserve all workloads and batch means.

## Source provenance and reproduction

The [source snapshots](benchmark-results/renderer-experiments/source/) contain each isolated probe's Rust source, manifest and lockfile, plus the metadata sampler. They are outside the application workspace and are not shipped. The sampler's only archival edit replaces the machine-specific repository path with a path relative to its restored `.tools` location. Raw JSON removes process IDs and reduces module/mapped-file paths to basenames. It preserves numeric values, executable hashes and graphics-adapter output. No private settings contents are included.

Both eframe probes use the published 0.33.3 crates and local integration patches whose upstream revision is `44cdd653e2317d300fb8a6c9c36b03f23991e803`. Dependency checksums are in the lockfiles; licenses and the patch upgrade procedure are in `vendor/`. D3D11's Windows bindings came from the locally cached published `windows` 0.62.2 crate. The WGPU dependency set was fetched separately for the isolated experiment.

Restore the archived sources to `.tools` from the repository root. Existing probe directories should be moved aside first if their local changes need to be retained. The commands below are for a future reproduction; writing this report did not rerun them.

```powershell
$source = 'docs/benchmark-results/renderer-experiments/source'
New-Item -ItemType Directory -Force .tools | Out-Null
Copy-Item -Recurse "$source/d3d11-probe" .tools/
Copy-Item -Recurse "$source/wgpu-probe" .tools/
Copy-Item "$source/memory-diagnostics.ps1" .tools/

cargo build --locked --release --example renderer_baseline
& .tools/memory-diagnostics.ps1 -RendererOnly

cargo build --locked --release --manifest-path .tools/d3d11-probe/Cargo.toml
$env:D3D11_PROBE_READY = "$PWD/.tools/d3d11-hardware-ready.json"
& .tools/memory-diagnostics.ps1 -ProbeExe .tools/d3d11-probe/target/release/earcontrol-d3d11-probe.exe -ProbeName empty_d3d11_hardware -OutputName .tools/memory-diagnostics-d3d11-hardware.json
$env:D3D11_PROBE_READY = "$PWD/.tools/d3d11-warp-ready.json"
& .tools/memory-diagnostics.ps1 -ProbeExe .tools/d3d11-probe/target/release/earcontrol-d3d11-probe.exe -ProbeArguments @('--warp') -ProbeName empty_d3d11_warp -OutputName .tools/memory-diagnostics-d3d11-warp.json
Remove-Item Env:D3D11_PROBE_READY

cargo build --locked --release --manifest-path .tools/wgpu-probe/Cargo.toml --target-dir .tools/wgpu-probe/target
& .tools/memory-diagnostics.ps1 -ProbeExe .tools/wgpu-probe/target/release/earcontrol-wgpu-probe.exe -ProbeName empty_eframe_dx12_performance -OutputName .tools/memory-diagnostics-dx12-performance.json
& .tools/memory-diagnostics.ps1 -ProbeExe .tools/wgpu-probe/target/release/earcontrol-wgpu-probe.exe -ProbeArguments @('--memory-usage') -ProbeName empty_eframe_dx12_memory_usage -OutputName .tools/memory-diagnostics-dx12-memory-usage.json
```

Close existing EarControl windows and finish compilation before collecting samples. Run one probe at a time on an unlocked desktop. Use `--offline` when the pinned dependency set is already cached. The helper closes only the process it launched; D3D11 also has a 30-second unattended-exit timer. Future results can differ with OS, drivers, DPI, hardware and application revisions.

The recorded `s`/`z` workload builds used the production source with process-local `CARGO_PROFILE_BENCH_OPT_LEVEL=s` or `z`. The `s` build used the normal target directory; `z` used `.tools/vendor-experiment-target`. The saved executables were `.tools/workloads-opt-s.exe` and `.tools/workloads-opt-z.exe`.

For the hybrid build, the root manifest/lockfile and desktop manifest, build script, source, assets, benches and examples were copied to `.tools/hybrid-experiment`. Only that copy's vendor paths were made absolute. The all-`s` control was built first using `.tools/size-profile` as its target directory. The copy then added `[profile.release.package.earcontrol]` with `opt-level = 3`, and built the hybrid app and workload. The production manifest stayed unchanged.

To repeat a profile gate, freeze one source revision, use separate target directories, and compile `cargo bench --locked --bench workloads --no-run` for both profiles with the bench overrides above. Save the resulting benchmark executables and run them directly in balanced order with no compiler running. Compare the combined batch means. For release-size comparisons, separately build the app with `cargo build --locked --release` and `CARGO_PROFILE_RELEASE_OPT_LEVEL=s` or `z`, or the isolated hybrid package override. Clear any inherited profile overrides first. Do not use the benchmark executable's size as the package size.

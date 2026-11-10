# Contributing to EarControl

Native development requires Windows 10/11 x64, PowerShell 7, stable Rust with the MSVC target and Microsoft C++ Build Tools with the Windows SDK. NSIS 3 is needed only for installers. The app has no Bun, Node, WebView2 or .NET dependency.

From the repository root:

```powershell
./scripts/windows.ps1 -Task Setup
./scripts/windows.ps1 -Task Dev
```

| Task | Purpose |
| --- | --- |
| `Setup` | Fetch locked Cargo dependencies |
| `Dev` | Run the native Rust app |
| `Check` | Check rustfmt and Clippy with warnings denied |
| `Test` | Run unit, settings compatibility and headless UI tests |
| `WindowsTest` | Run the interactive native input/media smoke test |
| `Build` | Build the release executable and portable ZIP |
| `Package` | Build portable files and a per-user NSIS installer |
| `Bench` | Benchmark core workloads and headless UI layout/tessellation |

`./scripts/verify-release.ps1` checks the version and release checksums. Use `-PortableOnly` after `Build`. `scripts/benchmark.ps1` measures release process trees against a supplied baseline. The optional root Bun commands route to these same scripts; Bun is only needed for editing the historical launch video.

The script recognizes an optional local Rust toolchain under `.tools/`. For NSIS, use an installed copy, set `NSIS_HOME`, or place it in `.tools/nsis`. Cargo's lockfile is at the repository root. Update the application version in `desktop/Cargo.toml` and regenerate the lockfile with Cargo.

The committed `.cargo/config.toml` selects the static C runtime for Windows x64. Local and CI builds use that same setting. Do not override it with `RUSTFLAGS` when packaging. Release verification rejects executable imports of Visual C++ runtime DLLs. `Test` also runs the standalone PowerShell checks in `scripts/test-release-verification.ps1`.

Keep native input safeguards in the existing engine/runtime modules. Preserve profile compatibility, draft validation, keyboard operation, accessible labels and both themes. Check the main window at 1160×780 and 860×620, and the practice window at its minimum size. The [architecture](docs/architecture.md) describes the source boundaries.

Run `Check` and `Test` before proposing changes. Input changes also need `WindowsTest` on an unlocked desktop. It focuses an isolated native window and sends actual keys to that window. Record physical headphone testing separately. Do not publish personal window titles, text macros or profile exports.

For an empty Monitor, run `cargo run --locked --example media_diagnostics` while Monitor is active. This read-only check counts EarControl sessions and reports whether Windows selects one. It does not send media commands or display another app's media titles. `WindowsTest` also exercises the real app runtime in Monitor mode, checks activity/repaint, and verifies that monitoring never injects shortcuts.

See [releasing](docs/releasing.md), the [port plan](docs/rust-port-plan.md) and [benchmarks](docs/benchmarks.md). Release binaries, caches and local tools stay outside Git.

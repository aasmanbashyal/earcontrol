# Contributing to EarControl

EarControl is a Windows desktop app. Native development requires Windows 10/11 x64, PowerShell 7 (`pwsh`), Bun 1.3.10, stable Rust with the MSVC target, Microsoft C++ Build Tools with the Windows SDK, and WebView2 Runtime.

## Start

Run commands from the repository root:

```powershell
bun run setup
bun run dev
```

| Command | Purpose |
| --- | --- |
| `bun run dev` | Run the native Tauri app with frontend hot reload |
| `bun run dev:web` | Browser UI preview at `http://127.0.0.1:1420` |
| `bun run check` | TypeScript, production frontend build, and locked Rust check |
| `bun run test` | Frontend tests, production frontend build, and Rust unit tests |
| `bun run test:windows` | Interactive native input/media smoke test |
| `bun run build` | Portable executable and ZIP |
| `bun run package` | Portable files plus the per-user NSIS installer |
| `bun run release:verify` | Version agreement and release SHA-256 checksums |

Browser preview has separate local settings and cannot capture headphones or send native shortcuts. Only `setup` and `dev:web` are intended to work outside Windows. Run `bun run setup` after dependency changes; dependency ownership stays in `desktop/package.json` and its lockfile.

The root package is a command router, not another application package. Windows tasks live in `scripts/windows.ps1`. PowerShell handles the native toolchain and ZIP packaging; there is one implementation shared by local commands and CI. The script also recognizes an optional local Rust toolchain under `.tools/`.

## Changes and checks

Keep session state and draft validation in `App.tsx`, reusable presentation in `components/`, and native behavior in the Rust modules described in [architecture](docs/architecture.md). Preserve keyboard operation, focus return, readable contrast, and both themes. Check the normal 1160×780 window and minimum 860×620 window for interface changes.

Run `bun run test` before proposing a change. Native input changes also need `bun run test:windows` on an unlocked desktop. Keep its isolated window focused; it sends actual keys to that window. Record hardware tests separately from simulated input. Do not paste personal window titles, text macros, or profile exports into issues without reviewing them.

The Windows workflow runs on pushes and pull requests. Include the relevant checks and screenshots in the PR. Release binaries, build caches, installed packages, and local toolchains stay outside Git.

See [releasing](docs/releasing.md) for versioning and packaging, and [the demo source](media/launch/README.md) for video edits.

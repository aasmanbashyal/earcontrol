# Releasing EarControl

## Prepare

1. Update the version together in `desktop/package.json`, `desktop/src-tauri/Cargo.toml`, `desktop/src-tauri/Cargo.lock`, and `desktop/src-tauri/tauri.conf.json`. Update the portable README and changelog. The UI reads its fallback version from package metadata.
2. Run `bun run setup`, `bun run test`, and the interactive `bun run test:windows` on Windows.
3. Close the release executable if it is running, then run `bun run package` and `bun run release:verify`.
4. Inspect the portable launch, light/dark themes, minimum window size, ZIP contents, and installer. Record checks actually performed. Native smoke tests do not establish compatibility with every headphone model.

`dist/` contains a versioned portable folder, ZIP, per-user NSIS installer, and SHA-256 manifest. The portable folder contains the executable, a self-contained README, and the MIT license. The installer also places the MIT license beside the installed executable. The manifest includes the executable inside that folder plus the ZIP and installer. Verify the executable entry after extracting the ZIP.

PowerShell is the Windows packaging implementation in `scripts/windows.ps1`. It validates versions, uses locked Rust builds, creates a fresh portable folder, copies the exact versioned installer, and writes checksums. Root Bun commands and CI call this same script.

## Draft on GitHub

After configuring the repository remote and committing the reviewed source, a pushed `vX.Y.Z` tag triggers `.github/workflows/release.yml`. It rejects a tag that differs from the package version, runs tests, builds and verifies artifacts, then creates a **draft** GitHub Release. It does not publish the draft.

The release job alone has `contents: write`. Ordinary push/PR checks have read-only repository permissions. Inspect the draft's notes, assets, and checksums before publishing. The workflow is intended for a new release tag; if a draft already exists, update it deliberately rather than rerunning `gh release create`.

No remote, tag, release, or public URL is created by local packaging. Configure repository-specific download links and CI badges only after the remote exists.

## Distribution status

Current binaries are unsigned. Code signing, an installed-package upgrade test, and hosted CI execution are separate release checks. WebView2 is required; the installer checks for the runtime. Portable updates replace the application files while profiles remain in the app configuration directory.

Application source is licensed under [MIT](../LICENSE). Demo music and third-party assets retain their separate licenses in [media credits](../media/launch/CREDITS.md).

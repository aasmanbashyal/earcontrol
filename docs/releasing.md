# Releasing EarControl

1. Update `desktop/Cargo.toml`, regenerate `Cargo.lock`, and update the changelog and portable README.
2. Run `./scripts/windows.ps1 -Task Check`, `-Task Test`, and `-Task WindowsTest` on Windows.
3. Close the release executable, then run `./scripts/windows.ps1 -Task Package` and `./scripts/verify-release.ps1`.
4. Inspect both windows, themes, minimum size, portable ZIP and installer. Record checks actually performed. Simulated input does not establish physical headphone compatibility.

`dist/` contains a versioned portable folder, ZIP, per-user NSIS installer and SHA-256 manifest. The folder contains `EarControl.exe`, README, MIT license and third-party font notices. The manifest covers the executable inside that folder, ZIP and installer. `Build` creates only portable files; verify those with `-PortableOnly`.

PowerShell in `scripts/windows.ps1` is the shared local/CI entry point. It uses locked Cargo builds and validates version agreement. NSIS is a build tool only. The installer requires no WebView2 download and runs without administrator privileges. Uninstall removes application files and keeps profiles in the existing roaming configuration directory.

Windows x64 builds link the C runtime statically using `.cargo/config.toml`, so they do not require a separately installed Visual C++ Redistributable. The verifier checks the executable's x64 GUI headers, version resources, and ordinary and RVA-based delay imports for Visual C++ runtime DLLs. This checks declared imports, not arbitrary DLLs loaded at runtime. System Windows DLLs and the OpenGL driver remain required. The same verifier runs in local release checks and both packaging workflows.

The hosted Windows workflow runs `./scripts/test-installer.ps1 -DisposableVm` on a fresh VM after packaging. It tests a clean install and uninstall, then downloads the published 2.1.2 installer and checks its SHA-256. It installs 2.1.2 into a custom directory, writes a version-2 profile fixture, and checks that the 3.0 installer refuses to replace the 2.x registration. After uninstalling 2.1.2 and installing 3.0, it verifies the Start menu target, installed files, and profile hash. It checks the profile hash again after uninstalling 3.0. The Rust test suite loads the same fixture through the app's settings loader.

To repeat installer QA locally, use a disposable Windows VM or Windows Sandbox. A custom `/D` directory still changes the current user's Start menu and uninstall registry entries. For an interactive check, confirm that 3.0 opens the saved profile after uninstalling 2.1.2 with **Delete app data** unchecked, and that reinstalling 3.x while its executable is running asks you to close it.

A pushed `vX.Y.Z` tag triggers `.github/workflows/release.yml`. It checks the tag, tests, packages, verifies and creates a draft GitHub Release for review. Ordinary push/PR jobs have read-only repository permissions. Local packaging does not push a branch, tag or release.

The draft uses `docs/release-notes/X.Y.Z.md` when present and attaches any `docs/benchmark-results/release-X.Y.Z-*.json` datasets alongside the Windows downloads.

Binaries are unsigned. Require a green hosted Windows build for the release revision and a passing installer and migration check before publication. Existing version-2 JSON profiles require no conversion.

Application source is [MIT licensed](../LICENSE). Historical demo assets retain their separate licenses in the media credits.

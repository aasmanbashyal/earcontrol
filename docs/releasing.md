# Releasing EarControl

1. Update `desktop/Cargo.toml`, regenerate `Cargo.lock`, and update the changelog and portable README.
2. Run `./scripts/windows.ps1 -Task Check`, `-Task Test`, and `-Task WindowsTest` on Windows.
3. Close the release executable, then run `./scripts/windows.ps1 -Task Package` and `./scripts/verify-release.ps1`.
4. Inspect both windows, themes, minimum size, portable ZIP and installer. Record checks actually performed. Simulated input does not establish physical headphone compatibility.

`dist/` contains a versioned portable folder, ZIP, per-user NSIS installer and SHA-256 manifest. The folder contains `EarControl.exe`, README, MIT license and third-party font notices. The manifest covers the executable inside that folder, ZIP and installer. `Build` creates only portable files; verify those with `-PortableOnly`.

PowerShell in `scripts/windows.ps1` is the shared local/CI entry point. It uses locked Cargo builds and validates version agreement. NSIS is a build tool only. The installer requires no WebView2 download and runs without administrator privileges. Uninstall removes application files and keeps profiles in the existing roaming configuration directory.

Windows x64 builds link the C runtime statically using `.cargo/config.toml`, so they do not require a separately installed Visual C++ Redistributable. The verifier checks the executable's x64 GUI headers, version resources, and ordinary and RVA-based delay imports for Visual C++ runtime DLLs. This checks declared imports, not arbitrary DLLs loaded at runtime. System Windows DLLs and the OpenGL driver remain required. The same verifier runs in local release checks and both packaging workflows.

Installer execution needs a disposable Windows VM or Windows Sandbox for isolated QA. A custom `/D` directory alone is not isolation: installation still changes the current user's Start menu and uninstall registry entries. In that disposable environment, check a clean install, upgrade from 2.1.2 with existing profiles, refusal to replace a running executable, and uninstall while preserving profiles. Artifact verification does not execute the installer or establish those results.

A pushed `vX.Y.Z` tag triggers `.github/workflows/release.yml`. It checks the tag, tests, packages, verifies and creates a draft GitHub Release for review. Ordinary push/PR jobs have read-only repository permissions. Local packaging does not push a branch, tag or release.

Binaries are unsigned. Hosted CI, code signing and an installed 2.x-to-3.x upgrade test are separate release checks. Do not publish the native port as a tested upgrade until that installation check has been performed. Existing version-2 JSON profiles require no conversion.

Application source is [MIT licensed](../LICENSE). Historical demo assets retain their separate licenses in the media credits.

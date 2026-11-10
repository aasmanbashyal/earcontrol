# EarControl

Map headphone media commands to Windows shortcuts and macros.

https://github.com/user-attachments/assets/53dbceda-f306-42fe-b55f-9e94693331cb

[Download the 20-second demo](https://github.com/aasmanbashyal/earcontrol/raw/refs/heads/main/docs/media/earcontrol-demo.mp4) · [User guide](docs/user-guide.md) · [Development](CONTRIBUTING.md) · [Support](SUPPORT.md)

EarControl lets a supported headphone command advance a slide, run a shortcut, or execute an ordered macro. Use Monitor to see what Windows receives, choose a profile and target window, then start control.

## Get started

This branch contains the **3.0.0 native Rust port**, including both UI windows. Local builds produce the installer and portable files below. Published downloads appear on the [Releases page](https://github.com/aasmanbashyal/earcontrol/releases); this branch does not imply a published 3.0.0 release. The video above shows the earlier 2.x interface.

- **Installer:** `EarControl_3.0.0_x64-setup.exe`, for a per-user installation.
- **Portable:** extract `EarControl-3.0.0-win-x64.zip` and open `EarControl.exe`.

Windows 10/11 x64 with a working OpenGL graphics driver is required. The app needs no WebView2, Bun, Node, Rust or .NET installation. Binaries are unsigned. Downloads include a SHA-256 checksum manifest. Release binaries stay outside Git.

1. Pair headphones with the PC in Windows Bluetooth settings.
2. Open **Monitor** and try each gesture. Activity shows the command received.
3. Choose a profile and target window, or open the **Practice deck**.
4. Click **Start control**, then focus the target window.
5. Click **Stop** to cancel control and any running macro. Closing stops capture; minimizing keeps it running.

## Make the buttons useful

Google Slides and PowerPoint profiles start with these mappings:

| Command | Shortcut | Action |
| --- | --- | --- |
| Play / pause | Right | Next slide |
| Next track | Right | Next slide |
| Previous track | Left | Previous slide |
| Stop playback | Escape | Exit presentation |

Create profiles for other apps. Macros combine **Shortcut**, **Delay**, and **Text** steps, with a countdown before testing and cancellation on Stop or target-focus loss. The practice deck provides slide navigation and a text field for checking mappings. Light, dark, and system themes share the same neutral controls.

The [port plan](docs/rust-port-plan.md) tracks scope and acceptance checks. [Port benchmarks](docs/benchmarks.md) compare the initial native port with the previous release. [Optimization results](docs/optimization.md) measure subsequent size, memory and CPU changes. The [full QA record](docs/qa.md) lists later fixes, physical-device checks, package hashes and remaining verification limits.

Windows controls which media session receives headphone events. Pause competing media apps if Monitor stays empty. Events do not identify the earbud side, tap count, or individual headset. The device list is informational; physical compatibility depends on the headphones and driver. See [headphone support](docs/user-guide.md#headphone-support).

## Build and contribute

Install the prerequisites in [CONTRIBUTING.md](CONTRIBUTING.md), then run from the repository root:

```powershell
./scripts/windows.ps1 -Task Setup
./scripts/windows.ps1 -Task Dev
./scripts/windows.ps1 -Task Test
./scripts/windows.ps1 -Task Package
./scripts/verify-release.ps1
```

The app and both windows are written in Rust using egui/eframe. Windows automation lives in `scripts/windows.ps1`; the optional root Bun commands call the same script. [Architecture](docs/architecture.md) explains the source boundaries. [Releasing](docs/releasing.md) explains version checks, draft releases, and distribution checks.

See the [changelog](CHANGELOG.md) for changes and [demo source and credits](media/launch/README.md) for the illustrated walkthrough. The demo uses the actual browser UI and keyboard-driven practice states; it does not show a physical headphone test.

Application source is [MIT licensed](LICENSE). Demo audio retains the separate licenses listed in its credits.

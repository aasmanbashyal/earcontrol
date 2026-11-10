# EarControl product demo

This is the historical 2.x interface. The 3.x application is native Rust and no longer provides the browser preview used to capture these assets. The saved composition remains editable; recapture UI assets from the native app for a new film.

The finished film is a 20.5-second, 1920×1080 illustrated walkthrough with music and restrained sound effects. It shows actual mappings, the macro editor, and a keyboard-driven practice-slide advance. The [poster](../../docs/media/demo-poster.jpg) is also the film's first frame.

https://github.com/user-attachments/assets/53dbceda-f306-42fe-b55f-9e94693331cb

[Download the MP4](https://github.com/aasmanbashyal/earcontrol/raw/refs/heads/main/docs/media/earcontrol-demo.mp4). GitHub's player uses an uploaded attachment; the original MP4 remains in `docs/media/earcontrol-demo.mp4`. After rendering a replacement, upload it as a GitHub video attachment and update the standalone player URL here and in the root README.

See [credits](CREDITS.md) before sharing; the music requires attribution. [Share copy](share-copy.txt) includes that credit.

## Edit and render

Install Bun, Node.js 22 or newer, and FFmpeg with ffprobe on PATH. Video tooling is isolated from the application dependencies.

```powershell
bun install --cwd media/launch/composition --frozen-lockfile
bun run demo:preview
bun run demo:check
bun run demo:render
```

The preview opens the editable Hyperframes timeline. Each scene is a separate template under `composition/compositions/`. Screenshots, the local GSAP runtime, music, and sound effects live under `composition/assets/`. Source and lockfiles are tracked; render caches are ignored.

`demo:render` checks the composition, renders to `media/launch/brag.mp4`, extracts the settled outro at 18.2 seconds, replaces only frame zero with that poster, and copies the final MP4 and poster into `docs/media/`. It preserves video duration and audio timing. Reinspect representative decoded frames after edits; automated contrast/layout checks cannot judge all screenshot text.

The theme colors and product name are composition variables. Screenshots are real captures, so changing those variables does not repaint controls inside the captured interface. Recapture the app when its appearance changes.

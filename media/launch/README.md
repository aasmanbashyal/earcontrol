# EarControl product demo

This is the historical 2.x interface. The 3.x application is native Rust and no longer provides the browser preview used to capture these assets. The saved composition remains editable; recapture UI assets from the native app for a new film.

The 20.5-second film shows actual mappings, the macro editor, and a keyboard-driven practice-slide advance. The original film and poster are in the [v2.1.2 archive](https://github.com/aasmanbashyal/earcontrol/tree/v2.1.2/docs/media).

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

`demo:render` checks the composition and renders to `media/launch/brag.mp4`. It extracts the settled outro at 18.2 seconds to `media/launch/brag.jpg` and uses it for frame zero, preserving video duration and audio timing. Reinspect representative decoded frames after edits to check screenshot text.

The theme colors and product name are composition variables. Screenshots are real captures, so changing those variables does not repaint controls inside the captured interface. Recapture the app when its appearance changes.

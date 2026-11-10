# Narrow egui integration patches

This directory contains `eframe` 0.33.3 and `egui-winit` 0.33.3 from their published crates. Both crates record upstream egui revision [`44cdd653e2317d300fb8a6c9c36b03f23991e803`](https://github.com/emilk/egui/tree/44cdd653e2317d300fb8a6c9c36b03f23991e803/crates). Source, required assets, original package manifests and upstream README files are preserved. The normalized standalone `Cargo.toml` files are the manifests used by this workspace; `Cargo.toml.orig` retains the unmodified upstream workspace form for reference.

Upstream licenses are preserved in [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). EarControl redistributes these components under the MIT option; the MIT notice is included in `desktop/assets/THIRD-PARTY-NOTICES.txt` for portable and installer distributions.

## Patch scope

- `eframe/Cargo.toml`: exposes `links` and `clipboard-images` as default-enabled features instead of unconditionally enabling both through `egui-winit`.
- `egui-winit/Cargo.toml`: separates text clipboard support from `arboard/image-data`, with `clipboard-images` enabled by default for upstream-compatible defaults.
- `egui-winit/src/clipboard.rs`: gates only the `set_image` implementation on `clipboard-images` and names that feature in its unsupported-operation message.

EarControl opts out of framework defaults and enables `glow` and `accesskit`. Native text copy/paste remains enabled. Current product code never emits `OpenUrl` or `CopyImage` output commands: `Ui::link` navigates within the app, and Bluetooth settings use the native Windows command. File dialogs, text editing, accessibility and media capture are unchanged. PNG icon decoding remains enabled.

The clipboard `get`, `set_text` and `init_arboard` implementations are unchanged from upstream. The feature graph retains `arboard` through `egui-winit/clipboard`, but excludes `arboard/image-data` and `egui-winit/links`. This prevents otherwise reachable framework command handlers from retaining unused URL/IDNA/ICU and image-clipboard code.

An isolated release build with `opt-level=z`, using the same frozen application source for both variants, measured 2,121,728 bytes without the patch and 1,905,152 bytes with it, a 10.21% reduction. This is a build-size comparison, not a runtime performance claim.

## Updating the vendor copies

1. Obtain both matching published crate versions and the licenses from their recorded upstream revision. Replace their source and required assets; do not modify Cargo's registry cache.
2. Review whether upstream now provides equivalent feature switches. If so, remove these patches and the root `[patch.crates-io]` entries.
3. Otherwise reapply only the manifest feature split and image-copy guard described above. Preserve upstream defaults, text clipboard code and all accessibility hooks. Update the pins, provenance and aggregate notices.
4. Run `cargo tree --target x86_64-pc-windows-msvc -e features` and verify text clipboard/accessibility remain enabled while URL opening and image clipboard remain excluded from EarControl's feature set.
5. Run the repository format, Clippy, unit/UI and interactive Windows checks. Before release, use Ctrl+C/Ctrl+V to copy Unicode text between the practice editor and a main-window text field, then close and reopen practice and repeat. Confirm native clipboard sharing with another text editor, along with practice UIA actions and Bluetooth settings navigation.
6. Repeat release-size and workload measurements. Keep the patch only while its measured gain justifies maintaining these two integration copies.
